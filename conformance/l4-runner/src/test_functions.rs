//! The suite's test functions `:test:function`, `:test:select` and
//! `:test:format` (`third_party/message-format-wg/test/README.md`, "Test
//! Functions"), written against `mf2-runtime`'s public [`Function`] trait
//! and nothing else — the proof that the trait can express a custom
//! function (`plans/03-runtime.md` §3).

use std::string::String;

use mf2_runtime::{
    Dir, ErrorSink, FnContext, FormatError, Function, Number, Options, Sink, SubPartSink, Value,
};

/// Which of the three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// `:test:function`: formats and selects.
    Both,
    /// `:test:select`: selects only.
    Select,
    /// `:test:format`: formats only.
    Format,
}

/// A test function handler.
#[derive(Debug)]
pub struct TestFunction {
    role: Role,
}

/// `:test:function`.
pub static FUNCTION: TestFunction = TestFunction { role: Role::Both };
/// `:test:select`.
pub static SELECT: TestFunction = TestFunction { role: Role::Select };
/// `:test:format`.
pub static FORMAT: TestFunction = TestFunction { role: Role::Format };

/// The resolved value of a test function: `Input`, `DecimalPlaces`,
/// `FailsFormat`, `FailsSelect`.
#[derive(Clone)]
struct TestValue {
    input: Number,
    decimal_places: u8,
    fails_format: bool,
    fails_select: bool,
}

/// `Input`'s parts: whether it is below zero, the digits of floor(|Input|),
/// and the first fraction digit.
fn digits(input: &Number) -> (bool, String, char) {
    let mut plain = String::new();
    input.write_plain(&mut plain);
    let (negative, abs) = match plain.strip_prefix('-') {
        Some(rest) => (true, rest.to_owned()),
        None => (false, plain),
    };
    let (int, frac) = abs.split_once('.').unwrap_or((abs.as_str(), ""));
    let zero = int.bytes().all(|b| b == b'0') && frac.bytes().all(|b| b == b'0');
    (
        negative && !zero,
        int.to_owned(),
        frac.chars().next().unwrap_or('0'),
    )
}

fn is_one(input: &Number) -> bool {
    let (negative, int, _) = digits(input);
    !negative && int == "1" && input.is_integer()
}

impl Function for TestFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let inherited = operand.and_then(|v| v.downcast_ref::<TestValue>()).cloned();
        let mut tv = match (inherited, operand) {
            (Some(tv), _) => tv,
            (None, Some(v)) => {
                let Some(input) = v.to_number(cx.host()) else {
                    errs.error(FormatError::BadOperand);
                    return None;
                };
                TestValue {
                    input,
                    decimal_places: 0,
                    fails_format: false,
                    fails_select: false,
                }
            }
            (None, None) => {
                errs.error(FormatError::BadOperand);
                return None;
            }
        };
        if let Some(dp) = options.get("decimalPlaces") {
            // "a numerical integer value 0 or 1 or their corresponding string
            // representations '0' or '1'" — and a `:test:function` value
            // used as an option value is its `Input` (test/README.md).
            let n = match dp.value {
                Value::Int(n) => Some(*n),
                Value::Number(n) if n.is_integer() => n.to_i64(),
                other => match (other.downcast_ref::<TestValue>(), other.as_str()) {
                    (Some(t), _) if t.input.is_integer() => t.input.to_i64(),
                    (_, Some("0")) => Some(0),
                    (_, Some("1")) => Some(1),
                    _ => None,
                },
            };
            let Some(n @ (0 | 1)) = n else {
                // An option whose value was a fallback is not in the mapping
                // at all, so this is a real bad value.
                errs.error(FormatError::BadOption);
                return None;
            };
            tv.decimal_places = u8::try_from(n).unwrap_or(0);
        }
        if let Some(fails) = options.get("fails") {
            match fails.value.as_str() {
                Some("always") => {
                    tv.fails_format = true;
                    tv.fails_select = true;
                }
                Some("format") => tv.fails_format = true,
                Some("select") => tv.fails_select = true,
                Some("never") => {}
                _ => errs.error(FormatError::BadOption),
            }
        }
        Some(Value::Boxed(Box::new(tv)))
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        if self.role == Role::Select {
            // "not-formattable": a Message Function Error of its own.
            return Err(FormatError::MessageFunctionError);
        }
        match value.downcast_ref::<TestValue>() {
            Some(tv) if tv.fails_format => Err(FormatError::BadOption),
            Some(_) => Ok(()),
            None => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        struct Join<'s>(&'s mut dyn Sink);
        impl SubPartSink for Join<'_> {
            fn sub_part(&mut self, _kind: &str, text: &str) {
                self.0.push_str(text);
            }
        }
        self.format_parts(cx, value, &mut Join(out));
    }

    fn format_parts(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        let Some(tv) = value.downcast_ref::<TestValue>() else {
            return;
        };
        let (negative, int, first) = digits(&tv.input);
        if negative {
            out.sub_part("minusSign", "-");
        }
        out.sub_part("integer", &int);
        if tv.decimal_places == 1 {
            out.sub_part("decimal", ".");
            out.sub_part("fraction", first.encode_utf8(&mut [0; 4]));
        }
    }

    fn part_kind(&self) -> &'static str {
        "test"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        Dir::Ltr
    }

    fn selectable(&self, value: &Value<'_>) -> bool {
        self.role != Role::Format
            && value
                .downcast_ref::<TestValue>()
                .is_some_and(|tv| !tv.fails_select)
    }

    fn matches(
        &self,
        _cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        _errs: &mut dyn ErrorSink,
    ) -> bool {
        let Some(tv) = value.downcast_ref::<TestValue>() else {
            return false;
        };
        is_one(&tv.input) && (key == "1" || (tv.decimal_places == 1 && key == "1.0"))
    }

    fn better_than(
        &self,
        _cx: &FnContext<'_>,
        _value: &Value<'_>,
        key1: &str,
        _key2: &str,
    ) -> bool {
        key1 == "1.0"
    }
}
