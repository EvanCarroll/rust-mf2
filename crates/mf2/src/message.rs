//! One trait over the call-site descriptions, for code that formats any of
//! them outside Leptos (a native application, a terminal UI adapter).

use leptos_mf2::{Tr, TrArgs, TrDyn, TrRich};
use mf2_runtime::{ErrorSink, Formatter, PartSink, Sink};

/// A call-site description `tr!` builds: formatted to text, or to parts.
pub trait Message {
    /// Formats it into `out`, reporting errors to `errs`.
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink);

    /// Formats it to parts — its markup among them — into `out`.
    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink);
}

impl Message for Tr {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        Tr::write(*self, f, out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        Tr::parts(*self, f, out, errs);
    }
}

impl Message for TrArgs {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        TrArgs::write(self, f, out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        TrArgs::parts(self, f, out, errs);
    }
}

impl Message for TrRich {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        TrRich::write(self, f, out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        TrRich::parts(self, f, out, errs);
    }
}

impl Message for TrDyn {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        TrDyn::write(self, f, out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        TrDyn::parts(self, f, out, errs);
    }
}
