//! Variable analysis: what a message needs from its caller — the input to
//! the build's manifest (plans/05-tooling.md §3).

use alloc::borrow::Cow;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use mf2_model::{Declaration, Expression, Message, OptionValue, Options, Pattern, PatternPart};

use crate::norm::nfc;

/// A name used by a message: its NFC form (what comparisons and the manifest
/// use) and its spelling as written (first occurrence in source order).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Name<'m> {
    /// The name in Unicode Normalization Form C.
    pub nfc: Cow<'m, str>,
    /// The name as first written in the message.
    pub spelling: &'m str,
}

/// What [`analyze`] found.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Analysis<'m> {
    /// External variables — every variable the caller must supply: the
    /// variables of `.input` declarations and every variable referenced
    /// where no `.local` of that name is in scope (including variables used
    /// only in options, selectors or markup options). Unique under NFC, in
    /// ascending bytewise order of the NFC form: the manifest's slot order.
    pub externals: Vec<Name<'m>>,
    /// Local variables (`.local` declarations), in declaration order, unique
    /// under NFC.
    pub locals: Vec<Name<'m>>,
    /// Markup names, unique under NFC, ascending by NFC form.
    pub markup: Vec<Name<'m>>,
    /// Function identifiers (`number`, `ns:fn`), unique under NFC, ascending
    /// by NFC form.
    pub functions: Vec<Name<'m>>,
}

/// Analyzes `message`.
///
/// Scope follows the declarations: a reference is local if a `.local` of
/// that name comes *before* it (a declaration's own expression does not see
/// the variable it binds). For a valid message this is the same as "every
/// referenced name that no `.local` declares".
pub fn analyze<'m>(message: &'m Message<'_>) -> Analysis<'m> {
    let mut a = Collector::default();
    // NFC names of the locals declared so far (in scope from here on).
    let mut locals: BTreeSet<Cow<'m, str>> = BTreeSet::new();
    for d in message.declarations() {
        match d {
            Declaration::Input(x) => {
                a.external(&x.name, &locals);
                if let Some(f) = &x.value.function {
                    a.functions.insert(&f.name);
                    a.options(&f.options, &locals);
                }
            }
            Declaration::Local(x) => {
                a.expression(&x.value, &locals);
                a.locals.insert(&x.name);
                locals.insert(nfc(&x.name));
            }
        }
    }
    match message {
        Message::Pattern(m) => a.pattern(&m.pattern, &locals),
        Message::Select(m) => {
            for s in &m.selectors {
                a.external(&s.name, &locals);
            }
            for v in &m.variants {
                a.pattern(&v.value, &locals);
            }
        }
    }
    a.finish()
}

/// Names unique under NFC, in first-seen order.
#[derive(Default)]
struct Names<'m> {
    order: Vec<Name<'m>>,
    seen: BTreeSet<Cow<'m, str>>,
}

impl<'m> Names<'m> {
    fn insert(&mut self, name: &'m str) {
        let n = nfc(name);
        if !self.seen.contains(&n) {
            self.seen.insert(n.clone());
            self.order.push(Name {
                nfc: n,
                spelling: name,
            });
        }
    }

    /// In ascending bytewise order of the NFC form.
    fn sorted(mut self) -> Vec<Name<'m>> {
        self.order
            .sort_by(|a, b| a.nfc.as_bytes().cmp(b.nfc.as_bytes()));
        self.order
    }
}

#[derive(Default)]
struct Collector<'m> {
    externals: Names<'m>,
    locals: Names<'m>,
    markup: Names<'m>,
    functions: Names<'m>,
}

impl<'m> Collector<'m> {
    fn external(&mut self, name: &'m str, locals: &BTreeSet<Cow<'m, str>>) {
        if !locals.contains(&nfc(name)) {
            self.externals.insert(name);
        }
    }

    fn options(&mut self, options: &'m Options<'_>, locals: &BTreeSet<Cow<'m, str>>) {
        for (_, v) in options.iter() {
            if let OptionValue::Variable(v) = v {
                self.external(&v.name, locals);
            }
        }
    }

    fn expression(&mut self, e: &'m Expression<'_>, locals: &BTreeSet<Cow<'m, str>>) {
        if let Expression::Variable(v) = e {
            self.external(&v.arg.name, locals);
        }
        if let Some(f) = e.function() {
            self.functions.insert(&f.name);
            self.options(&f.options, locals);
        }
    }

    fn pattern(&mut self, p: &'m Pattern<'_>, locals: &BTreeSet<Cow<'m, str>>) {
        for part in p.parts() {
            match part {
                PatternPart::Expression(e) => self.expression(e, locals),
                PatternPart::Markup(m) => {
                    self.markup.insert(&m.name);
                    self.options(&m.options, locals);
                }
                _ => {}
            }
        }
    }

    fn finish(self) -> Analysis<'m> {
        Analysis {
            externals: self.externals.sorted(),
            locals: self.locals.order,
            markup: self.markup.sorted(),
            functions: self.functions.sorted(),
        }
    }
}
