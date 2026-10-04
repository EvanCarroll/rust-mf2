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

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};
use mf2_catalog::{Catalog, MsgId};
use mf2_runtime::{
    Arg, Category, ErrorSink, FormatContext, FormatError, Formatter, Host, NumberFormatter,
    NumberOut, NumberRequest, NumberStyle, Part, PartSink, Registry, Sink, SubPartSink,
};

/// A host that prints no floats: the host's own cost
/// is not the runtime's (`mf2-host-web`, measured with the Leptos layer).
struct StubHost;

impl Host for StubHost {
    fn f64_to_text<'b>(&self, _x: f64, _buf: &'b mut [u8; 32]) -> Option<&'b str> {
        None
    }
}

static HOST: StubHost = StubHost;
static CX: FormatContext = FormatContext::new(&HOST);

/// The `intl` harnesses' host: the stub host
/// with a number formatter whose answers the host chooses and which hands
/// every field of each request to the host, so the runtime's and
/// `mf2-fn-number`'s `intl` code is all live. Linked only by
/// [`run_intl`]; the browser's formatter (`mf2-host-web`'s `Intl` glue) is
/// measured by `bench/intl-probe`.
struct IntlStubHost;

impl Host for IntlStubHost {
    fn f64_to_text<'b>(&self, _x: f64, _buf: &'b mut [u8; 32]) -> Option<&'b str> {
        None
    }

    fn numbers(&self) -> Option<&dyn NumberFormatter> {
        (param(9) != 0).then_some(&STUB_FORMATTER)
    }
}

struct StubFormatter;

static STUB_FORMATTER: StubFormatter = StubFormatter;

/// Every field of `r`, folded into one number for the host.
fn take(locale: &str, r: &NumberRequest<'_>) {
    sink_bytes(locale.as_bytes());
    sink_bytes(r.value.as_bytes());
    let style = match r.style {
        NumberStyle::Decimal => 1,
        NumberStyle::Percent => 2,
        NumberStyle::Currency {
            code,
            display,
            accounting,
            own_digits,
        } => {
            sink_bytes(code.as_bytes());
            3 ^ (display as u64) << 2 ^ u64::from(accounting) << 5 ^ u64::from(own_digits) << 6
        }
        NumberStyle::Unit { unit, display } => {
            sink_bytes(unit.as_bytes());
            4 ^ (display as u64) << 2
        }
        _ => 5,
    };
    let d = &r.digits;
    let (fmin, fmax) = d.fraction.unwrap_or((255, 255));
    let (smin, smax) = d.significant.unwrap_or((255, 255));
    sink(
        style
            ^ u64::from(r.neutral) << 8
            ^ (r.sign as u64) << 9
            ^ (r.grouping as u64) << 12
            ^ u64::from(r.ordinal) << 15
            ^ u64::from(d.minimum_integer) << 16
            ^ u64::from(fmin) << 24
            ^ u64::from(fmax) << 32
            ^ u64::from(smin) << 40
            ^ u64::from(smax) << 48
            ^ (d.priority as u64) << 56
            ^ u64::from(d.increment) << 3
            ^ (d.mode as u64) << 59
            ^ u64::from(d.strip_if_integer) << 63,
    );
}

impl NumberFormatter for StubFormatter {
    fn format(&self, locale: &str, request: &NumberRequest<'_>, out: NumberOut<'_>) -> bool {
        take(locale, request);
        match out {
            NumberOut::Text(o) => o.push_str(request.value),
            NumberOut::Parts(o) => o.sub_part("integer", request.value),
        }
        param(10) != 0
    }

    fn plural(&self, locale: &str, request: &NumberRequest<'_>) -> Option<Category> {
        take(locale, request);
        let c = param(11);
        (c < 6).then(|| Category::from_code(c as u8))
    }
}

static INTL_HOST: IntlStubHost = IntlStubHost;
static INTL_CX: FormatContext = FormatContext::new(&INTL_HOST);

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
            _ => {}
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
    run_in(registry, &CX)
}

/// [`run`] over the `intl` harnesses' host, which has a number formatter.
pub fn run_intl(registry: &Registry) -> u32 {
    run_in(registry, &INTL_CX)
}

fn run_in(registry: &Registry, cx: &FormatContext) -> u32 {
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
    let f = Formatter::new(&cat, registry, cx);
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
