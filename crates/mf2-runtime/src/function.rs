//! Function handlers and the closed-world registry (`plans/03-runtime.md`
//! §2.4, §3; budget B13).

use mf2_catalog::Catalog;
use mf2_model::Dir;

use crate::error::FormatError;
use crate::host::Host;
use crate::scratch::Scratch;
use crate::sink::{ErrorSink, Sink, SubPartSink};
use crate::value::Value;

/// A function handler. `:string`, `:number`, `:integer`, `:offset` are in
/// [`crate::functions`]; custom functions implement the same trait (the
/// suite's `:test:*` functions are written against it).
///
/// A handler first *resolves* an expression to a [`Value`]; the runtime then
/// asks the same handler, for that value, whether and how it formats, its
/// direction, and whether and how it selects.
pub trait Function: Sync {
    /// Function resolution (formatting.md): `operand` resolved — never a
    /// fallback value, which the runtime reports as *Bad Operand* itself —
    /// and `options` resolved with `u:id`/`u:dir` removed. `None` makes the
    /// expression a fallback value; the handler has reported why.
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

    /// The `type` of this handler's expression parts.
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
    pub fn catalog(&self) -> &'x Catalog {
        self.catalog
    }
}

/// A resolved option value.
#[derive(Clone, Copy)]
pub struct OptionValue<'o, 'a> {
    /// The value.
    pub value: &'o Value<'a>,
    /// Whether it was set directly by a literal (formatting.md, "Resolved
    /// Values": handlers may require literals, like `select`).
    pub literal: bool,
}

/// The resolved options of an expression, in source order.
pub(crate) type OptionList<'o, 'a> = Scratch<(&'a str, OptionValue<'o, 'a>), 8>;

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

/// The function handlers an application links: closed world (B13). Build
/// code generates it from exactly the functions the corpus uses, e.g.
/// `static REGISTRY: Registry = Registry::new(&[("integer",
/// &mf2_runtime::functions::INTEGER)]);` — an unused handler is never
/// referenced, so never linked.
#[derive(Clone, Copy)]
pub struct Registry {
    functions: &'static [(&'static str, &'static dyn Function)],
}

impl Registry {
    /// No functions: every annotation is an Unknown Function.
    pub const EMPTY: Registry = Registry { functions: &[] };

    /// The registry of `functions`: `(identifier, handler)`, the identifier
    /// as the catalog's FUNCS has it (`ns:name`, NFC). The first entry of a
    /// repeated identifier wins.
    pub const fn new(functions: &'static [(&'static str, &'static dyn Function)]) -> Registry {
        Registry { functions }
    }

    /// The handler for `name`.
    pub fn get(&self, name: &str) -> Option<&'static dyn Function> {
        self.functions
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, f)| f)
    }
}

impl core::fmt::Debug for Registry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.functions.iter().map(|(n, _)| n))
            .finish()
    }
}
