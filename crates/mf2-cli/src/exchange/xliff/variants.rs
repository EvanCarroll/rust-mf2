//! Which variants a message that selects offers a translator
//! (`plans/05-tooling.md` §6.3, owner question 7 of
//! `plans/16-phase-8-work-order.md`: the **target** language's plural
//! forms).
//!
//! A message is seen as selectors and variants; a message without `.match`
//! is no selector and one variant with no keys, so a source and a target
//! that differ in that respect are "another number of selectors".

use mf2_build::slice::selector_function;
use mf2_locale_data::plural::{Category, PluralKind, plural_rules};
use mf2_model::{
    CatchAllKey, Key, Literal, Message, OptionValue, Pattern, SelectMessage, VariableRef, Variant,
};

use crate::error::{Error, Result};

/// One unit of a message's group.
#[derive(Debug)]
pub(crate) struct Offer<'m> {
    /// The unit's keys.
    pub(crate) keys: Vec<Key<'static>>,
    /// The source variant MF2 would pick for those keys.
    pub(crate) source: &'m Pattern<'m>,
    /// The target's variant with those keys, if it has one.
    pub(crate) target: Option<&'m Pattern<'m>>,
}

/// What a message's group holds.
#[derive(Debug)]
pub(crate) struct Offers<'m> {
    pub(crate) units: Vec<Offer<'m>>,
    /// The target selects on another number of selectors than the source:
    /// only its own variants are offered, each against the source's
    /// catch-all.
    pub(crate) selects_differently: bool,
}

/// How one selector's column of keys is offered.
enum Column {
    /// By plural category: the target locale's categories of this kind.
    Plural(PluralKind),
    /// By the source's own keys.
    Keys,
}

/// The units of a message that selects on either side; `None` when neither
/// source nor target has `.match` (the message is one plain unit).
pub(crate) fn offers<'m>(
    source: &'m Message<'m>,
    target: Option<&'m Message<'m>>,
    locale: &str,
) -> Result<Option<Offers<'m>>> {
    let source_select = match source {
        Message::Select(s) => Some(s),
        Message::Pattern(_) => None,
    };
    let target_select = match target {
        Some(Message::Select(s)) => Some(s),
        _ => None,
    };
    if source_select.is_none() && target_select.is_none() {
        return Ok(None);
    }
    let source_variants = variants_of(source);
    let target_variants = target.map(variants_of);
    let arity = |m: &Message<'_>| match m {
        Message::Select(s) => s.selectors.len(),
        Message::Pattern(_) => 0,
    };
    if let Some(target) = target
        && arity(target) != arity(source)
    {
        let catch_all = catch_all(&source_variants);
        let units = target_variants
            .unwrap_or_default()
            .into_iter()
            .map(|(keys, pattern)| Offer {
                keys: keys.iter().cloned().map(Key::into_owned).collect(),
                source: catch_all,
                target: Some(pattern),
            })
            .collect();
        return Ok(Some(Offers {
            units,
            selects_differently: true,
        }));
    }
    // Same number of selectors, so the source has `.match`.
    let Some(select) = source_select else {
        return Ok(None);
    };
    let mut columns: Vec<Vec<Key<'static>>> = Vec::with_capacity(select.selectors.len());
    for (i, selector) in select.selectors.iter().enumerate() {
        columns.push(column(select, selector, i, locale)?);
    }
    let mut units: Vec<Offer<'m>> = Vec::new();
    for (keys, pattern) in target_variants.unwrap_or_default() {
        let keys: Vec<Key<'static>> = keys.iter().cloned().map(Key::into_owned).collect();
        units.push(Offer {
            source: pick(&source_variants, &keys),
            keys,
            target: Some(pattern),
        });
    }
    for keys in product(&columns) {
        if units.iter().any(|u| same_keys(&u.keys, &keys)) {
            continue;
        }
        units.push(Offer {
            source: pick(&source_variants, &keys),
            keys,
            target: None,
        });
    }
    Ok(Some(Offers {
        units,
        selects_differently: false,
    }))
}

/// A message's variants: a pattern message is one variant with no keys.
fn variants_of<'m>(message: &'m Message<'m>) -> Vec<(&'m [Key<'m>], &'m Pattern<'m>)> {
    match message {
        Message::Pattern(m) => vec![(&[][..], &m.pattern)],
        Message::Select(m) => m
            .variants
            .iter()
            .map(|v: &Variant<'m>| (v.keys.as_slice(), &v.value))
            .collect(),
    }
}

/// The source's all-`*` variant (MF2 requires one), or its last variant for
/// a model built otherwise.
fn catch_all<'m>(variants: &[(&'m [Key<'m>], &'m Pattern<'m>)]) -> &'m Pattern<'m> {
    variants
        .iter()
        .find(|(keys, _)| keys.iter().all(|k| matches!(k, Key::CatchAll(_))))
        .or(variants.last())
        .map_or(&EMPTY, |(_, p)| *p)
}

static EMPTY: Pattern<'static> = Pattern::new();

/// The keys offered for selector `i`.
fn column(
    select: &SelectMessage<'_>,
    selector: &VariableRef<'_>,
    i: usize,
    locale: &str,
) -> Result<Vec<Key<'static>>> {
    let source_keys = || {
        select
            .variants
            .iter()
            .filter_map(|v| v.keys.get(i))
            .map(|k| k.clone().into_owned())
    };
    let mut out: Vec<Key<'static>> = Vec::new();
    let add = |key: Key<'static>, out: &mut Vec<Key<'static>>| {
        if !out.iter().any(|k| same_key(k, &key)) {
            out.push(key);
        }
    };
    match kind(select, &selector.name) {
        Column::Keys => {
            for key in source_keys() {
                add(key, &mut out);
            }
        }
        Column::Plural(kind) => {
            // The source's exact-number keys first, in its order.
            for key in source_keys() {
                if let Key::Literal(l) = &key
                    && Category::from_name(&l.value).is_err()
                {
                    add(key, &mut out);
                }
            }
            let rules = plural_rules(kind, locale)
                .map_err(|e| Error::Usage(format!("plural rules of {locale}: {e}")))?;
            for category in Category::ALL {
                if category != Category::Other && rules.rules.iter().any(|r| r.category == category)
                {
                    add(
                        Key::Literal(Literal {
                            value: category.as_str().into(),
                        }),
                        &mut out,
                    );
                }
            }
        }
    }
    add(Key::CatchAll(CatchAllKey::default()), &mut out);
    Ok(out)
}

/// `:number` and `:integer` select by plural category — cardinal unless
/// `select=ordinal`, and not at all with `select=exact`. Any other selector
/// (`:string`, a custom function, a `select` given by a variable) offers the
/// source's keys.
fn kind(select: &SelectMessage<'_>, name: &str) -> Column {
    let Some(function) = selector_function(select, name, 0) else {
        return Column::Keys;
    };
    if function.name != "number" && function.name != "integer" {
        return Column::Keys;
    }
    match function.options.get("select") {
        None => Column::Plural(PluralKind::Cardinal),
        Some(OptionValue::Literal(l)) => match l.value.as_ref() {
            "ordinal" => Column::Plural(PluralKind::Ordinal),
            "plural" => Column::Plural(PluralKind::Cardinal),
            _ => Column::Keys,
        },
        Some(OptionValue::Variable(_)) => Column::Keys,
    }
}

/// Every combination of one key per column, the first column outermost.
fn product(columns: &[Vec<Key<'static>>]) -> Vec<Vec<Key<'static>>> {
    let mut out: Vec<Vec<Key<'static>>> = vec![Vec::new()];
    for column in columns {
        let mut next = Vec::with_capacity(out.len() * column.len());
        for prefix in &out {
            for key in column {
                let mut keys = prefix.clone();
                keys.push(key.clone());
                next.push(keys);
            }
        }
        out = next;
    }
    out
}

/// The source variant MF2 would pick for `keys`: among those whose every key
/// equals the unit's or is `*`, the one with a literal key at the first
/// selector where they differ — MF2 sorts by the first selector first.
fn pick<'m>(variants: &[(&'m [Key<'m>], &'m Pattern<'m>)], keys: &[Key<'_>]) -> &'m Pattern<'m> {
    let mut best: Option<(Vec<bool>, &'m Pattern<'m>)> = None;
    for (vkeys, pattern) in variants {
        if vkeys.len() != keys.len() {
            continue;
        }
        let fits = vkeys
            .iter()
            .zip(keys)
            .all(|(v, k)| matches!(v, Key::CatchAll(_)) || same_key(v, k));
        if !fits {
            continue;
        }
        let rank: Vec<bool> = vkeys.iter().map(|k| matches!(k, Key::Literal(_))).collect();
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, pattern));
        }
    }
    best.map_or_else(|| catch_all(variants), |(_, p)| p)
}

/// Keys compare as MF2 writes them: a literal by its value, `*` as `*`.
pub(crate) fn same_key(a: &Key<'_>, b: &Key<'_>) -> bool {
    match (a, b) {
        (Key::CatchAll(_), Key::CatchAll(_)) => true,
        (Key::Literal(x), Key::Literal(y)) => x.value == y.value,
        _ => false,
    }
}

pub(crate) fn same_keys(a: &[Key<'_>], b: &[Key<'_>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_key(x, y))
}

#[cfg(test)]
mod tests {
    use super::offers;
    use mf2_model::{Key, Message};

    fn parse(source: &str) -> Message<'_> {
        mf2_syntax::parse_model(source).message.expect("valid")
    }

    fn names(source: &str, target: Option<&str>, locale: &str) -> Vec<(String, String, bool)> {
        let s = parse(source);
        let t = target.map(parse);
        let offers = offers(&s, t.as_ref(), locale)
            .expect("rules")
            .expect("a group");
        offers
            .units
            .iter()
            .map(|u| {
                let keys: Vec<String> = u
                    .keys
                    .iter()
                    .map(|k| match k {
                        Key::CatchAll(_) => "*".to_owned(),
                        Key::Literal(l) => l.value.to_string(),
                    })
                    .collect();
                let text = u.source.as_simple_text().unwrap_or("…").to_owned();
                (keys.join(" "), text, u.target.is_some())
            })
            .collect()
    }

    const EN: &str = ".input {$n :integer}\n.match $n\n0 {{none}}\none {{one}}\n* {{many}}";

    /// Polish gets four forms; its `few` and `many` show English `*`; the
    /// exact key `0` stays first.
    #[test]
    fn a_new_polish_translation_gets_polish_forms() {
        assert_eq!(
            names(EN, None, "pl"),
            [
                ("0".into(), "none".into(), false),
                ("one".into(), "one".into(), false),
                ("few".into(), "many".into(), false),
                ("many".into(), "many".into(), false),
                ("*".into(), "many".into(), false),
            ]
        );
    }

    /// An existing translation's variants first, as they stand; then the
    /// forms it lacks.
    #[test]
    fn an_existing_translation_keeps_its_variants_first() {
        let pl = ".input {$n :integer}\n.match $n\none {{jeden}}\n* {{wiele}}";
        assert_eq!(
            names(EN, Some(pl), "pl"),
            [
                ("one".into(), "one".into(), true),
                ("*".into(), "many".into(), true),
                ("0".into(), "none".into(), false),
                ("few".into(), "many".into(), false),
                ("many".into(), "many".into(), false),
            ]
        );
    }

    /// Two selectors multiply; Arabic has six forms, so up to 36 units.
    #[test]
    fn two_counts_in_arabic_make_thirty_six_units() {
        let en =
            ".input {$a :number}\n.input {$b :number}\n.match $a $b\none one {{1 1}}\n* * {{n n}}";
        assert_eq!(names(en, None, "ar").len(), 36);
    }

    /// `:string` and `select=exact` offer the source's keys; ordinal rules
    /// are the target's ordinal ones.
    #[test]
    fn other_selectors_offer_the_source_keys() {
        let en = ".input {$g :string}\n.match $g\nmale {{he}}\n* {{they}}";
        let keys: Vec<String> = names(en, None, "pl").into_iter().map(|u| u.0).collect();
        assert_eq!(keys, ["male", "*"]);
        let en = ".input {$n :number select=exact}\n.match $n\n1 {{one}}\n* {{n}}";
        let keys: Vec<String> = names(en, None, "ar").into_iter().map(|u| u.0).collect();
        assert_eq!(keys, ["1", "*"]);
        let en = ".input {$n :number select=ordinal}\n.match $n\none {{st}}\ntwo {{nd}}\nfew {{rd}}\n* {{th}}";
        let keys: Vec<String> = names(en, None, "pl").into_iter().map(|u| u.0).collect();
        assert_eq!(keys, ["*"], "Polish ordinals are all `other`");
    }

    /// A target that selects on another number of selectors shows only its
    /// own variants, each against the source's catch-all.
    #[test]
    fn a_target_that_selects_differently_keeps_its_own_variants() {
        let s = parse(EN);
        let t = parse("{{wiele}}");
        let o = offers(&s, Some(&t), "pl").expect("rules").expect("a group");
        assert!(o.selects_differently);
        assert_eq!(o.units.len(), 1);
        assert_eq!(o.units[0].source.as_simple_text(), Some("many"));
        assert!(o.units[0].keys.is_empty());
        // Both plain: one unit, no group.
        let plain = parse("hi");
        assert!(offers(&plain, None, "pl").expect("rules").is_none());
    }
}
