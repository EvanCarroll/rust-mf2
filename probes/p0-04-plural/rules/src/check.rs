//! The P0.4 correctness run: every `@integer` / `@decimal` sample of every
//! category of every locale, cardinal and ordinal, through
//! parse → encode → `plural_eval::select`, cross-checked against the
//! independent reference implementation, plus the exclusivity check (each
//! sample is claimed by exactly its own category, implicit `other` included).

use std::collections::BTreeSet;

use plural_eval::{Category, Operands, select};

use crate::cldr::{Cldr, Kind};
use crate::encode::{encode, encode_rule};
use crate::reference;

pub struct Failure {
    pub kind: Kind,
    pub locale: String,
    pub category: Category,
    pub sample: String,
    pub cause: String,
}

#[derive(Default, Clone, Copy)]
pub struct Counts {
    pub locales: usize,
    /// Rules with a condition (zero/one/two/few/many).
    pub explicit_rules: usize,
    /// `other` rules present in the JSON.
    pub other_rules: usize,
    /// Sample list items as written (a range counts once).
    pub listed_items: usize,
    pub ranges: usize,
    pub integer_samples: usize,
    pub decimal_samples: usize,
    /// Samples using the compact-exponent form (`1c6`, `1.1c6`).
    pub exponent_samples: usize,
}

impl Counts {
    pub fn samples(&self) -> usize {
        self.integer_samples + self.decimal_samples
    }
}

pub struct Report {
    pub cardinal: Counts,
    pub ordinal: Counts,
    /// Individual assertions made (5 per sample: operands, byte select,
    /// reference select, byte exclusivity, reference exclusivity).
    pub assertions: usize,
    pub failures: Vec<Failure>,
    /// Samples the audit's `hand` evaluator would get wrong: it treated `c`/`e`
    /// as 0. (Informational — `hand` is replaced by `plural-eval`.)
    pub hand_semantics_failures: Vec<(Kind, String, String)>,
    /// Categories that carry no sample at all (none expected in CLDR 48).
    pub unsampled: Vec<(Kind, String, &'static str)>,
}

fn names(set: &BTreeSet<u8>) -> String {
    if set.is_empty() {
        return "other".into();
    }
    set.iter().map(|&c| Category::from_code(c).as_str()).collect::<Vec<_>>().join("+")
}

pub fn run(cldr: &Cldr) -> Report {
    let mut report = Report {
        cardinal: Counts::default(),
        ordinal: Counts::default(),
        assertions: 0,
        failures: Vec::new(),
        hand_semantics_failures: Vec::new(),
        unsampled: Vec::new(),
    };
    for kind in Kind::ALL {
        let mut counts = Counts::default();
        for (locale, lr) in cldr.get(kind) {
            counts.locales += 1;
            let full = encode(&lr.rules);
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
            for rule in &lr.rules {
                if rule.category == Category::Other {
                    counts.other_rules += 1;
                } else {
                    counts.explicit_rules += 1;
                }
                if rule.integer.is_none() && rule.decimal.is_none() {
                    report.unsampled.push((kind, locale.clone(), rule.category.as_str()));
                }
                let expected: BTreeSet<u8> = if rule.category == Category::Other {
                    BTreeSet::new()
                } else {
                    BTreeSet::from([rule.category as u8])
                };
                for (is_integer, list) in [(true, &rule.integer), (false, &rule.decimal)] {
                    let Some(list) = list else { continue };
                    counts.listed_items += list.items.len();
                    counts.ranges += list
                        .items
                        .iter()
                        .filter(|i| matches!(i, crate::samples::SampleItem::Range(..)))
                        .count();
                    let samples = match list.expand() {
                        Ok(s) => s,
                        Err(e) => {
                            report.failures.push(Failure {
                                kind,
                                locale: locale.clone(),
                                category: rule.category,
                                sample: String::new(),
                                cause: format!("sample expansion: {e}"),
                            });
                            continue;
                        }
                    };
                    for sample in samples {
                        if is_integer {
                            counts.integer_samples += 1;
                        } else {
                            counts.decimal_samples += 1;
                        }
                        if sample.contains(['c', 'e']) {
                            counts.exponent_samples += 1;
                        }
                        let mut fail = |cause: String| {
                            report.failures.push(Failure {
                                kind,
                                locale: locale.clone(),
                                category: rule.category,
                                sample: sample.clone(),
                                cause,
                            });
                        };
                        report.assertions += 5;
                        let (Some(ops), Some(r)) = (Operands::parse(sample.as_bytes()), reference::operands(&sample))
                        else {
                            fail("operand extraction rejected the sample".into());
                            continue;
                        };
                        let same = u128::from(ops.i) == r.i
                            && ops.v == r.v
                            && ops.w == r.w
                            && u128::from(ops.f) == r.f
                            && u128::from(ops.t) == r.t
                            && ops.e == r.e;
                        if !same {
                            fail(format!("operands differ: client i={} v={} w={} f={} t={} e={}, reference {r:?}", ops.i, ops.v, ops.w, ops.f, ops.t, ops.e));
                        }
                        let got = select(&full, &ops);
                        if got != rule.category {
                            fail(format!("encoded rules select `{}`", got.as_str()));
                        }
                        let got_ref = reference::select(&lr.rules, &r);
                        if got_ref != rule.category {
                            fail(format!("reference evaluator selects `{}`", got_ref.as_str()));
                        }
                        let claimed: BTreeSet<u8> = singles
                            .iter()
                            .filter(|(c, bytes)| select(bytes, &ops) == *c)
                            .map(|(c, _)| *c as u8)
                            .collect();
                        if claimed != expected {
                            fail(format!("claimed by {} (encoded, rule by rule)", names(&claimed)));
                        }
                        let claimed_ref: BTreeSet<u8> = lr
                            .rules
                            .iter()
                            .filter(|rule| reference::holds(rule, &r))
                            .map(|rule| rule.category as u8)
                            .collect();
                        if claimed_ref != expected {
                            fail(format!("claimed by {} (reference, rule by rule)", names(&claimed_ref)));
                        }
                        let hand = Operands { e: 0, ..ops };
                        if select(&full, &hand) != rule.category {
                            report.hand_semantics_failures.push((kind, locale.clone(), sample.clone()));
                        }
                    }
                }
            }
        }
        match kind {
            Kind::Cardinal => report.cardinal = counts,
            Kind::Ordinal => report.ordinal = counts,
        }
    }
    report
}
