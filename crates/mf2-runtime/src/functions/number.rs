//! `:number`, `:integer`, `:offset` (`functions/number.md`): the core
//! numeric semantics of [`crate::number`], with neutral symbols. P4's
//! `fn-number` supplies the localized handlers under the same names.

use mf2_model::Dir;

use crate::error::FormatError;
use crate::function::{FnContext, Function, Options};
use crate::number::{self, Spec};
use crate::sink::{ErrorSink, Sink, SubPartSink};
use crate::value::Value;

/// A numeric handler: `:number`, `:integer` or `:offset`.
#[derive(Clone, Copy)]
pub struct NumberFunction {
    spec: Spec,
}

impl NumberFunction {
    pub(crate) const NUMBER: NumberFunction = NumberFunction {
        spec: number::NUMBER,
    };
    pub(crate) const INTEGER: NumberFunction = NumberFunction {
        spec: number::INTEGER,
    };
    pub(crate) const OFFSET: NumberFunction = NumberFunction {
        spec: number::OFFSET,
    };
}

impl Function for NumberFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        number::resolve(self.spec, cx, operand, *options, errs).map(Value::Number)
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Number(_) => Ok(()),
            _ => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Number(n) = value {
            n.write_display(out);
        }
    }

    fn format_parts(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Value::Number(n) = value {
            n.display_parts(out);
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        // Neutral digits (P4: the locale's numbering system decides).
        Dir::Ltr
    }

    fn selectable(&self, value: &Value<'_>) -> bool {
        matches!(value, Value::Number(n) if n.selectable())
    }

    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        errs: &mut dyn ErrorSink,
    ) -> bool {
        match value {
            Value::Number(n) => number::matches(n, cx, key, errs),
            _ => false,
        }
    }

    fn better_than(&self, _cx: &FnContext<'_>, _value: &Value<'_>, key1: &str, key2: &str) -> bool {
        number::better_than(key1, key2)
    }
}

impl core::fmt::Debug for NumberFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NumberFunction")
    }
}
