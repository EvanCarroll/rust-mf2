//! UTS #35 plural-rule samples (`@integer …`, `@decimal …`) and their
//! expansion (ported from P0.4):
//!
//! * a single value is one sample, kept verbatim (`1.0` and `1` differ: the
//!   visible fraction digits are operands);
//! * `x~y` is every value from `x` to `y` in steps of one unit in the last
//!   visible fraction digit; both ends have the same fraction digits and no
//!   exponent (`0.0~1.5` is 0.0, 0.1, …, 1.5);
//! * a trailing `…` (or `...`) only says the list is open.

use crate::error::ParseError;

/// One item of a sample list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleItem {
    Value(String),
    Range(String, String),
}

/// A sample list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleList {
    pub items: Vec<SampleItem>,
    /// The list ended in `…`.
    pub open: bool,
}

impl SampleList {
    /// Every sample, ranges expanded.
    pub fn expand(&self) -> Result<Vec<String>, ParseError> {
        let mut out = Vec::new();
        for item in &self.items {
            match item {
                SampleItem::Value(v) => out.push(v.clone()),
                SampleItem::Range(a, b) => expand_range(a, b, &mut out)?,
            }
        }
        Ok(out)
    }
}

/// `sample_value = digit+ ('.' digit+)? ([ce] digit+)?`
pub fn is_sample_value(s: &str) -> bool {
    let (mantissa, exp) = match s.find(['c', 'e']) {
        Some(p) => (&s[..p], Some(&s[p + 1..])),
        None => (s, None),
    };
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let (int, frac) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    digits(int) && frac.is_none_or(digits) && exp.is_none_or(digits)
}

fn parse_list(text: &str) -> Result<SampleList, ParseError> {
    let mut items = Vec::new();
    let mut open = false;
    for raw in text.split(',') {
        let item = raw.trim();
        if open {
            return Err(ParseError::Sample(item.to_owned()));
        }
        if item == "…" || item == "..." {
            open = true;
            continue;
        }
        let parsed = match item.split_once('~') {
            Some((a, b)) if is_sample_value(a) && is_sample_value(b) => {
                SampleItem::Range(a.to_owned(), b.to_owned())
            }
            None if is_sample_value(item) => SampleItem::Value(item.to_owned()),
            _ => return Err(ParseError::Sample(item.to_owned())),
        };
        items.push(parsed);
    }
    if items.is_empty() {
        return Err(ParseError::Sample(text.to_owned()));
    }
    Ok(SampleList { items, open })
}

/// Parses the samples of a rule string (everything from its first `@`).
pub fn parse_samples(text: &str) -> Result<(Option<SampleList>, Option<SampleList>), ParseError> {
    let text = text.trim();
    let mut integer = None;
    let mut decimal = None;
    let mut rest = text;
    if let Some(r) = rest.strip_prefix("@integer") {
        let end = r.find("@decimal").unwrap_or(r.len());
        integer = Some(parse_list(&r[..end])?);
        rest = r[end..].trim_start();
    }
    if let Some(r) = rest.strip_prefix("@decimal") {
        decimal = Some(parse_list(r)?);
        rest = "";
    }
    if rest.is_empty() {
        Ok((integer, decimal))
    } else {
        Err(ParseError::Sample(rest.to_owned()))
    }
}

fn split_scaled(s: &str) -> Option<(u128, usize)> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let digits: String = [int, frac].concat();
    Some((digits.parse().ok()?, frac.len()))
}

fn expand_range(a: &str, b: &str, out: &mut Vec<String>) -> Result<(), ParseError> {
    let bad = || ParseError::SampleRange(format!("{a}~{b}"));
    if a.contains(['c', 'e']) || b.contains(['c', 'e']) {
        return Err(bad());
    }
    let (lo, va) = split_scaled(a).ok_or_else(bad)?;
    let (hi, vb) = split_scaled(b).ok_or_else(bad)?;
    if va != vb || hi < lo {
        return Err(bad());
    }
    for x in lo..=hi {
        let digits = format!("{x:0>width$}", width = va + 1);
        let (int, frac) = digits.split_at(digits.len() - va);
        out.push(if va == 0 {
            int.to_owned()
        } else {
            format!("{int}.{frac}")
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_samples;

    #[test]
    fn expands_ranges() {
        let (i, d) =
            parse_samples("@integer 0, 2~4, 1c6, … @decimal 0.0~0.2, 0.00~0.02, 1.0000001c6, …")
                .expect("parses");
        assert_eq!(
            i.expect("integer").expand().expect("expands"),
            ["0", "2", "3", "4", "1c6"]
        );
        assert_eq!(
            d.expect("decimal").expand().expect("expands"),
            ["0.0", "0.1", "0.2", "0.00", "0.01", "0.02", "1.0000001c6"]
        );
        let (_, d) = parse_samples("@decimal 0.9~1.1").expect("parses");
        assert_eq!(
            d.expect("decimal").expand().expect("expands"),
            ["0.9", "1.0", "1.1"]
        );
    }

    #[test]
    fn rejects() {
        assert!(parse_samples("@integer").is_err());
        assert!(parse_samples("@integer 1, …, 2").is_err());
        assert!(parse_samples("@integer 1x").is_err());
        assert!(parse_samples("@frac 1").is_err());
        let (_, d) = parse_samples("@decimal 0.0~1").expect("parses");
        assert!(d.expect("decimal").expand().is_err());
    }
}
