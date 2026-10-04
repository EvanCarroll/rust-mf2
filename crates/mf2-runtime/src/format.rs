//! The entry point.

use mf2_catalog::{Catalog, Entry, MsgId, Names, StrRef};

use crate::datetime::TimeZone;
use crate::error::FormatError;
use crate::eval::{self, Args, Env, Out};
use crate::function::Registry;
use crate::host::Host;
use crate::parts::{ExpressionPart, FallbackSource, Isolation, MarkupPart, Part, PartSink};
use crate::scratch::Scratch;
use crate::sink::{ErrorSink, Sink};
use crate::value::Arg;

/// A bidirectional isolation strategy (formatting.md, "Handling
/// Bidirectional Text").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[non_exhaustive]
pub enum BidiStrategy {
    /// The Default Bidi Strategy: isolate placeholders with U+2066–U+2069.
    #[default]
    Default,
    /// No isolation.
    None,
}

/// What formatting needs beyond the catalog and the registry; build it
/// with [`FormatContext::new`].
#[non_exhaustive]
pub struct FormatContext {
    /// The bidi strategy.
    pub bidi: BidiStrategy,
    /// The platform services.
    pub host: &'static dyn Host,
    /// The default time zone: what `timeZone` defaults to, and the zone of a
    /// floating date/time.
    pub time_zone: TimeZone,
}

impl FormatContext {
    /// The Default Bidi Strategy over `host`, in UTC.
    pub const fn new(host: &'static dyn Host) -> FormatContext {
        FormatContext {
            bidi: BidiStrategy::Default,
            host,
            time_zone: TimeZone::UTC,
        }
    }
}

impl core::fmt::Debug for FormatContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FormatContext")
            .field("bidi", &self.bidi)
            .field("time_zone", &self.time_zone)
            .finish_non_exhaustive()
    }
}

/// Formats messages of one catalog.
#[derive(Clone, Copy)]
pub struct Formatter<'c> {
    catalog: &'c Catalog,
    registry: &'c Registry,
    cx: &'c FormatContext,
}

impl core::fmt::Debug for Formatter<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Formatter")
            .field("catalog", self.catalog)
            .finish_non_exhaustive()
    }
}

impl<'c> Formatter<'c> {
    /// A formatter over `catalog`, with the handlers of `registry`.
    pub const fn new(catalog: &'c Catalog, registry: &'c Registry, cx: &'c FormatContext) -> Self {
        Formatter {
            catalog,
            registry,
            cx,
        }
    }

    /// The catalog.
    pub fn catalog(&self) -> &'c Catalog {
        self.catalog
    }

    /// The text of a `simple` message whose text is valid: no evaluator, no
    /// allocation. `None` for any other message (use [`Formatter::write`]).
    #[inline]
    pub fn simple(&self, id: MsgId) -> Option<&'c str> {
        match self.catalog.get(id) {
            Entry::Simple(r) => self.catalog.text(r),
            _ => None,
        }
    }

    /// The text reference of a `simple` message — the seam for catalog text
    /// as JS strings.
    #[inline]
    #[doc(hidden)]
    pub fn simple_ref(&self, id: MsgId) -> Option<StrRef> {
        match self.catalog.get(id) {
            Entry::Simple(r) => Some(r),
            _ => None,
        }
    }

    /// Formats message `id` with positional `args` into `out`.
    pub fn write(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        self.run(id, Args::Positional(args), &mut StrOut { out }, errs);
    }

    /// Formats message `id` with positional `args` to parts.
    pub fn parts(
        &self,
        id: MsgId,
        args: &[Arg<'_>],
        out: &mut dyn PartSink,
        errs: &mut dyn ErrorSink,
    ) {
        self.run(id, Args::Positional(args), &mut PartsOut { out }, errs);
    }

    /// Formats message `id` with named `args`: each slot takes the argument
    /// whose name is canonically equivalent to the slot's name in NAMES.
    pub fn write_named(
        &self,
        id: MsgId,
        args: &[(&str, Arg<'_>)],
        out: &mut dyn Sink,
        errs: &mut dyn ErrorSink,
    ) {
        let map = self.slot_map(id, args);
        self.run(id, Args::Named(args, &map), &mut StrOut { out }, errs);
    }

    /// Formats message `id` with named `args` to parts.
    pub fn parts_named(
        &self,
        id: MsgId,
        args: &[(&str, Arg<'_>)],
        out: &mut dyn PartSink,
        errs: &mut dyn ErrorSink,
    ) {
        let map = self.slot_map(id, args);
        self.run(id, Args::Named(args, &map), &mut PartsOut { out }, errs);
    }

    /// Per slot of message `id`, the index of its argument in `args`.
    fn slot_map(&self, id: MsgId, args: &[(&str, Arg<'_>)]) -> Scratch<u32> {
        let names = match self.catalog.get(id) {
            Entry::Pattern(m) | Entry::Select(m) => m.names(),
            _ => Names::EMPTY,
        };
        let mut map = Scratch::new();
        // The names are NFC in the catalog; a name the program passed in is
        // whatever it wrote, so the comparison is canonical equivalence,
        // decided from the catalog's own map (`plan/01` §4.3).
        let nfc = self.catalog.nfc_map();
        for slot in 0..names.external_count() {
            let name = names.external(slot).and_then(|r| self.catalog.text(r));
            let found = name.and_then(|name| {
                args.iter()
                    .position(|(n, _)| crate::nfc::equivalent(nfc, n, name))
            });
            let i = found
                .and_then(|i| u32::try_from(i).ok())
                .unwrap_or(u32::MAX);
            if !map.push(i) {
                break;
            }
        }
        map
    }

    fn run(&self, id: MsgId, args: Args<'_, '_>, out: &mut dyn Out, errs: &mut dyn ErrorSink) {
        match self.catalog.get(id) {
            Entry::Simple(r) => {
                if !out.text(self.catalog, r) {
                    errs.error(FormatError::Malformed);
                    let iso = (self.cx.bidi == BidiStrategy::Default).then_some(Isolation::Fsi);
                    out.fallback(FallbackSource::Unknown, iso);
                }
            }
            Entry::Pattern(m) | Entry::Select(m) => {
                let names = m.names();
                // Positional arguments are the message's slots: past the
                // last slot there is none, as with named arguments (only a
                // damaged catalog refers past it).
                let args = match args {
                    Args::Positional(a) => {
                        Args::Positional(a.get(..names.external_count() as usize).unwrap_or(a))
                    }
                    named @ Args::Named(..) => named,
                };
                let env = Env {
                    catalog: self.catalog,
                    registry: self.registry,
                    host: self.cx.host,
                    bidi: self.cx.bidi,
                    time_zone: &self.cx.time_zone,
                    names,
                    args,
                };
                eval::run(&env, m, out, errs);
            }
            Entry::Absent => errs.error(FormatError::MissingMessage),
        }
    }
}

/// String output.
struct StrOut<'s> {
    out: &'s mut dyn Sink,
}

impl Out for StrOut<'_> {
    fn text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        self.out.push_catalog_text(catalog, r)
    }

    fn expression(&mut self, part: ExpressionPart<'_>, iso: Option<Isolation>) {
        if let Some(i) = iso {
            self.out.push_str(i.as_str());
        }
        part.write(self.out);
        if iso.is_some() {
            self.out.push_str(Isolation::Pdi.as_str());
        }
    }

    fn fallback(&mut self, source: FallbackSource<'_>, iso: Option<Isolation>) {
        if let Some(i) = iso {
            self.out.push_str(i.as_str());
        }
        self.out.push_str("{");
        source.write(self.out);
        self.out.push_str("}");
        if iso.is_some() {
            self.out.push_str(Isolation::Pdi.as_str());
        }
    }

    fn markup(&mut self, _part: MarkupPart<'_>) {}

    fn wants_markup(&self) -> bool {
        false
    }
}

/// Parts output.
struct PartsOut<'s> {
    out: &'s mut dyn PartSink,
}

impl Out for PartsOut<'_> {
    fn text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        self.out.part_catalog_text(catalog, r)
    }

    fn expression(&mut self, part: ExpressionPart<'_>, iso: Option<Isolation>) {
        if let Some(i) = iso {
            self.out.part(Part::BidiIsolation(i));
        }
        self.out.part(Part::Expression(part));
        if iso.is_some() {
            self.out.part(Part::BidiIsolation(Isolation::Pdi));
        }
    }

    fn fallback(&mut self, source: FallbackSource<'_>, iso: Option<Isolation>) {
        if let Some(i) = iso {
            self.out.part(Part::BidiIsolation(i));
        }
        self.out.part(Part::Fallback(source));
        if iso.is_some() {
            self.out.part(Part::BidiIsolation(Isolation::Pdi));
        }
    }

    fn markup(&mut self, part: MarkupPart<'_>) {
        self.out.part(Part::Markup(part));
    }

    fn wants_markup(&self) -> bool {
        true
    }
}
