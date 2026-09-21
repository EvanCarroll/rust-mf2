//! An independent reference implementation used to cross-check the client
//! path: operands computed by plain string manipulation, relations evaluated
//! over the AST with exact rational arithmetic (`n = N / 10^v`, and
//! `n % m = (N mod m·10^v) / 10^v`), no shortcuts shared with `plural-eval`.

use plural_eval::Category;

use crate::rule::{Operand, Relation, Rule};

/// Operands of a sample string, as wide integers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefOperands {
    /// All visible digits of n after the exponent shift, as an integer (N).
    pub scaled: u128,
    pub i: u128,
    pub v: u32,
    pub w: u32,
    pub f: u128,
    pub t: u128,
    pub e: u32,
}

/// Operands of `digit+ ('.' digit+)? ([ce] digit+)?`.
pub fn operands(sample: &str) -> Option<RefOperands> {
    let (mantissa, e) = match sample.split_once(['c', 'e']) {
        Some((m, x)) => (m, x.parse::<u32>().ok()?),
        None => (sample, 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    // Move the decimal point e places right, padding with zeros.
    let mut all = format!("{int}{frac}");
    let point = int.len() + e as usize;
    while all.len() < point {
        all.push('0');
    }
    let (int, frac) = all.split_at(point);
    let trimmed = frac.trim_end_matches('0');
    let num = |s: &str| if s.is_empty() { Some(0) } else { s.parse::<u128>().ok() };
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

fn relation(r: &Relation, o: &RefOperands) -> bool {
    let value: Option<u128> = match r.operand {
        Operand::N => {
            let unit = 10u128.pow(o.v);
            let rem = match r.modulus {
                Some(m) => o.scaled % (u128::from(m) * unit),
                None => o.scaled,
            };
            // n (or n % m) is integral iff the remainder has no fraction part.
            (rem % unit == 0).then_some(rem / unit)
        }
        other => {
            let x = match other {
                Operand::I => o.i,
                Operand::V => u128::from(o.v),
                Operand::W => u128::from(o.w),
                Operand::F => o.f,
                Operand::T => o.t,
                Operand::C => u128::from(o.e),
                Operand::N => unreachable!(),
            };
            Some(match r.modulus {
                Some(m) => x % u128::from(m),
                None => x,
            })
        }
    };
    let hit = value.is_some_and(|x| {
        r.items.iter().any(|&(lo, hi)| u128::from(lo) <= x && x <= u128::from(hi))
    });
    hit != r.negated
}

/// Whether the rule's condition holds (`other` never "holds": it is the fallback).
pub fn holds(rule: &Rule, o: &RefOperands) -> bool {
    !rule.condition.is_empty()
        && rule.condition.iter().any(|group| group.iter().all(|r| relation(r, o)))
}

/// First holding rule's category, else `other`.
pub fn select(rules: &[Rule], o: &RefOperands) -> Category {
    rules.iter().find(|r| holds(r, o)).map_or(Category::Other, |r| r.category)
}
