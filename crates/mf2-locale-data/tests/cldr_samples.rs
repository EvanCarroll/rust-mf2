//! Every CLDR `@integer`/`@decimal` sample of every locale, cardinal and
//! ordinal, through the shipped table → the encoder → the **client
//! evaluator** (`mf2_runtime::plural_category`), cross-checked against an
//! independent reference evaluator (exact rational arithmetic over the parsed
//! rules), and exclusive: each sample is claimed by exactly its own category.
//! P0.4's threshold: 15,041 / 15,041 (plans/10 A6).

use std::collections::BTreeSet;

use mf2_locale_data::plural::{Category, Operand, Relation, Rule, encode, encode_rule};
use mf2_locale_data::{PluralKind, plural_entry, plural_locales, plural_rules};
use mf2_runtime::{Operands, plural_category};

/// Operands of a sample by plain string arithmetic (no code shared with the
/// runtime).
#[derive(Debug, PartialEq, Eq)]
struct RefOperands {
    scaled: u128,
    i: u128,
    v: u32,
    w: u32,
    f: u128,
    t: u128,
    e: u32,
}

fn ref_operands(sample: &str) -> Option<RefOperands> {
    let (mantissa, e) = match sample.split_once(['c', 'e']) {
        Some((m, x)) => (m, x.parse::<u32>().ok()?),
        None => (sample, 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut all = format!("{int}{frac}");
    let point = int.len() + e as usize;
    while all.len() < point {
        all.push('0');
    }
    let (int, frac) = all.split_at(point);
    let trimmed = frac.trim_end_matches('0');
    let num = |s: &str| {
        if s.is_empty() {
            Some(0)
        } else {
            s.parse::<u128>().ok()
        }
    };
    Some(RefOperands {
        scaled: num(&all)?,
        i: num(int)?,
        v: u32::try_from(frac.len()).ok()?,
        w: u32::try_from(trimmed.len()).ok()?,
        f: num(frac)?,
        t: num(trimmed)?,
        e,
    })
}

fn ref_relation(r: &Relation, o: &RefOperands) -> bool {
    let value = match r.operand {
        Operand::N => {
            let unit = 10u128.pow(o.v);
            let rem = match r.modulus {
                Some(m) => o.scaled % (u128::from(m) * unit),
                None => o.scaled,
            };
            (rem % unit == 0).then_some(rem / unit)
        }
        other => {
            let x = match other {
                Operand::I => o.i,
                Operand::V => u128::from(o.v),
                Operand::W => u128::from(o.w),
                Operand::F => o.f,
                Operand::T => o.t,
                _ => u128::from(o.e),
            };
            Some(match r.modulus {
                Some(m) => x % u128::from(m),
                None => x,
            })
        }
    };
    let hit = value.is_some_and(|x| {
        r.items
            .iter()
            .any(|&(lo, hi)| u128::from(lo) <= x && x <= u128::from(hi))
    });
    hit != r.negated
}

fn ref_holds(rule: &Rule, o: &RefOperands) -> bool {
    !rule.condition.is_empty()
        && rule
            .condition
            .iter()
            .any(|group| group.iter().all(|r| ref_relation(r, o)))
}

fn ref_select(rules: &[Rule], o: &RefOperands) -> Category {
    rules
        .iter()
        .find(|r| ref_holds(r, o))
        .map_or(Category::Other, |r| r.category)
}

fn runtime_name(c: mf2_runtime::Category) -> &'static str {
    c.as_str()
}

#[test]
fn every_cldr_sample_passes_the_client_evaluator() {
    let mut failures = Vec::new();
    let mut samples = [0usize; 2];
    let mut locales = [0usize; 2];
    for (k, kind) in PluralKind::ALL.into_iter().enumerate() {
        for locale in plural_locales(kind).expect("table") {
            locales[k] += 1;
            let lr = plural_rules(kind, locale).expect("rules");
            assert_eq!(lr.locale, locale);
            let full = plural_entry(kind, locale).expect("entry");
            assert_eq!(full, encode(lr.rules));
            let singles: Vec<(Category, Vec<u8>)> = lr
                .rules
                .iter()
                .filter(|r| r.category != Category::Other)
                .map(|r| {
                    let mut b = Vec::new();
                    encode_rule(r, &mut b);
                    (r.category, b)
                })
                .collect();
            for rule in lr.rules {
                assert!(
                    rule.integer.is_some() || rule.decimal.is_some(),
                    "{locale} {} has no samples",
                    rule.category.as_str()
                );
                let expected: BTreeSet<&str> = if rule.category == Category::Other {
                    BTreeSet::new()
                } else {
                    BTreeSet::from([rule.category.as_str()])
                };
                for list in [&rule.integer, &rule.decimal].into_iter().flatten() {
                    for sample in list.expand().expect("expands") {
                        samples[k] += 1;
                        let mut fail = |cause: String| {
                            failures.push(format!(
                                "{} {locale} {} {sample:?}: {cause}",
                                kind.name(),
                                rule.category.as_str()
                            ));
                        };
                        let (Some(ops), Some(r)) =
                            (Operands::parse(&sample), ref_operands(&sample))
                        else {
                            fail("operands rejected".into());
                            continue;
                        };
                        let same = u128::from(ops.i) == r.i
                            && ops.v == r.v
                            && ops.w == r.w
                            && u128::from(ops.f) == r.f
                            && u128::from(ops.t) == r.t
                            && ops.e == r.e;
                        if !same {
                            fail(format!("operands {ops:?} vs reference {r:?}"));
                        }
                        let got = runtime_name(plural_category(&full, &ops));
                        if got != rule.category.as_str() {
                            fail(format!("client evaluator selects {got}"));
                        }
                        let got = ref_select(lr.rules, &r);
                        if got != rule.category {
                            fail(format!("reference selects {}", got.as_str()));
                        }
                        let claimed: BTreeSet<&str> = singles
                            .iter()
                            .filter(|(c, b)| runtime_name(plural_category(b, &ops)) == c.as_str())
                            .map(|(c, _)| c.as_str())
                            .collect();
                        if claimed != expected {
                            fail(format!("claimed by {claimed:?} (client, rule by rule)"));
                        }
                        let claimed: BTreeSet<&str> = lr
                            .rules
                            .iter()
                            .filter(|rule| ref_holds(rule, &r))
                            .map(|rule| rule.category.as_str())
                            .collect();
                        if claimed != expected {
                            fail(format!("claimed by {claimed:?} (reference, rule by rule)"));
                        }
                    }
                }
            }
        }
    }
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "{} failures", failures.len());
    assert_eq!(locales, [224, 108]);
    assert_eq!(samples, [12_396, 2_645], "cardinal, ordinal samples");
    eprintln!(
        "CLDR samples: {} cardinal + {} ordinal = {} passed",
        samples[0],
        samples[1],
        samples[0] + samples[1]
    );
}

#[test]
fn lookup_truncates_then_falls_back_to_root() {
    let c = PluralKind::Cardinal;
    assert_eq!(plural_rules(c, "en").expect("en").locale, "en");
    assert_eq!(plural_rules(c, "en-US").expect("en-US").locale, "en");
    assert_eq!(plural_rules(c, "EN_us").expect("EN_us").locale, "en");
    assert_eq!(plural_rules(c, "pt-PT").expect("pt-PT").locale, "pt-PT");
    assert_eq!(plural_rules(c, "pt-BR").expect("pt-BR").locale, "pt");
    assert_eq!(
        plural_rules(c, "kok-Latn-IN").expect("kok").locale,
        "kok-Latn"
    );
    assert_eq!(plural_rules(c, "und").expect("und").locale, "und");
    assert_eq!(plural_rules(c, "xx-YY").expect("xx").locale, "und");
    assert!(plural_entry(c, "xx").expect("xx").is_empty());
    assert!(plural_entry(c, "ja").expect("ja").is_empty());
    assert_eq!(
        plural_entry(c, "en").expect("en"),
        [0x21, 0x01, 0x05, 0x82, 0x01]
    );
    // Ordinals: 116 of 224 locales have none (truncation, then root).
    assert!(
        plural_entry(PluralKind::Ordinal, "de")
            .expect("de")
            .is_empty()
    );
    assert!(
        !plural_entry(PluralKind::Ordinal, "en-GB")
            .expect("en-GB")
            .is_empty()
    );
}
