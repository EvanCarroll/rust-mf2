//! Client view of the `unit.data` LOCALE entry, v1
//! (`plans/02-catalog-format.md` §4.7): the configured units' patterns per
//! width and plural category, their per-unit patterns and (optionally)
//! display names, and the locale's `per` compound pattern — what `:unit`
//! formats with.
//!
//! Client-path code: borrowing, allocation-free, panic-free, fmt-free.
//! [`Units::parse`] reads the header and bounds the index (O(1));
//! [`Units::get`] binary-searches the records by identifier and parses one.
//! Nothing is walked at load.

use crate::bytes::{Cur, u32_at};
use crate::format::locale_key;
use crate::number::{Forms, Template, str8};
use crate::reader::Catalog;

/// Entry flags bits 0–2: the widths carried (long, short, narrow).
pub(crate) const F_WIDTHS: u8 = 0x07;
/// Entry flag: display names are carried.
pub(crate) const F_NAMES: u8 = 0x08;

/// Block head: the block is the previous width's (nothing follows).
pub(crate) const B_SAME: u8 = 0x01;
/// Block head: a display name follows.
pub(crate) const B_NAME: u8 = 0x02;
/// Block head: a per-unit pattern follows.
pub(crate) const B_PER: u8 = 0x04;

/// `unitDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Width {
    Long = 0,
    Short = 1,
    Narrow = 2,
}

impl Width {
    /// All widths, in entry order.
    pub const ALL: [Width; 3] = [Width::Long, Width::Short, Width::Narrow];

    /// The entry-flag bit of this width.
    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A `unit.data` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Units<'a> {
    flags: u8,
    per: [Option<Template<'a>>; 3],
    index: &'a [u8],
    records: &'a [u8],
}

impl<'a> Units<'a> {
    /// Parses the header and bounds the index; `None` when malformed.
    pub fn parse(entry: &'a [u8]) -> Option<Units<'a>> {
        let mut c = Cur::new(entry, 0);
        let flags = c.u8()?;
        if flags & !(F_WIDTHS | F_NAMES) != 0 {
            return None;
        }
        let mut per = [None; 3];
        for w in Width::ALL {
            if flags & w.bit() != 0
                && let Some(slot) = per.get_mut(w as usize)
            {
                *slot = Some(Template::read(&mut c)?);
            }
        }
        let lo = c.u8()?;
        let hi = c.u8()?;
        let n = usize::from(u16::from_le_bytes([lo, hi]));
        let index = c.take(n.checked_mul(4)?)?;
        let records = entry.get(c.pos()..)?;
        Some(Units {
            flags,
            per,
            index,
            records,
        })
    }

    /// The catalog's `unit.data` entry; `None` when it has none.
    pub fn of(catalog: &'a Catalog) -> Option<Units<'a>> {
        Units::parse(catalog.locale_entry(locale_key::UNIT_DATA)?)
    }

    /// Whether `width` is carried.
    pub const fn has_width(&self, width: Width) -> bool {
        self.flags & width.bit() != 0
    }

    /// Whether display names are carried.
    pub const fn names_carried(&self) -> bool {
        self.flags & F_NAMES != 0
    }

    /// The locale's `per` compound pattern for `width` (`{0} per {1}`,
    /// `{0}/{1}`), to compose `X-per-Y` from two units of the entry when
    /// CLDR has no such unit; `None` when the width is not carried.
    pub fn per_pattern(&self, width: Width) -> Option<Template<'a>> {
        self.per.get(width as usize).copied().flatten()
    }

    /// The number of units.
    pub const fn len(&self) -> usize {
        self.index.len() / 4
    }

    /// Whether the entry has no unit.
    pub const fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    fn offset(&self, i: usize) -> Option<usize> {
        u32_at(self.index, i.checked_mul(4)?).map(|o| o as usize)
    }

    fn id_at(&self, i: usize) -> Option<&'a str> {
        let mut c = Cur::new(self.records, self.offset(i)?);
        str8(&mut c)
    }

    /// The identifiers, in order.
    pub fn ids(&self) -> impl Iterator<Item = &'a str> + '_ {
        (0..self.len()).filter_map(|i| self.id_at(i))
    }

    /// The unit `id` (a CLDR unit identifier without its category:
    /// `kilometer`, `kilometer-per-hour`); `None` when the entry does not have
    /// it or its record is malformed.
    pub fn get(&self, id: &str) -> Option<Unit<'a>> {
        let (mut lo, mut hi) = (0usize, self.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.id_at(mid)?.as_bytes().cmp(id.as_bytes()) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => {
                    let mut c = Cur::new(self.records, self.offset(mid)?);
                    return Unit::read(&mut c, self.flags);
                }
            }
        }
        None
    }

    /// Whether the whole entry is well-formed: identifiers strictly
    /// ascending, offsets ascending and each record ending where the next
    /// begins, nothing trailing. Linear; the writer and tests use it.
    pub fn is_valid(&self) -> bool {
        let mut end = 0usize;
        let mut prev: Option<&str> = None;
        for i in 0..self.len() {
            let Some(off) = self.offset(i) else {
                return false;
            };
            if off != end {
                return false;
            }
            let mut c = Cur::new(self.records, off);
            let Some(u) = Unit::read(&mut c, self.flags) else {
                return false;
            };
            if prev.is_some_and(|p| p.as_bytes() >= u.id.as_bytes()) {
                return false;
            }
            prev = Some(u.id);
            end = c.pos();
        }
        end == self.records.len()
    }
}

/// One width of one unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Block<'a> {
    name: Option<&'a str>,
    per_unit: Option<Template<'a>>,
    patterns: Forms<'a>,
}

/// One unit of a [`Units`] entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unit<'a> {
    id: &'a str,
    blocks: [Option<Block<'a>>; 3],
}

impl<'a> Unit<'a> {
    fn read(c: &mut Cur<'a>, flags: u8) -> Option<Unit<'a>> {
        let id = str8(c)?;
        let mut blocks = [None; 3];
        let mut prev: Option<Block<'a>> = None;
        for w in Width::ALL {
            if flags & w.bit() == 0 {
                continue;
            }
            let head = c.u8()?;
            if head & !(B_SAME | B_NAME | B_PER) != 0 {
                return None;
            }
            let block = if head & B_SAME != 0 {
                if head != B_SAME {
                    return None;
                }
                prev?
            } else {
                let name = if head & B_NAME != 0 {
                    if flags & F_NAMES == 0 {
                        return None;
                    }
                    let s = str8(c)?;
                    if s.bytes().any(|b| b < 0x20) {
                        return None;
                    }
                    Some(s)
                } else {
                    None
                };
                let per_unit = if head & B_PER != 0 {
                    Some(Template::read(c)?)
                } else {
                    None
                };
                Block {
                    name,
                    per_unit,
                    patterns: Forms::read(c, true)?,
                }
            };
            if let Some(slot) = blocks.get_mut(w as usize) {
                *slot = Some(block);
            }
            prev = Some(block);
        }
        Some(Unit { id, blocks })
    }

    fn block(&self, width: Width) -> Option<Block<'a>> {
        self.blocks.get(width as usize).copied().flatten()
    }

    /// The identifier.
    pub const fn id(&self) -> &'a str {
        self.id
    }

    /// The pattern for `width` and plural `category` (0 zero … 5 other; a
    /// category the unit lacks takes `other`'s): `{0}` the formatted number.
    /// `None` when the width is not carried or CLDR has no pattern for this
    /// unit in it.
    pub fn pattern(&self, width: Width, category: u8) -> Option<Template<'a>> {
        self.block(width)?
            .patterns
            .get(category)
            .map(Template::trusted)
    }

    /// Whether CLDR has patterns for this unit in `width`.
    pub fn has_patterns(&self, width: Width) -> bool {
        self.block(width).is_some_and(|b| !b.patterns.is_empty())
    }

    /// The display name for `width` (`kilometers`, `km`); `None` when names
    /// are not carried or CLDR has none.
    pub fn display_name(&self, width: Width) -> Option<&'a str> {
        self.block(width)?.name
    }

    /// The per-unit pattern for `width` (`{0} per hour`, `{0}/h`): the
    /// formatted numerator of `X-per-this` goes in `{0}`.
    pub fn per_unit_pattern(&self, width: Width) -> Option<Template<'a>> {
        self.block(width)?.per_unit
    }
}
