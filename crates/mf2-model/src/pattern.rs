//! Patterns: sequences of text, expressions and markup.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

use crate::expression::{Expression, Markup};

/// One element of a [`Pattern`].
///
/// Not exhaustive: the spec says future versions may add placeholder kinds.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum PatternPart<'a> {
    /// Literal text, escapes processed. Never empty inside a [`Pattern`].
    Text(Cow<'a, str>),
    /// An expression placeholder.
    Expression(Expression<'a>),
    /// A markup placeholder.
    Markup(Markup<'a>),
}

impl PatternPart<'_> {
    /// The same part with every string owned.
    pub fn into_owned(self) -> PatternPart<'static> {
        match self {
            PatternPart::Text(t) => PatternPart::Text(Cow::Owned(t.into_owned())),
            PatternPart::Expression(e) => PatternPart::Expression(e.into_owned()),
            PatternPart::Markup(m) => PatternPart::Markup(m.into_owned()),
        }
    }
}

/// A pattern: a sequence of parts with no empty `Text` and no two adjacent
/// `Text` parts ([`Pattern::push`] merges and drops accordingly).
///
/// The representation stores one part inline, so a placeholder-free message
/// needs no allocation (the D1 target).
#[derive(Clone, Default)]
pub struct Pattern<'a>(Repr<'a>);

// Private: `Many` may hold any number of parts (it comes from
// `with_capacity` or `From<Vec<_>>`); every observer goes through `parts()`,
// so two representations of the same parts are indistinguishable.
#[derive(Clone, Default)]
enum Repr<'a> {
    #[default]
    Empty,
    One(PatternPart<'a>),
    Many(Vec<PatternPart<'a>>),
}

impl<'a> Pattern<'a> {
    /// The empty pattern (does not allocate).
    pub const fn new() -> Self {
        Pattern(Repr::Empty)
    }

    /// An empty pattern with room for `capacity` parts: no allocation for 0 or
    /// 1 part, one allocation of exactly `capacity` parts otherwise.
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity < 2 {
            Pattern(Repr::Empty)
        } else {
            Pattern(Repr::Many(Vec::with_capacity(capacity)))
        }
    }

    /// The pattern holding just `text`; `""` gives the empty pattern. Does not
    /// allocate.
    pub fn from_text(text: Cow<'a, str>) -> Self {
        if text.is_empty() {
            Pattern(Repr::Empty)
        } else {
            Pattern(Repr::One(PatternPart::Text(text)))
        }
    }

    /// Appends `part`: an empty `Text` is dropped, and a `Text` after a `Text`
    /// is merged into it (which makes the merged text owned).
    pub fn push(&mut self, part: PatternPart<'a>) {
        self.push_with_hint(part, 0);
    }

    /// [`Pattern::push`], reserving room for `more` further parts when the
    /// pattern first needs a vector.
    fn push_with_hint(&mut self, part: PatternPart<'a>, more: usize) {
        let part = match part {
            PatternPart::Text(text) => {
                if text.is_empty() {
                    return;
                }
                if let Some(PatternPart::Text(last)) = self.last_mut() {
                    last.to_mut().push_str(&text);
                    return;
                }
                PatternPart::Text(text)
            }
            other => other,
        };
        self.0 = match core::mem::take(&mut self.0) {
            Repr::Empty => Repr::One(part),
            Repr::One(first) => {
                let mut v = Vec::with_capacity(2 + more);
                v.push(first);
                v.push(part);
                Repr::Many(v)
            }
            Repr::Many(mut v) => {
                v.push(part);
                Repr::Many(v)
            }
        };
    }

    fn last_mut(&mut self) -> Option<&mut PatternPart<'a>> {
        match &mut self.0 {
            Repr::Empty => None,
            Repr::One(p) => Some(p),
            Repr::Many(v) => v.last_mut(),
        }
    }

    /// The parts, in order.
    pub fn parts(&self) -> &[PatternPart<'a>] {
        match &self.0 {
            Repr::Empty => &[],
            Repr::One(p) => core::slice::from_ref(p),
            Repr::Many(v) => v,
        }
    }

    /// Iterates over the parts, in order.
    pub fn iter(&self) -> core::slice::Iter<'_, PatternPart<'a>> {
        self.parts().iter()
    }

    /// How many parts.
    pub fn len(&self) -> usize {
        self.parts().len()
    }

    /// No parts.
    pub fn is_empty(&self) -> bool {
        self.parts().is_empty()
    }

    /// `Some` for the empty pattern (`""`) and for a pattern that is a single
    /// `Text` part; `None` if it has a placeholder.
    pub fn as_simple_text(&self) -> Option<&str> {
        match self.parts() {
            [] => Some(""),
            [PatternPart::Text(t)] => Some(t),
            _ => None,
        }
    }

    /// The parts as a vector (allocates unless the pattern already holds one).
    pub fn into_parts(self) -> Vec<PatternPart<'a>> {
        match self.0 {
            Repr::Empty => Vec::new(),
            Repr::One(p) => alloc::vec![p],
            Repr::Many(v) => v,
        }
    }

    /// The same pattern with every string owned.
    pub fn into_owned(self) -> Pattern<'static> {
        Pattern(match self.0 {
            Repr::Empty => Repr::Empty,
            Repr::One(p) => Repr::One(p.into_owned()),
            Repr::Many(v) => Repr::Many(v.into_iter().map(PatternPart::into_owned).collect()),
        })
    }
}

impl PartialEq for Pattern<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}

impl Eq for Pattern<'_> {}

impl Hash for Pattern<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parts().hash(state);
    }
}

impl core::fmt::Debug for Pattern<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.parts()).finish()
    }
}

impl<'a> FromIterator<PatternPart<'a>> for Pattern<'a> {
    fn from_iter<I: IntoIterator<Item = PatternPart<'a>>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let mut pattern = Pattern::new();
        while let Some(part) = iter.next() {
            pattern.push_with_hint(part, iter.size_hint().0);
        }
        pattern
    }
}

impl<'a> From<Vec<PatternPart<'a>>> for Pattern<'a> {
    /// Keeps the vector's allocation when it already satisfies the pattern
    /// invariants (no empty or adjacent `Text`); otherwise normalizes.
    fn from(parts: Vec<PatternPart<'a>>) -> Self {
        let mut previous_text = false;
        let normal = parts.iter().all(|p| {
            let (ok, is_text) = match p {
                PatternPart::Text(t) => (!t.is_empty() && !previous_text, true),
                _ => (true, false),
            };
            previous_text = is_text;
            ok
        });
        if !normal {
            return parts.into_iter().collect();
        }
        match parts.len() {
            0 => Pattern::new(),
            1 => parts.into_iter().collect(),
            _ => Pattern(Repr::Many(parts)),
        }
    }
}

impl<'p, 'a> IntoIterator for &'p Pattern<'a> {
    type Item = &'p PatternPart<'a>;
    type IntoIter = core::slice::Iter<'p, PatternPart<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.parts().iter()
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::vec;

    use super::{Pattern, PatternPart, Repr};
    use crate::expression::{
        Attributes, Expression, Markup, MarkupKind, Options, VariableExpression, VariableRef,
    };

    fn text(s: &str) -> PatternPart<'_> {
        PatternPart::Text(Cow::Borrowed(s))
    }

    fn var(name: &str) -> PatternPart<'_> {
        PatternPart::Expression(Expression::Variable(VariableExpression {
            arg: VariableRef { name: name.into() },
            function: None,
            attributes: Attributes::new(),
        }))
    }

    fn markup(name: &str) -> PatternPart<'_> {
        PatternPart::Markup(Markup {
            kind: MarkupKind::Open,
            name: name.into(),
            options: Options::new(),
            attributes: Attributes::new(),
        })
    }

    #[test]
    fn empty_text_is_dropped() {
        let mut p = Pattern::new();
        p.push(text(""));
        assert!(p.is_empty());
        assert_eq!(p.as_simple_text(), Some(""));
        assert!(Pattern::from_text(Cow::Borrowed("")).is_empty());
        p.push(var("x"));
        p.push(text(""));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn adjacent_text_merges() {
        let mut p = Pattern::new();
        p.push(text("a"));
        p.push(text("b"));
        assert_eq!(p.parts(), [text("ab")]);
        assert!(matches!(&p.parts()[0], PatternPart::Text(Cow::Owned(_))));
        p.push(var("x"));
        p.push(text("c"));
        p.push(text("d"));
        assert_eq!(p.parts(), [text("ab"), var("x"), text("cd")]);
        assert_eq!(p.as_simple_text(), None);
    }

    #[test]
    fn one_part_is_stored_inline() {
        let p = Pattern::from_text(Cow::Borrowed("Hello"));
        assert!(matches!(p.0, Repr::One(_)));
        assert_eq!(p.as_simple_text(), Some("Hello"));
        let mut q = Pattern::new();
        q.push(markup("b"));
        assert!(matches!(q.0, Repr::One(_)));
        q.push(text("x"));
        assert!(matches!(q.0, Repr::Many(_)));
        // `with_capacity` below 2 does not allocate either.
        assert!(matches!(Pattern::with_capacity(1).0, Repr::Empty));
        let mut r = Pattern::with_capacity(3);
        r.push(text("t"));
        assert_eq!(r.parts(), [text("t")]);
    }

    #[test]
    fn equality_ignores_representation() {
        let inline = Pattern::from_text(Cow::Borrowed("x"));
        let mut many = Pattern::with_capacity(4);
        many.push(text("x"));
        assert!(matches!(many.0, Repr::Many(_)));
        assert_eq!(inline, many);
        assert_eq!(Pattern::new(), Pattern::with_capacity(8));
    }

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn from_vec_and_iter_normalize() {
        let p: Pattern<'_> = vec![text("a"), text(""), text("b"), var("x")].into();
        assert_eq!(p.parts(), [text("ab"), var("x")]);
        let q: Pattern<'_> = vec![text("a"), var("x"), text("b")].into();
        assert_eq!(q.len(), 3);
        let r: Pattern<'_> = vec![var("x")].into();
        assert!(matches!(r.0, Repr::One(_)));
        let s: Pattern<'_> = [text("a"), text("b")].into_iter().collect();
        assert_eq!(s.parts(), [text("ab")]);
        let t: Pattern<'_> = vec![].into();
        assert!(t.is_empty());
        let collected: alloc::vec::Vec<&PatternPart<'_>> = (&q).into_iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(q.clone().into_parts().len(), 3);
    }

    #[test]
    fn into_owned_keeps_value() {
        let mut p = Pattern::new();
        p.push(text("a"));
        p.push(var("x"));
        let owned: Pattern<'static> = p.clone().into_owned();
        assert_eq!(owned, p);
    }
}
