//! The (probe-sized) message model a catalog stores: enough of the MF2 data
//! model to render the reference workload (patterns, placeholders, markup,
//! `.input` + `.match` on one or more selectors).

/// One piece of a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Literal text (already unescaped).
    Text(String),
    /// An external variable, by manifest slot.
    Var(u32),
    /// A literal placeholder (`{|x|}`).
    Lit(String),
    /// `{#name}`.
    MarkupOpen(String),
    /// `{/name}`.
    MarkupClose(String),
    /// `{#name /}`.
    MarkupStandalone(String),
}

/// A variant key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// The catch-all `*`.
    Star,
    /// A literal key.
    Lit(String),
}

/// How a selector value is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    /// `:string` or no annotation: exact match.
    Plain,
    /// `:integer`: exact, then plural category.
    Integer,
    /// `:number`: exact, then plural category.
    Number,
}

/// A selector: a variable slot and its function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    /// Manifest slot.
    pub slot: u32,
    /// Selection function.
    pub func: Func,
}

/// One `.match` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// One key per selector.
    pub keys: Vec<Key>,
    /// The pattern.
    pub pattern: Vec<Part>,
}

/// A message body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A single pattern.
    Pattern(Vec<Part>),
    /// A `.match`.
    Select {
        /// Selectors.
        selectors: Vec<Selector>,
        /// Variants, in source order.
        variants: Vec<Variant>,
    },
}
