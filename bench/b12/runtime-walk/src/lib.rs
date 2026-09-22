//! The walk of the B12 runtime harnesses (`b12-runtime`,
//! `b12-runtime-nonum`, `b12-runtime-fixed`): the client path of a real
//! application — `Catalog::new` on bytes from the host, then every message
//! formatted with `Formatter::simple`, `write` and `parts`, with arguments of
//! every kind chosen by the host (so no variant is constant-folded away) —
//! against the registry each harness passes (closed world, B13). Outputs and
//! errors are folded into one number for the host's sink; the sinks do not
//! allocate.
//!
//! The host (`b12-harness`'s imports) and this crate's sinks are not the
//! runtime's cost: the stub [`Host`] here is a few bytes, and the browser's is
//! `mf2-host-web`'s, measured with the Leptos layer.
#![no_std]
#![allow(clippy::cast_possible_truncation)]

extern crate alloc;

use alloc::string::String;

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};
use mf2_catalog::{Catalog, MsgId};
use mf2_runtime::{
    Arg, ErrorSink, FormatContext, FormatError, Formatter, Host, Part, PartSink, Registry, Sink,
    SubPartSink,
};

/// A host that normalizes nothing and prints no floats: the host's own cost
/// is not the runtime's (`mf2-host-web`, measured with the Leptos layer).
struct StubHost;

impl Host for StubHost {
    fn nfc<'a>(&self, s: &'a str, _buf: &'a mut String) -> &'a str {
        s
    }

    fn f64_to_text<'b>(&self, _x: f64, _buf: &'b mut [u8; 32]) -> Option<&'b str> {
        None
    }
}

static HOST: StubHost = StubHost;
static CX: FormatContext = FormatContext::new(&HOST);

/// Folds everything written into one number.
struct Fold(u64);

impl Fold {
    #[inline(never)]
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = self.0.rotate_left(5) ^ u64::from(x);
        }
    }
}

impl Sink for Fold {
    fn push_str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
}

impl SubPartSink for Fold {
    fn sub_part(&mut self, kind: &str, text: &str) {
        self.bytes(kind.as_bytes());
        self.bytes(text.as_bytes());
    }
}

impl ErrorSink for Fold {
    fn error(&mut self, e: FormatError) {
        // No `as` on a `#[non_exhaustive]` enum of another crate.
        let code = match e {
            FormatError::UnresolvedVariable => 1,
            FormatError::UnknownFunction => 2,
            FormatError::BadSelector => 3,
            FormatError::BadOperand => 4,
            FormatError::BadOption => 5,
            FormatError::BadVariantKey => 6,
            FormatError::UnsupportedOperation => 7,
            FormatError::MessageFunctionError => 8,
            FormatError::MissingFallbackVariant => 9,
            FormatError::MissingMessage => 10,
            FormatError::Malformed => 11,
            _ => 12,
        };
        self.0 = self.0.rotate_left(3) ^ code;
    }
}

impl PartSink for Fold {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(t) => self.bytes(t.as_bytes()),
            Part::BidiIsolation(i) => self.bytes(i.as_str().as_bytes()),
            Part::Expression(e) => {
                self.bytes(e.kind().as_bytes());
                self.bytes(e.locale().as_bytes());
                self.0 ^= e.dir() as u64;
                if let Some(id) = e.id() {
                    self.bytes(id.as_bytes());
                }
                e.write(self);
                e.sub_parts(self);
            }
            Part::Markup(m) => {
                self.0 ^= m.kind() as u64;
                self.bytes(m.name().as_bytes());
                if let Some(id) = m.id() {
                    self.bytes(id.as_bytes());
                }
                for (name, value) in m.options() {
                    self.bytes(name.as_bytes());
                    if let Some(s) = value.as_str() {
                        self.bytes(s.as_bytes());
                    }
                }
            }
            Part::Fallback(src) => src.write(self),
        }
    }
}

/// The argument of kind `k` (chosen by the host) over the host's `text`.
fn arg(k: u64, text: &str) -> Arg<'_> {
    match k % 5 {
        0 => Arg::Str(text),
        1 => Arg::Int(param(1) as i64),
        2 => Arg::Float(f64::from_bits(param(2))),
        3 => Arg::Decimal(text),
        _ => Arg::Unset,
    }
}

/// Loads the host's catalog and formats every message with `registry`.
pub fn run(registry: &Registry) -> u32 {
    let Some(bytes) = input(INPUT_CATALOG) else {
        return 1;
    };
    let Some(key) = input(INPUT_KEY) else {
        return 1;
    };
    let Ok(cat) = Catalog::new(bytes, param(0)) else {
        return 2;
    };
    let text = core::str::from_utf8(&key).unwrap_or("");
    // Four slots, each of a kind the host chooses.
    let kinds = param(3);
    let args = [
        arg(kinds, text),
        arg(kinds >> 3, text),
        arg(kinds >> 6, text),
        arg(kinds >> 9, text),
    ];
    let f = Formatter::new(&cat, registry, &CX);
    let mut fold = Fold(0);
    for i in 0..=cat.message_count() {
        let Some(id) = MsgId::new(cat.chunk(), i) else {
            continue;
        };
        if let Some(s) = f.simple(id) {
            fold.bytes(s.as_bytes());
        }
        let mut errs = Fold(0);
        f.write(id, &args, &mut fold, &mut errs);
        f.parts(id, &args, &mut fold, &mut errs);
        fold.0 ^= errs.0;
    }
    sink(fold.0);
    sink_bytes(&key);
    sink_bytes(cat.as_bytes());
    0
}
