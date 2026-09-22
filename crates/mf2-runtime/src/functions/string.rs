//! `:string` (`functions/string.md`): formats its operand's string form and
//! selects by comparing the operand's NFC form with the (NFC) key. No
//! options; direction unknown.

use alloc::string::String;

use crate::error::FormatError;
use crate::function::{FnContext, Function, Options};
use crate::number::equals;
use crate::sink::{ErrorSink, Sink};
use crate::text::nfc_quick;
use crate::unannotated;
use crate::value::Value;

/// The `:string` handler.
#[derive(Clone, Copy, Default, Debug)]
pub struct StringFunction;

impl Function for StringFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        _options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        // Any literal, and any value with a string conversion; the rest is a
        // Bad Operand.
        let value = operand.and_then(Value::try_copy);
        match value {
            Some(v) if unannotated::formattable(&v, cx.host()).is_ok() => Some(v),
            _ => {
                errs.error(FormatError::BadOperand);
                None
            }
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        unannotated::format(value, cx.host(), out);
    }

    fn selectable(&self, _value: &Value<'_>) -> bool {
        true
    }

    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        _errs: &mut dyn ErrorSink,
    ) -> bool {
        match value.as_str() {
            Some(s) if nfc_quick(s) => s == key,
            Some(s) => {
                let mut buf = String::new();
                cx.host().nfc(s, &mut buf) == key
            }
            // A number's string form is ASCII, so already NFC.
            None => equals(key, |out| unannotated::format(value, cx.host(), out)),
        }
    }
}
