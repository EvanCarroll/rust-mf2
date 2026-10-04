//! Function handlers and the closed-world registry
//! (budget B13).

use mf2_catalog::Catalog;
use mf2_model::Dir;

use crate::datetime::TimeZone;
use crate::error::FormatError;
use crate::host::Host;
use crate::scratch::Scratch;
use crate::sink::{ErrorSink, Sink, SubPartSink};
use crate::unannotated;
use crate::value::Value;

/// A function handler. `:string`, `:number`, `:integer`, `:offset` are in
/// [`crate::functions`]; custom functions implement the same trait (the
/// suite's `:test:*` functions are written against it).
///
/// A handler first *resolves* an expression to a [`Value`]; the runtime then
/// asks the same handler, for that value, whether and how it formats, its
/// direction, and whether and how it selects.
pub trait Function: Sync {
    /// Function resolution (formatting.md): `operand` resolved — a
    /// [`Value::Fallback`] when it failed to resolve, so the handler decides
    /// — and `options` resolved with
    /// `u:id`/`u:dir` removed. `None` makes the expression a fallback value;
    /// the handler has reported why.
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>>;

    /// Whether `value` can be formatted. `Err(e)`: the placeholder is a
    /// fallback value and `e` is reported.
    fn formattable(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        let _ = (cx, value);
        Ok(())
    }

    /// Writes `value`'s formatted text.
    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink);

    /// Writes `value`'s formatted sub-parts (none by default).
    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        let _ = (cx, value, out);
    }

    /// The `type` of this handler's expression parts. A handler whose parts
    /// are `"datetime"` is a *date function* (`:datetime`, `:date`, `:time`,
    /// and so may an application's own): a message that calls one is a
    /// *date message*, which a Leptos client rewrites after hydration when
    /// the server's date formatter or zone was not its own (`plan/08` §4.3).
    /// Only a date function formats a date: an unannotated date/time is a
    /// Bad Operand. The kind is the mark, so no build pays a method of its
    /// own for it.
    fn part_kind(&self) -> &'static str {
        "string"
    }

    /// The direction of `value`'s formatted text (Default Bidi Strategy);
    /// `Auto` = unknown.
    fn dir(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Dir {
        let _ = (cx, value);
        Dir::Auto
    }

    /// Whether `value` supports selection (formatting.md, "Resolve
    /// Selectors"). A value that does not matches only `*`, with *Bad
    /// Selector*.
    fn selectable(&self, value: &Value<'_>) -> bool {
        let _ = value;
        false
    }

    /// Match(`value`, `key`); `key` is NFC. Report e.g. *Bad Variant Key*
    /// through `errs`.
    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        errs: &mut dyn ErrorSink,
    ) -> bool {
        let _ = (cx, value, key, errs);
        false
    }

    /// `BetterThan(value, key1, key2)`, for two keys that both match.
    fn better_than(&self, cx: &FnContext<'_>, value: &Value<'_>, key1: &str, key2: &str) -> bool {
        let _ = (cx, value, key1, key2);
        false
    }
}

/// What a handler may see of the formatting context: read-only and minimal
/// (formatting.md, "Function Handler").
#[derive(Clone, Copy)]
pub struct FnContext<'x> {
    pub(crate) catalog: &'x Catalog,
    pub(crate) host: &'static dyn Host,
    pub(crate) dir: Option<Dir>,
    pub(crate) time_zone: &'x TimeZone,
}

impl<'x> FnContext<'x> {
    /// The locale (the catalog's).
    pub fn locale(&self) -> &'x str {
        self.catalog.locale()
    }

    /// The expression's `u:dir` (`Ltr`, `Rtl` or `Auto`), if set.
    pub fn dir(&self) -> Option<Dir> {
        self.dir
    }

    /// The platform services.
    pub fn host(&self) -> &'static dyn Host {
        self.host
    }

    /// The catalog, for its locale data (`Catalog::locale_entry`).
    #[doc(hidden)]
    pub fn catalog(&self) -> &'x Catalog {
        self.catalog
    }

    /// Whether `value`, a string the program passed in, is canonically
    /// equivalent to `key`, a string this catalog holds in NFC — the
    /// comparison MF2 asks for between a selector value and a variant key,
    /// and the one `:string` makes. Decided from the small map the catalog
    /// carries (`plan/01` §4.3), so a custom selector can make it without
    /// the normalization tables and without allocating.
    ///
    /// The map answers exactly for any `key` whose characters, decomposed,
    /// it holds, and every key and name of the catalog is such a key.
    /// `None` means that `key` holds a character the map does not reach, so
    /// it cannot decide. A custom selector comparing against a string of
    /// its own may then fall back to byte equality (`value == key`), which
    /// never matches wrongly but misses equivalent spellings, or treat the
    /// key as unsupported. An identical `value`, and two strings below
    /// U+0300, are always answered; a catalog whose keys and names are all
    /// below U+0300 carries an empty map, so there any other pair is `None`.
    pub fn equivalent(&self, value: &str, key: &str) -> Option<bool> {
        crate::nfc::check(self.catalog.nfc_map(), value, key)
    }

    /// The formatting context's time zone (the default of `timeZone`).
    pub fn time_zone(&self) -> &'x TimeZone {
        self.time_zone
    }
}

/// The catalog, `u:dir` and the time zone; the host is left out.
impl core::fmt::Debug for FnContext<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FnContext")
            .field("catalog", self.catalog)
            .field("dir", &self.dir)
            .field("time_zone", self.time_zone)
            .finish_non_exhaustive()
    }
}

/// A resolved option value.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct OptionValue<'o, 'a> {
    /// The value.
    pub value: &'o Value<'a>,
    /// Whether it was set directly by a literal (formatting.md, "Resolved
    /// Values": handlers may require literals, like `select`).
    pub literal: bool,
}

/// The resolved options of an expression, in source order.
pub(crate) type OptionList<'o, 'a> = Scratch<(&'a str, OptionValue<'o, 'a>)>;

/// A view of an [`OptionList`] (possibly empty).
#[derive(Clone, Copy)]
pub(crate) struct OptionEntries<'o, 'a> {
    list: Option<&'o OptionList<'o, 'a>>,
}

impl<'o, 'a> OptionEntries<'o, 'a> {
    pub(crate) fn new(list: &'o OptionList<'o, 'a>) -> Self {
        OptionEntries { list: Some(list) }
    }

    pub(crate) fn len(self) -> usize {
        self.list.map_or(0, Scratch::len)
    }

    pub(crate) fn get(self, i: usize) -> Option<&'o (&'a str, OptionValue<'o, 'a>)> {
        self.list?.get(i)
    }
}

/// The resolved options passed to [`Function::resolve`]. Their order is not
/// significant; a repeated name (a Duplicate Option Name, which the build
/// rejects) resolves to the last.
#[derive(Clone, Copy)]
pub struct Options<'o, 'a> {
    pub(crate) entries: OptionEntries<'o, 'a>,
}

impl<'o, 'a> Options<'o, 'a> {
    /// The option named `name` (NFC).
    pub fn get(&self, name: &str) -> Option<OptionValue<'o, 'a>> {
        let mut found = None;
        for i in 0..self.entries.len() {
            if let Some((n, v)) = self.entries.get(i)
                && *n == name
            {
                found = Some(*v);
            }
        }
        found
    }

    /// Every option, in source order.
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, OptionValue<'o, 'a>)> + 'o {
        let entries = self.entries;
        (0..entries.len()).filter_map(move |i| entries.get(i).copied())
    }

    /// The number of options.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }
}

/// How many options there are. Written apart from the iterator a handler
/// uses (`iter`), so that a client's build inlines that as before.
impl core::fmt::Debug for Options<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Options")
            .field("len", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// The function handlers an application links: closed world (B13). Build
/// code generates it from exactly the functions the corpus uses, e.g.
/// `static REGISTRY: Registry = Registry::new(&[("integer",
/// &mf2_runtime::functions::INTEGER)]);` — an unused handler is never
/// referenced, so never linked.
#[derive(Clone, Copy)]
pub struct Registry {
    functions: &'static [(&'static str, &'static dyn Function)],
    numbers: Option<&'static dyn Function>,
}

impl Registry {
    /// No functions: every annotation is an Unknown Function.
    pub const EMPTY: Registry = Registry {
        functions: &[],
        numbers: None,
    };

    /// The registry of `functions`: `(identifier, handler)`, the identifier
    /// as the catalog's FUNCS has it (`ns:name`, NFC). The first entry of a
    /// repeated identifier wins.
    pub const fn new(functions: &'static [(&'static str, &'static dyn Function)]) -> Registry {
        Registry {
            functions,
            numbers: None,
        }
    }

    /// This registry, with `f` formatting unannotated numeric values
    /// (integer, float and decimal arguments): `mf2-fn-number`'s localized
    /// exact value. The evaluator checks such a
    /// value as any unannotated value (a non-finite float is a Bad Operand)
    /// and then asks `f` for its direction, text, sub-parts and part kind;
    /// it still does not select. Without it they format in neutral digits
    /// (§2.6).
    #[must_use]
    pub const fn with_numbers(self, f: &'static dyn Function) -> Registry {
        Registry {
            numbers: Some(f),
            ..self
        }
    }

    /// The handler for an unannotated value `v`, if the registry has one:
    /// only a number has one. An unannotated date/time is a Bad Operand —
    /// only a date function formats a date (`plan/08` §4.3).
    pub(crate) fn unannotated(&self, v: &Value<'_>) -> Option<&'static dyn Function> {
        if unannotated::is_numeric(v) {
            self.numbers
        } else {
            None
        }
    }

    /// The handler for `name`.
    pub fn get(&self, name: &str) -> Option<&'static dyn Function> {
        self.functions
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, f)| f)
    }

    /// Whether `name` (a FUNCS identifier) is a handler here that formats
    /// dates: its parts are `"datetime"` ([`Function::part_kind`]).
    pub fn is_date_function(&self, name: &str) -> bool {
        self.get(name).is_some_and(|f| f.part_kind() == "datetime")
    }
}

impl core::fmt::Debug for Registry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.functions.iter().map(|(n, _)| n))
            .finish()
    }
}
