//! Error kinds shared by every crate, and how a frontend reports them.

use alloc::vec::Vec;

/// The kind of an MF2 error: the 13 error types of the WG test suite
/// (`test/README.md`, "Error Codes") plus *Unsupported Operation* and the
/// umbrella *Message Function Error* of `spec/errors.md`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Syntax Error (`syntax-error`): the source is not well-formed.
    Syntax,
    /// Variant Key Mismatch (`variant-key-mismatch`).
    VariantKeyMismatch,
    /// Missing Fallback Variant (`missing-fallback-variant`).
    MissingFallbackVariant,
    /// Missing Selector Annotation (`missing-selector-annotation`).
    MissingSelectorAnnotation,
    /// Duplicate Declaration (`duplicate-declaration`).
    DuplicateDeclaration,
    /// Duplicate Option Name (`duplicate-option-name`).
    DuplicateOptionName,
    /// Duplicate Variant (`duplicate-variant`).
    DuplicateVariant,
    /// Unresolved Variable (`unresolved-variable`).
    UnresolvedVariable,
    /// Unknown Function (`unknown-function`).
    UnknownFunction,
    /// Bad Selector (`bad-selector`).
    BadSelector,
    /// Bad Operand (`bad-operand`).
    BadOperand,
    /// Bad Option (`bad-option`).
    BadOption,
    /// Bad Variant Key (`bad-variant-key`).
    BadVariantKey,
    /// Unsupported Operation — not in the suite's schema; used for an
    /// operation a configuration does not provide (e.g. a locale-dependent
    /// numeric option without the `fn-number` feature).
    UnsupportedOperation,
    /// Any other Message Function Error — the spec's umbrella category, not in
    /// the suite's schema.
    MessageFunctionError,
}

/// The category of an [`ErrorKind`] (`spec/errors.md`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ErrorClass {
    /// The source is not well-formed.
    Syntax,
    /// The message is well-formed but not valid.
    DataModel,
    /// A value cannot be determined at runtime.
    Resolution,
    /// An error from, or specific to, a function handler.
    MessageFunction,
}

impl ErrorKind {
    /// Every kind, in declaration order.
    pub const ALL: [ErrorKind; 15] = [
        ErrorKind::Syntax,
        ErrorKind::VariantKeyMismatch,
        ErrorKind::MissingFallbackVariant,
        ErrorKind::MissingSelectorAnnotation,
        ErrorKind::DuplicateDeclaration,
        ErrorKind::DuplicateOptionName,
        ErrorKind::DuplicateVariant,
        ErrorKind::UnresolvedVariable,
        ErrorKind::UnknownFunction,
        ErrorKind::BadSelector,
        ErrorKind::BadOperand,
        ErrorKind::BadOption,
        ErrorKind::BadVariantKey,
        ErrorKind::UnsupportedOperation,
        ErrorKind::MessageFunctionError,
    ];

    /// The category of this kind.
    pub const fn class(self) -> ErrorClass {
        match self {
            ErrorKind::Syntax => ErrorClass::Syntax,
            ErrorKind::VariantKeyMismatch
            | ErrorKind::MissingFallbackVariant
            | ErrorKind::MissingSelectorAnnotation
            | ErrorKind::DuplicateDeclaration
            | ErrorKind::DuplicateOptionName
            | ErrorKind::DuplicateVariant => ErrorClass::DataModel,
            ErrorKind::UnresolvedVariable | ErrorKind::UnknownFunction | ErrorKind::BadSelector => {
                ErrorClass::Resolution
            }
            ErrorKind::BadOperand
            | ErrorKind::BadOption
            | ErrorKind::BadVariantKey
            | ErrorKind::UnsupportedOperation
            | ErrorKind::MessageFunctionError => ErrorClass::MessageFunction,
        }
    }

    /// The error's name in the WG test suite (`"bad-option"`, …). The 13 kinds
    /// of the suite's schema use its names; the other two use the spec's
    /// names in the same style (`"unsupported-operation"`,
    /// `"message-function-error"`), which never occur in the suite.
    #[cfg(feature = "suite-names")]
    pub fn suite_name(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "syntax-error",
            ErrorKind::VariantKeyMismatch => "variant-key-mismatch",
            ErrorKind::MissingFallbackVariant => "missing-fallback-variant",
            ErrorKind::MissingSelectorAnnotation => "missing-selector-annotation",
            ErrorKind::DuplicateDeclaration => "duplicate-declaration",
            ErrorKind::DuplicateOptionName => "duplicate-option-name",
            ErrorKind::DuplicateVariant => "duplicate-variant",
            ErrorKind::UnresolvedVariable => "unresolved-variable",
            ErrorKind::UnknownFunction => "unknown-function",
            ErrorKind::BadSelector => "bad-selector",
            ErrorKind::BadOperand => "bad-operand",
            ErrorKind::BadOption => "bad-option",
            ErrorKind::BadVariantKey => "bad-variant-key",
            ErrorKind::UnsupportedOperation => "unsupported-operation",
            ErrorKind::MessageFunctionError => "message-function-error",
        }
    }

    /// The kind named `s` (the inverse of [`ErrorKind::suite_name`]).
    #[cfg(feature = "suite-names")]
    pub fn from_suite_name(s: &str) -> Option<ErrorKind> {
        ErrorKind::ALL.into_iter().find(|k| k.suite_name() == s)
    }
}

/// A byte range in the source: `start ≤ end ≤ len`, both on char boundaries.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Span {
    /// Offset of the first byte.
    pub start: u32,
    /// Offset one past the last byte.
    pub end: u32,
}

/// One reported error.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Diagnostic {
    /// What kind of error.
    pub kind: ErrorKind,
    /// 0 = none; otherwise the frontend's stable detail code (`mf2-syntax`
    /// documents its table).
    pub code: u16,
    /// Where, in the source; `None` for models built in code.
    pub span: Option<Span>,
}

/// The errors a frontend or validator reported, in the order reported.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    /// No diagnostics (does not allocate).
    pub const fn new() -> Self {
        Diagnostics(Vec::new())
    }

    /// Appends `d`.
    pub fn push(&mut self, d: Diagnostic) {
        self.0.push(d);
    }

    /// The diagnostics, in the order reported.
    pub fn iter(&self) -> core::slice::Iter<'_, Diagnostic> {
        self.0.iter()
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// None reported.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Some diagnostic has kind `kind`.
    pub fn has(&self, kind: ErrorKind) -> bool {
        self.0.iter().any(|d| d.kind == kind)
    }

    /// Some diagnostic's kind is of class `class`.
    pub fn has_class(&self, class: ErrorClass) -> bool {
        self.0.iter().any(|d| d.kind.class() == class)
    }

    /// The diagnostics as a vector.
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.0
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = core::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<Vec<Diagnostic>> for Diagnostics {
    fn from(v: Vec<Diagnostic>) -> Self {
        Diagnostics(v)
    }
}

impl Extend<Diagnostic> for Diagnostics {
    fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Diagnostics, ErrorClass, ErrorKind, Span};

    #[test]
    fn classes() {
        use ErrorClass::{DataModel, MessageFunction, Resolution, Syntax};
        let classes: alloc::vec::Vec<ErrorClass> =
            ErrorKind::ALL.iter().map(|k| k.class()).collect();
        assert_eq!(
            classes,
            [
                Syntax,
                DataModel,
                DataModel,
                DataModel,
                DataModel,
                DataModel,
                DataModel,
                Resolution,
                Resolution,
                Resolution,
                MessageFunction,
                MessageFunction,
                MessageFunction,
                MessageFunction,
                MessageFunction,
            ]
        );
    }

    #[test]
    fn discriminants_follow_declaration_order() {
        for (i, k) in ErrorKind::ALL.iter().enumerate() {
            assert_eq!(*k as u8 as usize, i);
        }
    }

    #[cfg(feature = "suite-names")]
    #[test]
    fn suite_names_round_trip() {
        for k in ErrorKind::ALL {
            assert_eq!(ErrorKind::from_suite_name(k.suite_name()), Some(k));
        }
        assert_eq!(
            ErrorKind::from_suite_name("bad-option"),
            Some(ErrorKind::BadOption)
        );
        assert_eq!(ErrorKind::from_suite_name("Bad-Option"), None);
        assert_eq!(ErrorKind::from_suite_name(""), None);
    }

    #[test]
    fn diagnostics_queries() {
        let mut d = Diagnostics::new();
        assert!(d.is_empty());
        assert_eq!(d.len(), 0);
        assert!(!d.has(ErrorKind::Syntax));
        d.push(Diagnostic {
            kind: ErrorKind::DuplicateVariant,
            code: 0,
            span: Some(Span { start: 1, end: 4 }),
        });
        assert_eq!(d.len(), 1);
        assert!(d.has(ErrorKind::DuplicateVariant));
        assert!(!d.has(ErrorKind::Syntax));
        assert!(d.has_class(ErrorClass::DataModel));
        assert!(!d.has_class(ErrorClass::Syntax));
        assert_eq!(d.iter().count(), 1);
        let v = d.into_vec();
        assert_eq!(v[0].span, Some(Span { start: 1, end: 4 }));
    }
}
