//! The core functions (`plans/03-runtime.md` §3, §5.1): statics for a
//! [`crate::Registry`]. An application's generated registry names only the
//! ones its corpus uses (closed world, B13).

mod number;
mod string;

pub use number::NumberFunction;
pub use string::StringFunction;

/// `:string` (`functions/string.md`).
pub static STRING: StringFunction = StringFunction;

/// `:number` (`functions/number.md`), neutral symbols.
pub static NUMBER: NumberFunction = NumberFunction::NUMBER;

/// `:integer` (`functions/number.md`), neutral symbols.
pub static INTEGER: NumberFunction = NumberFunction::INTEGER;

/// `:offset` (`functions/number.md`), neutral symbols.
pub static OFFSET: NumberFunction = NumberFunction::OFFSET;
