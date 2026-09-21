//! Validation: the six Data Model Errors (`spec/syntax.md`, `spec/errors.md`).
//!
//! Names and keys are compared under NFC ([`crate::norm`]); the model is not
//! changed. [`validate`] works on any model, including one built in code
//! (spans are then `None`); `parse_model` runs the same checks and maps each
//! error's location to a source span.
//!
//! * **Duplicate Declaration** — a declaration binds a variable that is bound
//!   by, or appears in, a previous declaration (code 104); or its own
//!   expression uses it — for `.input`, within its function (code 105).
//! * **Duplicate Option Name** — two options of one function or markup.
//! * **Missing Selector Annotation** — a selector that does not, directly or
//!   through `.local $x = {$y}` chains, reach a declaration with a function.
//! * **Variant Key Mismatch** — a variant's key count differs from the
//!   selector count (one error per such variant).
//! * **Missing Fallback Variant** — no variant whose keys are all `*`
//!   (whatever their number: `.match $x * * {{…}}` has a fallback and a key
//!   mismatch, as the WG suite expects).
//! * **Duplicate Variant** — a variant whose keys equal an earlier one's
//!   (`*` equals `*`; literals under NFC; `|*|` is a literal, not `*`).
//!
//! Cost: each check compares pairwise, without allocating, while it has at
//! most [`SMALL`] items (every real message); beyond that it sorts or uses an
//! ordered set — O(n log n) — so no input can make validation quadratic.

use alloc::borrow::Cow;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mf2_model::{
    Declaration, Diagnostic, Diagnostics, ErrorKind, Expression, Key, Message, OptionValue,
    Options, Pattern, PatternPart, Variant,
};

use crate::code;
use crate::norm::{nfc, nfc_eq};

/// Up to this many items, checks compare pairwise and allocate nothing.
pub(crate) const SMALL: usize = 16;

/// Where a data-model error is, in terms of the model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Loc {
    /// The variable bound by declaration `i`.
    Declaration(usize),
    /// Selector `i`.
    Selector(usize),
    /// The keys of variant `i`.
    Variant(usize),
    /// The `.match` keyword.
    Matcher,
    /// Option `index` of the function or markup of an expression.
    Option { expr: ExprLoc, index: usize },
}

/// Which expression or markup.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ExprLoc {
    /// The value of declaration `i`.
    Declaration(usize),
    /// Part `part` of the message's pattern (`variant: None`) or of a
    /// variant's pattern.
    Part { variant: Option<usize>, part: usize },
}

/// Checks a data model for the six Data Model Errors. Spans are `None`.
pub fn validate(message: &Message<'_>) -> Diagnostics {
    let mut out = Diagnostics::new();
    check(message, &mut |kind, code, _| {
        out.push(Diagnostic {
            kind,
            code,
            span: None,
        });
    });
    out
}

/// Runs every check, reporting `(kind, code, location)` roughly in source
/// order.
pub(crate) fn check(message: &Message<'_>, report: &mut impl FnMut(ErrorKind, u16, Loc)) {
    let declarations = message.declarations();
    let duplicates = duplicate_declarations(declarations);
    for (i, d) in declarations.iter().enumerate() {
        let duplicate = if let Some(set) = &duplicates {
            set.binary_search(&i).is_ok()
        } else {
            earlier_declarations_use(declarations, i, d.name())
        };
        if duplicate {
            report(
                ErrorKind::DuplicateDeclaration,
                code::DUPLICATE_DECLARATION,
                Loc::Declaration(i),
            );
        } else if uses_itself(d) {
            report(
                ErrorKind::DuplicateDeclaration,
                code::SELF_REFERENCING_DECLARATION,
                Loc::Declaration(i),
            );
        }
        let function = match d {
            Declaration::Input(x) => x.value.function.as_ref(),
            Declaration::Local(x) => x.value.function(),
        };
        if let Some(f) = function {
            check_options(&f.options, ExprLoc::Declaration(i), report);
        }
    }
    match message {
        Message::Pattern(p) => check_pattern(&p.pattern, None, report),
        Message::Select(s) => {
            let annotated = Annotations::new(declarations);
            for (i, selector) in s.selectors.iter().enumerate() {
                if !annotated.is_annotated(declarations, &selector.name) {
                    report(
                        ErrorKind::MissingSelectorAnnotation,
                        code::MISSING_SELECTOR_ANNOTATION,
                        Loc::Selector(i),
                    );
                }
            }
            let has_fallback = s
                .variants
                .iter()
                .any(|v| v.keys.iter().all(|k| matches!(k, Key::CatchAll(_))));
            if !has_fallback {
                report(
                    ErrorKind::MissingFallbackVariant,
                    code::MISSING_FALLBACK_VARIANT,
                    Loc::Matcher,
                );
            }
            let duplicates = duplicate_variants(&s.variants);
            for (i, v) in s.variants.iter().enumerate() {
                if v.keys.len() != s.selectors.len() {
                    report(
                        ErrorKind::VariantKeyMismatch,
                        code::VARIANT_KEY_MISMATCH,
                        Loc::Variant(i),
                    );
                }
                let duplicate = if let Some(set) = &duplicates {
                    set.binary_search(&i).is_ok()
                } else {
                    let earlier = s.variants.get(..i).unwrap_or_default();
                    earlier.iter().any(|w| keys_equal(&w.keys, &v.keys))
                };
                if duplicate {
                    report(
                        ErrorKind::DuplicateVariant,
                        code::DUPLICATE_VARIANT,
                        Loc::Variant(i),
                    );
                }
                check_pattern(&v.value, Some(i), report);
            }
        }
    }
}

// ── declarations ────────────────────────────────────────────────────────────

/// Whether variable `name` is bound by, or appears in, a declaration before
/// `i` (the pairwise form).
fn earlier_declarations_use(declarations: &[Declaration<'_>], i: usize, name: &str) -> bool {
    let earlier = declarations.get(..i).unwrap_or_default();
    earlier
        .iter()
        .any(|p| nfc_eq(p.name(), name) || variables_used(p).any(|v| nfc_eq(v, name)))
}

/// For more than [`SMALL`] declarations: the sorted indices of those that
/// bind a variable bound by, or appearing in, an earlier declaration (one
/// pass with an ordered set of NFC names). `None` below the threshold.
fn duplicate_declarations(declarations: &[Declaration<'_>]) -> Option<Vec<usize>> {
    if declarations.len() <= SMALL {
        return None;
    }
    let mut seen: BTreeSet<Cow<'_, str>> = BTreeSet::new();
    let mut out = Vec::new();
    for (i, d) in declarations.iter().enumerate() {
        let name = nfc(d.name());
        if seen.contains(&name) {
            out.push(i);
        }
        seen.insert(name);
        for v in variables_used(d) {
            seen.insert(nfc(v));
        }
    }
    Some(out)
}

/// The variables a declaration uses other than the one it binds: for
/// `.input`, those in its function's options; for `.local`, its operand and
/// its options.
fn variables_used<'d>(d: &'d Declaration<'_>) -> impl Iterator<Item = &'d str> {
    let (operand, function) = match d {
        Declaration::Input(x) => (None, x.value.function.as_ref()),
        Declaration::Local(x) => (
            match &x.value {
                Expression::Variable(v) => Some(&*v.arg.name),
                _ => None,
            },
            x.value.function(),
        ),
    };
    let options = function.into_iter().flat_map(|f| {
        f.options.iter().filter_map(|(_, v)| match v {
            OptionValue::Variable(v) => Some(&*v.name),
            OptionValue::Literal(_) => None,
        })
    });
    operand.into_iter().chain(options)
}

/// Whether a declaration's own expression uses the variable it binds.
fn uses_itself(d: &Declaration<'_>) -> bool {
    let name = d.name();
    variables_used(d).any(|v| nfc_eq(v, name))
}

/// Which declarations directly or indirectly reference a declaration with a
/// function.
enum Annotations<'d> {
    /// Few declarations: walk the chain back for each selector.
    Walk,
    /// Many: computed in one forward pass. `last` maps an NFC name to the
    /// index of the last declaration binding it.
    Table {
        annotated: Vec<bool>,
        last: BTreeMap<Cow<'d, str>, usize>,
    },
}

impl<'d> Annotations<'d> {
    fn new(declarations: &'d [Declaration<'_>]) -> Self {
        if declarations.len() <= SMALL {
            return Annotations::Walk;
        }
        let mut annotated = Vec::with_capacity(declarations.len());
        let mut last: BTreeMap<Cow<'d, str>, usize> = BTreeMap::new();
        for (i, d) in declarations.iter().enumerate() {
            let a = match d {
                Declaration::Input(x) => x.value.function.is_some(),
                Declaration::Local(x) => {
                    x.value.function().is_some()
                        || match &x.value {
                            Expression::Variable(v) => last
                                .get(&nfc(&v.arg.name))
                                .is_some_and(|&j| annotated.get(j).copied().unwrap_or(false)),
                            _ => false,
                        }
                }
            };
            annotated.push(a);
            last.insert(nfc(d.name()), i);
        }
        Annotations::Table { annotated, last }
    }

    fn is_annotated(&self, declarations: &[Declaration<'_>], name: &str) -> bool {
        match self {
            Annotations::Walk => walk_annotation(declarations, name),
            Annotations::Table { annotated, last } => last
                .get(&nfc(name))
                .is_some_and(|&j| annotated.get(j).copied().unwrap_or(false)),
        }
    }
}

/// The pairwise form: follow `.local $x = {$y}` back to a declaration with a
/// function. Each step looks only at declarations before the current one, so
/// the walk ends even in a message with duplicate declarations.
fn walk_annotation(declarations: &[Declaration<'_>], name: &str) -> bool {
    let mut name = name;
    let mut upto = declarations.len();
    loop {
        let candidates = declarations.get(..upto).unwrap_or_default();
        let Some(i) = candidates.iter().rposition(|d| nfc_eq(d.name(), name)) else {
            return false;
        };
        match candidates.get(i) {
            Some(Declaration::Input(d)) => return d.value.function.is_some(),
            Some(Declaration::Local(d)) => {
                if d.value.function().is_some() {
                    return true;
                }
                match &d.value {
                    Expression::Variable(v) => {
                        name = &v.arg.name;
                        upto = i;
                    }
                    _ => return false,
                }
            }
            None => return false,
        }
    }
}

// ── variants ────────────────────────────────────────────────────────────────

fn keys_equal(a: &[Key<'_>], b: &[Key<'_>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Key::CatchAll(_), Key::CatchAll(_)) => true,
            (Key::Literal(x), Key::Literal(y)) => nfc_eq(&x.value, &y.value),
            _ => false,
        })
}

/// For more than [`SMALL`] variants: the sorted indices of variants whose
/// keys equal an earlier variant's (sorting the key lists, `*` as `None`,
/// literals in NFC). `None` below the threshold.
fn duplicate_variants(variants: &[Variant<'_>]) -> Option<Vec<usize>> {
    if variants.len() <= SMALL {
        return None;
    }
    let mut keyed: Vec<(Vec<Option<Cow<'_, str>>>, usize)> = variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let keys = v
                .keys
                .iter()
                .map(|k| match k {
                    Key::Literal(l) => Some(nfc(&l.value)),
                    Key::CatchAll(_) => None,
                })
                .collect();
            (keys, i)
        })
        .collect();
    keyed.sort();
    Some(later_duplicates(&keyed))
}

/// Given `(key, index)` pairs sorted by key then index, the indices that
/// repeat an earlier key, in ascending order.
fn later_duplicates<K: PartialEq>(sorted: &[(K, usize)]) -> Vec<usize> {
    let mut out: Vec<usize> = sorted
        .windows(2)
        .filter(|w| w[0].0 == w[1].0)
        .map(|w| w[1].1)
        .collect();
    out.sort_unstable();
    out
}

// ── options ─────────────────────────────────────────────────────────────────

fn check_pattern(
    pattern: &Pattern<'_>,
    variant: Option<usize>,
    report: &mut impl FnMut(ErrorKind, u16, Loc),
) {
    for (part, p) in pattern.parts().iter().enumerate() {
        let options = match p {
            PatternPart::Expression(e) => e.function().map(|f| &f.options),
            PatternPart::Markup(m) => Some(&m.options),
            _ => None,
        };
        if let Some(options) = options {
            check_options(options, ExprLoc::Part { variant, part }, report);
        }
    }
}

fn check_options(
    options: &Options<'_>,
    expr: ExprLoc,
    report: &mut impl FnMut(ErrorKind, u16, Loc),
) {
    let mut emit = |index| {
        report(
            ErrorKind::DuplicateOptionName,
            code::DUPLICATE_OPTION_NAME,
            Loc::Option { expr, index },
        );
    };
    if options.len() <= SMALL {
        for (index, (name, _)) in options.iter().enumerate() {
            if options.iter().take(index).any(|(p, _)| nfc_eq(p, name)) {
                emit(index);
            }
        }
    } else {
        let mut keyed: Vec<(Cow<'_, str>, usize)> = options
            .iter()
            .enumerate()
            .map(|(i, (name, _))| (nfc(name), i))
            .collect();
        keyed.sort();
        for index in later_duplicates(&keyed) {
            emit(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::fmt::Write as _;

    use mf2_model::{Diagnostics, ErrorKind, Message};

    use super::{SMALL, validate};

    fn kinds(d: &Diagnostics) -> Vec<ErrorKind> {
        d.iter().map(|x| x.kind).collect()
    }

    fn parse(src: &str) -> Message<'_> {
        crate::parse_model(src).message.expect("parses")
    }

    /// The pairwise (≤ SMALL) and the sorting (> SMALL) forms agree: the same
    /// errors, at the same indices, whichever side of the threshold a message
    /// falls on.
    #[test]
    fn both_forms_of_every_check_agree() {
        for n in [SMALL - 1, SMALL, SMALL + 1, 3 * SMALL] {
            // Options: every third one repeats an earlier name (one written in
            // NFD).
            let mut src = String::from("{:f");
            for i in 0..n {
                let name = if i % 3 == 2 {
                    format!("o{}", i - 2)
                } else {
                    format!("o{i}")
                };
                let _ = write!(src, " {name}=1");
            }
            src.push_str(" \u{e9}=1 e\u{301}=2}");
            let d = validate(&parse(&src));
            assert_eq!(d.len(), n / 3 + 1, "options, n = {n}");
            assert!(
                kinds(&d)
                    .iter()
                    .all(|k| *k == ErrorKind::DuplicateOptionName)
            );

            // Declarations: redeclarations and uses of earlier names.
            let mut src = String::new();
            for i in 0..n {
                let _ = write!(src, ".local $v{i} = {{$e{i}}} ");
            }
            src.push_str(".local $v1 = {1} .local $e2 = {2} .local $w = {$w} {{}}");
            let d = validate(&parse(&src));
            assert_eq!(
                kinds(&d),
                [ErrorKind::DuplicateDeclaration; 3],
                "declarations, n = {n}"
            );

            // Variants: duplicates under NFC, and `*` vs `|*|`.
            let mut src = String::from(".input {$x :f} .match $x ");
            for i in 0..n {
                let _ = write!(src, "k{i} {{{{}}}} ");
            }
            src.push_str("k1 {{}} |*| {{}} \u{e9} {{}} e\u{301} {{}} * {{}} * {{}}");
            let d = validate(&parse(&src));
            assert_eq!(
                kinds(&d),
                [ErrorKind::DuplicateVariant; 3],
                "variants, n = {n}"
            );

            // Selector annotation through a long chain of locals.
            let mut src = String::from(".input {$a0 :f} ");
            for i in 1..n {
                let _ = write!(src, ".local $a{i} = {{$a{}}} ", i - 1);
            }
            let _ = write!(src, ".local $z = {{1}} .match $a{} $z * * {{{{}}}}", n - 1);
            let d = validate(&parse(&src));
            assert_eq!(
                kinds(&d),
                [ErrorKind::MissingSelectorAnnotation],
                "selectors, n = {n}"
            );
        }
    }
}
