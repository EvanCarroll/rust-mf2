//! Layer L3 — the binary catalog.
//!
//! For each test that parses without data-model errors (the `n/a` matrix
//! excludes the rest): `parse_model` → `analyze` (the slots) →
//! `writer::single` → `Catalog::new` with the manifest hash → `decode`. The
//! decoded model MUST equal the L2 model: the format is lossless over the full
//! data model (F1). On the same message:
//!
//! * the catalog's header says what was written (manifest hash, one message,
//!   locale, direction, not stripped, the id `""` found through IDS), and the
//!   same bytes under another expected hash are rejected with
//!   `ManifestMismatch` (F6);
//! * the stripped catalog (COLD and IDS omitted) has the same manifest, loads
//!   under the same hash, decodes to the [formatting-relevant
//!   model](formatting_model) of the L2 model, and reports `cold_dropped`
//!   exactly when the message has COLD data ([`has_cold_data`]). So stripped
//!   and unstripped catalogs decode to the same formatting-relevant model; that
//!   they also *format* identically is checked from Phase 3 on, when L4 exists
//!   (the ledger's `stripped-formats-identically` note).
//!
//! [`check_model`] takes any model, valid or not (the writer accepts invalid
//! models): `conformance/tests/generated_l3.rs` runs it on ABNF-generated
//! messages.

use std::borrow::Cow;
use std::collections::BTreeMap;

use mf2_catalog::writer::{self, Options as WriteOptions};
use mf2_catalog::{Catalog, CatalogError, Dir, MsgId, decode_report};
use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionExpression, FunctionRef,
    InputDeclaration, Key, Literal, LiteralExpression, LocalDeclaration, Markup, Message,
    OptionValue, Options, Pattern, PatternMessage, PatternPart, SelectMessage, VariableExpression,
    VariableRef, Variant,
};
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::suite::SuiteTest;

/// The one message of a `writer::single` catalog.
const ID: MsgId = MsgId::from_raw(0);

/// The direction the harness writes into the header (it has no effect on
/// the model).
const DIR: Dir = Dir::Ltr;

/// Checks one test at L3: `Ok` = pass, `Err` = what failed.
pub fn check(test: &SuiteTest) -> Result<(), String> {
    check_source(&test.src, &test.locale)
}

/// Checks `src` at L3, written as a catalog for `locale`.
pub fn check_source(src: &str, locale: &str) -> Result<(), String> {
    let parsed = mf2_syntax::parse_model(src);
    let Some(model) = parsed.message else {
        return Err(format!("syntax error: {:?}", parsed.diagnostics));
    };
    check_model(&model, locale)
}

/// Checks the model `m` at L3: the unstripped catalog decodes to `m`, the
/// stripped one to [`formatting_model`]`(m)`.
pub fn check_model(m: &Message<'_>, locale: &str) -> Result<(), String> {
    let analysis = mf2_syntax::analyze(m);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();

    let options = WriteOptions::new(locale, DIR);
    let (bytes, manifest) =
        writer::single(m, &slots, &options).map_err(|e| format!("writer::single: {e}"))?;
    let hash = manifest.hash();
    match Catalog::new(bytes.clone(), hash ^ 1) {
        Err(CatalogError::ManifestMismatch) => {}
        Err(e) => {
            return Err(format!(
                "under a wrong manifest hash: {e:?}, not ManifestMismatch"
            ));
        }
        Ok(_) => return Err("the catalog loads under a wrong manifest hash".to_owned()),
    }
    let cat = Catalog::new(bytes, hash).map_err(|e| format!("Catalog::new: {e:?}"))?;
    header(&cat, hash, locale, false)?;
    let decoded = decode_report(&cat, ID).map_err(|e| format!("decode: {e}"))?;
    if decoded.cold_dropped {
        return Err("the unstripped catalog reports cold_dropped".to_owned());
    }
    if decoded.message != *m {
        return Err(format!(
            "decode(catalog) != model\n    model   {}\n    decoded {}",
            json(m),
            json(&decoded.message)
        ));
    }

    let (bytes, stripped_manifest) = writer::single(m, &slots, &options.stripped())
        .map_err(|e| format!("writer::single, stripped: {e}"))?;
    if stripped_manifest != manifest {
        return Err("stripping changed the manifest".to_owned());
    }
    let cat = Catalog::new(bytes, hash).map_err(|e| format!("Catalog::new, stripped: {e:?}"))?;
    header(&cat, hash, locale, true)?;
    let decoded = decode_report(&cat, ID).map_err(|e| format!("decode, stripped: {e}"))?;
    let want = formatting_model(m);
    if decoded.message != want {
        return Err(format!(
            "decode(stripped catalog) != formatting_model(model)\n    model   {}\n    want    {}\n    decoded {}",
            json(m),
            json(&want),
            json(&decoded.message)
        ));
    }
    let cold = want != *m;
    if decoded.cold_dropped != cold {
        return Err(format!(
            "the stripped catalog reports cold_dropped = {}, but the message {} COLD data: {}",
            decoded.cold_dropped,
            if cold { "has" } else { "has no" },
            json(m)
        ));
    }
    Ok(())
}

/// The header and IDS of a one-message catalog written by the harness.
fn header(cat: &Catalog, hash: u64, locale: &str, stripped: bool) -> Result<(), String> {
    let got = (
        cat.manifest_hash(),
        cat.message_count(),
        cat.locale(),
        cat.dir(),
        cat.cold_stripped(),
        cat.ids_stripped(),
        cat.lookup(""),
    );
    let want = (
        hash,
        1,
        locale,
        DIR,
        stripped,
        stripped,
        (!stripped).then_some(ID),
    );
    if got == want {
        Ok(())
    } else {
        Err(format!(
            "header (hash, count, locale, dir, cold stripped, ids stripped, lookup(\"\")): \
             want {want:?}, got {got:?}"
        ))
    }
}

fn json(m: &Message<'_>) -> String {
    serde_json::to_string(m).unwrap_or_else(|e| format!("<not JSON: {e}>"))
}

/// The **formatting-relevant model** of `m`: what a catalog stripped of COLD
/// decodes to. It is `m` with
///
/// * every attribute removed (of expressions, `.input` expressions and
///   markup);
/// * every catch-all key's value removed (the key stays `*`);
/// * literal keys, function identifiers, option names (of functions and of
///   markup) and markup names in NFC;
/// * every variable reference — an `.input`'s variable (and its name), an
///   operand, an option value, a selector — spelled as what it resolves to:
///   the name *as declared* of the last preceding `.local` whose name is
///   NFC-equal to it (a declaration's own expression does not see the name it
///   binds), otherwise its NFC form (an external variable: its slot name);
///
/// and everything else unchanged: `.local` names as declared, text, literal
/// operands and literal option values as written, and the number and order of
/// declarations, selectors, variants, keys, parts, options.
///
/// The projection is idempotent. It is the identity exactly when `m` has no
/// COLD data ([`has_cold_data`]).
pub fn formatting_model(m: &Message<'_>) -> Message<'static> {
    let mut p = Projection::default();
    let declarations = m.declarations().iter().map(|d| p.declaration(d)).collect();
    match m {
        Message::Pattern(x) => Message::Pattern(PatternMessage {
            declarations,
            pattern: p.pattern(&x.pattern),
        }),
        Message::Select(x) => Message::Select(SelectMessage {
            declarations,
            selectors: x.selectors.iter().map(|s| p.var(s)).collect(),
            variants: x
                .variants
                .iter()
                .map(|v| Variant {
                    keys: v.keys.iter().map(key).collect(),
                    value: p.pattern(&v.value),
                })
                .collect(),
        }),
        other => other.clone().into_owned(),
    }
}

/// Whether `m` has COLD data: an attribute, a catch-all value, or a name,
/// key or identifier spelled otherwise than in its
/// [formatting-relevant model](formatting_model) (a non-NFC spelling, or a
/// reference to a `.local` spelled otherwise than its declaration). Exactly
/// then a stripped catalog loses something of `m`, and its decoder says
/// `cold_dropped`.
pub fn has_cold_data(m: &Message<'_>) -> bool {
    formatting_model(m) != *m
}

/// `s` in NFC.
fn nfc(s: &str) -> Cow<'static, str> {
    Cow::Owned(if is_nfc(s) {
        s.to_owned()
    } else {
        s.nfc().collect()
    })
}

fn owned(s: &str) -> Cow<'static, str> {
    Cow::Owned(s.to_owned())
}

fn literal(l: &Literal<'_>) -> Literal<'static> {
    Literal {
        value: owned(&l.value),
    }
}

fn key(k: &Key<'_>) -> Key<'static> {
    match k {
        Key::Literal(l) => Key::Literal(Literal {
            value: nfc(&l.value),
        }),
        Key::CatchAll(_) => Key::CatchAll(CatchAllKey { value: None }),
        other => other.clone().into_owned(),
    }
}

/// The walk, with the `.local`s in scope.
#[derive(Default)]
struct Projection {
    /// NFC name → the name as declared by the last `.local` binding it so far.
    scope: BTreeMap<String, String>,
}

impl Projection {
    /// A variable reference, spelled as what it resolves to.
    fn name(&self, written: &str) -> Cow<'static, str> {
        let n = nfc(written);
        match self.scope.get(&*n) {
            Some(declared) => owned(declared),
            None => n,
        }
    }

    fn var(&self, v: &VariableRef<'_>) -> VariableRef<'static> {
        VariableRef {
            name: self.name(&v.name),
        }
    }

    fn declaration(&mut self, d: &Declaration<'_>) -> Declaration<'static> {
        match d {
            Declaration::Input(x) => Declaration::Input(InputDeclaration {
                name: self.name(&x.name),
                value: VariableExpression {
                    arg: self.var(&x.value.arg),
                    function: x.value.function.as_ref().map(|f| self.function(f)),
                    attributes: Attributes::new(),
                },
            }),
            Declaration::Local(x) => {
                let value = self.expression(&x.value);
                self.scope
                    .insert(nfc(&x.name).into_owned(), x.name.to_string());
                Declaration::Local(LocalDeclaration {
                    name: owned(&x.name),
                    value,
                })
            }
            other => other.clone().into_owned(),
        }
    }

    fn expression(&self, e: &Expression<'_>) -> Expression<'static> {
        let function = e.function().map(|f| self.function(f));
        let attributes = Attributes::new();
        match e {
            Expression::Literal(x) => Expression::Literal(LiteralExpression {
                arg: literal(&x.arg),
                function,
                attributes,
            }),
            Expression::Variable(x) => Expression::Variable(VariableExpression {
                arg: self.var(&x.arg),
                function,
                attributes,
            }),
            Expression::Function(x) => Expression::Function(FunctionExpression {
                function: self.function(&x.function),
                attributes,
            }),
            // A kind the model may add later: no projection is defined, so
            // it is kept as is (the writer refuses it anyway).
            other => other.clone().into_owned(),
        }
    }

    fn function(&self, f: &FunctionRef<'_>) -> FunctionRef<'static> {
        FunctionRef {
            name: nfc(&f.name),
            options: self.options(&f.options),
        }
    }

    fn options(&self, o: &Options<'_>) -> Options<'static> {
        o.iter()
            .map(|(name, value)| {
                let value = match value {
                    OptionValue::Literal(l) => OptionValue::Literal(literal(l)),
                    OptionValue::Variable(v) => OptionValue::Variable(self.var(v)),
                    other => other.clone().into_owned(),
                };
                (nfc(name), value)
            })
            .collect()
    }

    fn pattern(&self, p: &Pattern<'_>) -> Pattern<'static> {
        p.parts()
            .iter()
            .map(|part| match part {
                PatternPart::Text(t) => PatternPart::Text(owned(t)),
                PatternPart::Expression(e) => PatternPart::Expression(self.expression(e)),
                PatternPart::Markup(m) => PatternPart::Markup(Markup {
                    kind: m.kind,
                    name: nfc(&m.name),
                    options: self.options(&m.options),
                    attributes: Attributes::new(),
                }),
                other => other.clone().into_owned(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use mf2_model::{Key, Message, Pattern, PatternMessage};

    use super::{check_model, check_source, formatting_model, has_cold_data};

    fn parse(src: &str) -> Message<'_> {
        let p = mf2_syntax::parse_model(src);
        p.message
            .unwrap_or_else(|| panic!("{src:?}: {:?}", p.diagnostics))
    }

    /// `formatting_model(parse(src)) == parse(want)`, and the projection is
    /// idempotent.
    fn projects(src: &str, want: &str) {
        let m = parse(src);
        let got = formatting_model(&m);
        assert_eq!(got, parse(want), "{src:?}");
        assert_eq!(formatting_model(&got), got, "idempotent on {src:?}");
        assert_eq!(has_cold_data(&m), src != want, "{src:?}");
        check_source(src, "en").unwrap_or_else(|e| panic!("{src:?}: {e}"));
    }

    #[test]
    fn attributes_are_removed() {
        projects("{$x :number @a @b=|1|}", "{$x :number}");
        projects("{|lit| @a}{:fn @a}", "{|lit|}{:fn}");
        projects(
            ".input {$x :string @a} {{{$x}}}",
            ".input {$x :string} {{{$x}}}",
        );
        projects(".local $y = {$x @a} {{{$y}}}", ".local $y = {$x} {{{$y}}}");
        projects("{#b @t}x{/b @u}{#img @v /}", "{#b}x{/b}{#img /}");
    }

    #[test]
    fn identifiers_keys_and_external_variables_are_nfc() {
        // "é" decomposed (e + U+0301) and composed (U+00E9); Kelvin sign → K.
        projects("{:ns:cafe\u{301}}", "{:ns:caf\u{e9}}");
        projects("{$x :f \u{212a}=1}", "{$x :f K=1}");
        projects(
            "{#cafe\u{301} \u{212a}=$cafe\u{301}}{/cafe\u{301}}",
            "{#caf\u{e9} K=$caf\u{e9}}{/caf\u{e9}}",
        );
        projects("{$\u{212a}}", "{$K}");
        projects(
            ".input {$cafe\u{301} :string} .match $cafe\u{301} cafe\u{301} {{a}} * {{b}}",
            ".input {$caf\u{e9} :string} .match $caf\u{e9} caf\u{e9} {{a}} * {{b}}",
        );
    }

    #[test]
    fn locals_keep_their_declared_spelling() {
        // A reference to a `.local` is spelled as the declaration wrote it.
        projects(
            ".local $\u{212a} = {1} {{{$K} {$\u{212a}}}}",
            ".local $\u{212a} = {1} {{{$\u{212a}} {$\u{212a}}}}",
        );
        projects(
            ".local $K = {1} {{{$\u{212a}}}}",
            ".local $K = {1} {{{$K}}}",
        );
        // A declaration's own expression does not see the name it binds.
        projects(
            ".local $\u{212a} = {$\u{212a}} {{{$\u{212a}}}}",
            ".local $\u{212a} = {$K} {{{$\u{212a}}}}",
        );
    }

    #[test]
    fn literals_and_text_are_unchanged() {
        for src in [
            "cafe\u{301} {|cafe\u{301}|}",
            "{$x :f o=|cafe\u{301}|}",
            "{#b o=|\u{212a}|/}",
            ".local $x = {1} .match $x one {{}} * {{{$x}}}",
        ] {
            projects(src, src);
        }
    }

    #[test]
    fn catch_all_values_are_removed() {
        // No syntax gives a catch-all a value; other formats do.
        let src = ".input {$x :string} .match $x a {{A}} * {{B}}";
        let mut m = parse(src);
        let Message::Select(s) = &mut m else {
            unreachable!()
        };
        s.variants[1].keys[0] = Key::CatchAll(mf2_model::CatchAllKey {
            value: Some(Cow::Borrowed("other")),
        });
        assert!(has_cold_data(&m));
        assert_eq!(formatting_model(&m), parse(src));
        check_model(&m, "en").unwrap();
    }

    #[test]
    fn a_model_the_writer_refuses_fails() {
        // U+0000 has no catalog form (nor any syntax form).
        let m = Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern: Pattern::from_text(Cow::Borrowed("a\0b")),
        });
        let e = check_model(&m, "en").unwrap_err();
        assert!(e.starts_with("writer::single"), "{e}");
        assert!(check_source("{", "en").is_err());
    }
}
