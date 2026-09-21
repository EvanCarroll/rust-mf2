//! The NAMES question (A8): format v1 writes a NAMES entry as `varint n_ext ·
//! varint n_local · str32{n}` — fixed 4-byte string references, so a name is
//! one O(1) read — where P0.7's prototype used varint references. This module
//! re-encodes the NAMES section of a finished catalog with another entry
//! encoding and re-points every MESSAGES head at the new entry offsets (the
//! INDEX offsets of the records follow), copying everything else byte for
//! byte, so the size cost of the choice can be measured exactly. The result
//! is only measured, never loaded: `Catalog::new` reads NAMES as `str32`s.

use std::collections::BTreeMap;

use mf2_catalog::format::{HEADER_LEN, SECTION_ENTRY_LEN, header, kind, section};

use crate::error::{Error, Result};

/// How a NAMES entry writes its string references (after its two varint
/// counts `n_ext`, `n_local`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Format v1: fixed 4-byte little-endian string references (a name is one O(1) read).
    Str32,
    /// P0.7's prototype: varint string references (a name is found by skipping the
    /// varints before it).
    Varint,
}

impl Encoding {
    /// Reads one entry at `*pos`: `(n_ext, n_local, refs)`.
    fn read(self, b: &[u8], pos: &mut usize) -> Result<(u32, u32, Vec<u32>)> {
        let n_ext = read_varint(b, pos)?;
        let n_local = read_varint(b, pos)?;
        let n = n_ext
            .checked_add(n_local)
            .ok_or(Error::Names("entry too large"))?;
        let mut refs = Vec::with_capacity(n as usize);
        for _ in 0..n {
            refs.push(match self {
                Encoding::Str32 => {
                    let r = u32_at(b, *pos)?;
                    *pos += 4;
                    r
                }
                Encoding::Varint => read_varint(b, pos)?,
            });
        }
        Ok((n_ext, n_local, refs))
    }

    /// Writes one entry.
    fn write(self, n_ext: u32, n_local: u32, refs: &[u32], out: &mut Vec<u8>) {
        varint(n_ext, out);
        varint(n_local, out);
        for &r in refs {
            match self {
                Encoding::Str32 => out.extend_from_slice(&r.to_le_bytes()),
                Encoding::Varint => varint(r, out),
            }
        }
    }
}

/// Minimal unsigned LEB128.
pub fn varint(mut v: u32, out: &mut Vec<u8>) {
    loop {
        let low = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

fn read_varint(b: &[u8], pos: &mut usize) -> Result<u32> {
    let mut value = 0u32;
    for shift in (0..35).step_by(7) {
        let byte = *b.get(*pos).ok_or(Error::Names("truncated varint"))?;
        *pos += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(Error::Names("varint too long"))
}

fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    b.get(at..at + 2)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(Error::Names("truncated u16"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(Error::Names("truncated u32"))
}

/// What the re-encoding produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reencoded {
    /// The whole catalog with the new NAMES.
    pub bytes: Vec<u8>,
    /// NAMES entries.
    pub entries: usize,
    /// Names (string references) in them.
    pub names: usize,
    /// NAMES section length before.
    pub names_before: usize,
    /// NAMES section length after.
    pub names_after: usize,
    /// MESSAGES section length before.
    pub messages_before: usize,
    /// MESSAGES section length after (heads are varints of the entry offsets).
    pub messages_after: usize,
}

/// Rebuilds `catalog` (a `.mf2b` as the writer made it, its NAMES entries
/// written `from`) with every NAMES entry written `to`. See the module
/// documentation.
pub fn reencode(catalog: &[u8], from: Encoding, to: Encoding) -> Result<Reencoded> {
    let n = usize::from(u16_at(catalog, header::SECTION_COUNT)?);
    let count = u32_at(catalog, header::MESSAGE_COUNT)? as usize;
    let mut sections = Vec::with_capacity(n);
    for i in 0..n {
        let at = HEADER_LEN + i * SECTION_ENTRY_LEN;
        let kind = u16_at(catalog, at)?;
        let off = u32_at(catalog, at + 2)? as usize;
        let len = u32_at(catalog, at + 6)? as usize;
        let data = catalog
            .get(off..off + len)
            .ok_or(Error::Names("section out of bounds"))?;
        sections.push((kind, data.to_vec()));
    }
    let find = |k: u16| {
        sections
            .iter()
            .position(|s| s.0 == k)
            .ok_or(Error::Names("a required section is missing"))
    };
    let (i_index, i_messages, i_names) = (
        find(section::INDEX)?,
        find(section::MESSAGES)?,
        find(section::NAMES)?,
    );

    // NAMES: old entry offset → new entry offset.
    let old_names = &sections[i_names].1;
    let mut map = BTreeMap::new();
    let mut new_names = Vec::with_capacity(old_names.len());
    let (mut entries, mut names) = (0usize, 0usize);
    let mut pos = 0;
    while pos < old_names.len() {
        let start = pos;
        let (n_ext, n_local, refs) = from.read(old_names, &mut pos)?;
        map.insert(start, new_names.len());
        to.write(n_ext, n_local, &refs, &mut new_names);
        entries += 1;
        names += refs.len();
    }

    // INDEX (byte planes) → entries; MESSAGES records are contiguous, in
    // MsgId order, each starting with `varint names`.
    let index = &sections[i_index].1;
    if index.len() != 4 * count {
        return Err(Error::Names("INDEX length"));
    }
    let mut ix: Vec<u32> = (0..count)
        .map(|i| {
            u32::from_le_bytes([
                index[i],
                index[count + i],
                index[2 * count + i],
                index[3 * count + i],
            ])
        })
        .collect();
    let records: Vec<(usize, usize)> = ix
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(*e >> kind::SHIFT, kind::PATTERN | kind::SELECT))
        .map(|(i, e)| (i, (e & kind::OFFSET_MASK) as usize))
        .collect();
    let old_msgs = &sections[i_messages].1;
    let mut new_msgs = Vec::with_capacity(old_msgs.len());
    for (k, &(i, off)) in records.iter().enumerate() {
        let end = records.get(k + 1).map_or(old_msgs.len(), |r| r.1);
        let rec = old_msgs
            .get(off..end)
            .ok_or(Error::Names("MESSAGES record out of bounds"))?;
        let mut p = 0;
        let names_ref = read_varint(rec, &mut p)?;
        let new_ref = match names_ref.checked_sub(1) {
            None => 0,
            Some(old) => {
                let new = map
                    .get(&(old as usize))
                    .ok_or(Error::Names("a head points inside a NAMES entry"))?;
                u32::try_from(*new + 1).map_err(|_| Error::Names("NAMES too large"))?
            }
        };
        let at = u32::try_from(new_msgs.len()).map_err(|_| Error::Names("MESSAGES too large"))?;
        ix[i] = (ix[i] & !kind::OFFSET_MASK) | at;
        varint(new_ref, &mut new_msgs);
        new_msgs.extend_from_slice(&rec[p..]);
    }
    let mut planes = Vec::with_capacity(4 * count);
    for plane in 0..4 {
        planes.extend(ix.iter().map(|e| e.to_le_bytes()[plane]));
    }

    let (names_before, messages_before) = (old_names.len(), old_msgs.len());
    let (names_after, messages_after) = (new_names.len(), new_msgs.len());
    sections[i_index].1 = planes;
    sections[i_messages].1 = new_msgs;
    sections[i_names].1 = new_names;

    // Reassemble: the fixed header unchanged, a new section table, the data.
    let table_len = HEADER_LEN + n * SECTION_ENTRY_LEN;
    let mut out = Vec::with_capacity(catalog.len());
    out.extend_from_slice(&catalog[..HEADER_LEN]);
    let mut off = table_len;
    for (k, data) in &sections {
        out.extend_from_slice(&k.to_le_bytes());
        for v in [off, data.len()] {
            let v = u32::try_from(v).map_err(|_| Error::Names("catalog too large"))?;
            out.extend_from_slice(&v.to_le_bytes());
        }
        off += data.len();
    }
    for (_, data) in &sections {
        out.extend_from_slice(data);
    }
    Ok(Reencoded {
        bytes: out,
        entries,
        names,
        names_before,
        names_after,
        messages_before,
        messages_after,
    })
}

#[cfg(test)]
mod tests {
    use mf2_catalog::writer::{Options, catalog};
    use mf2_catalog::{Catalog, Dir, Manifest};

    use super::{Encoding, reencode};

    /// A small catalog with NAMES entries shared between messages, and a
    /// string reference ≥ 128 (a 2-byte varint).
    fn sample() -> (Vec<u8>, u64) {
        let long = "x".repeat(200);
        let srcs = [
            format!("{long} {{$name}}"),
            "plain".to_owned(),
            "{$a} and {$b}".to_owned(),
            "Bye {$name}!".to_owned(),
            ".local $x = {$a} {{{$x} {$a}}}".to_owned(),
            "{$zzz}".to_owned(),
        ];
        let models: Vec<_> = srcs
            .iter()
            .map(|s| mf2_syntax::parse_model(s).message.unwrap())
            .collect();
        let mut m = Manifest::default();
        for (i, msg) in models.iter().enumerate() {
            let a = mf2_syntax::analyze(msg);
            m.ids.push(format!("id-{i}"));
            m.slots
                .push(a.externals.iter().map(|n| n.nfc.to_string()).collect());
            m.markup.push(Vec::new());
        }
        let refs: Vec<_> = models.iter().map(Some).collect();
        let bytes = catalog(&m, &refs, &Options::new("en", Dir::Ltr).stripped()).unwrap();
        (bytes, m.hash())
    }

    #[test]
    fn str32_to_str32_is_the_identity() {
        let (bytes, hash) = sample();
        Catalog::new(bytes.clone(), hash).unwrap();
        let r = reencode(&bytes, Encoding::Str32, Encoding::Str32).unwrap();
        assert_eq!(r.bytes, bytes);
        assert_eq!(r.entries, 4);
        assert_eq!(r.names, 1 + 2 + 2 + 1);
    }

    #[test]
    fn varint_round_trips_and_only_shrinks_names_and_heads() {
        let (bytes, _) = sample();
        let v = reencode(&bytes, Encoding::Str32, Encoding::Varint).unwrap();
        assert!(v.names_after < v.names_before);
        assert!(v.messages_after <= v.messages_before);
        assert_eq!(
            bytes.len() - v.bytes.len(),
            (v.names_before - v.names_after) + (v.messages_before - v.messages_after)
        );
        let back = reencode(&v.bytes, Encoding::Varint, Encoding::Str32).unwrap();
        assert_eq!(back.bytes, bytes);
    }
}
