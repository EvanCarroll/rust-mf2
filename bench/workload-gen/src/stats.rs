//! Shape statistics, measured on the generated output (not on the plan), and
//! the tolerance check of plans/06 §2: every percentage within ± 1 percentage
//! point, mean text length within ± 5 %.

use std::collections::BTreeSet;

use crate::model::Workload;
use crate::output::Files;
use crate::shape::target;
use crate::sites::{ARG_SITES, Mode, Shape, SitePlan};

/// One line of the report.
#[derive(Debug, Clone)]
pub struct Row {
    /// What is measured.
    pub label: String,
    /// The plan's value, if it states one.
    pub target: String,
    /// Measured value.
    pub actual: String,
    /// `Some(pass)` when the row is checked.
    pub ok: Option<bool>,
}

/// A statistics report.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Rows in order.
    pub rows: Vec<Row>,
}

impl Report {
    fn info(
        &mut self,
        label: impl Into<String>,
        target: impl Into<String>,
        actual: impl Into<String>,
    ) {
        self.rows.push(Row {
            label: label.into(),
            target: target.into(),
            actual: actual.into(),
            ok: None,
        });
    }

    fn check(
        &mut self,
        label: impl Into<String>,
        target: impl Into<String>,
        actual: impl Into<String>,
        ok: bool,
    ) {
        self.rows.push(Row {
            label: label.into(),
            target: target.into(),
            actual: actual.into(),
            ok: Some(ok),
        });
    }

    /// A percentage row checked at ± 1 percentage point.
    fn pct(&mut self, label: &str, target_pct: f64, count: usize, total: usize) {
        let actual = percent(count, total);
        let ok = (actual - target_pct).abs() <= target::PCT_TOLERANCE + 1e-9;
        self.check(
            label,
            format!("{target_pct:.1} % ±1 pp"),
            format!("{actual:.2} % ({count})"),
            ok,
        );
    }

    /// Whether every checked row passed.
    pub fn passed(&self) -> bool {
        self.rows.iter().all(|r| r.ok != Some(false))
    }

    /// Labels of the failed rows.
    pub fn failures(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.ok == Some(false))
            .map(|r| format!("{}: {} (target {})", r.label, r.actual, r.target))
            .collect()
    }

    /// A fixed-width text table.
    pub fn render(&self) -> String {
        let w0 = self
            .rows
            .iter()
            .map(|r| r.label.chars().count())
            .max()
            .unwrap_or(0);
        let w1 = self
            .rows
            .iter()
            .map(|r| r.target.chars().count())
            .max()
            .unwrap_or(0);
        let w2 = self
            .rows
            .iter()
            .map(|r| r.actual.chars().count())
            .max()
            .unwrap_or(0);
        let mut out = String::new();
        for r in &self.rows {
            let status = match r.ok {
                Some(true) => "ok",
                Some(false) => "FAIL",
                None => "",
            };
            let line = format!(
                "{:w0$}  {:w1$}  {:w2$}  {status}",
                r.label, r.target, r.actual
            );
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

#[allow(clippy::cast_precision_loss)] // counts far below 2^52
fn percent(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * count as f64 / total as f64
    }
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: &[usize]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<usize>() as f64 / values.len() as f64
    }
}

/// Classification of one message source.
#[derive(Debug, Clone, Default)]
pub struct Class {
    /// Distinct external variable names.
    pub vars: usize,
    /// `.match` message.
    pub select: bool,
    /// Uses markup.
    pub markup: bool,
    /// No `{` at all and not a complex message: the parser gate's
    /// placeholder-free subset.
    pub placeholder_free: bool,
}

/// Classifies MF2 source by scanning (enough for generated messages).
pub fn classify(src: &str) -> Class {
    let complex = src.trim_start().starts_with('.');
    let mut names = BTreeSet::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'-' | b'.'))
            {
                end += 1;
            }
            if end > start {
                names.insert(&src[start..end]);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Class {
        vars: names.len(),
        select: complex && src.contains(".match"),
        markup: src.contains("{#") || src.contains("{/"),
        placeholder_free: !complex && !src.contains('{'),
    }
}

/// Whether `id` is dotted kebab-case: parts of `[a-z0-9-]`, no empty part,
/// no leading/trailing `-`.
pub fn is_kebab_id(id: &str) -> bool {
    id.split('.').all(|part| {
        !part.is_empty()
            && !part.starts_with('-')
            && !part.ends_with('-')
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

/// Rows for a flat corpus (`id → source`, any order).
pub fn corpus(report: &mut Report, messages: &[(String, String)]) {
    let n = messages.len();
    let classes: Vec<Class> = messages.iter().map(|(_, s)| classify(s)).collect();
    let by_vars = |k: usize| classes.iter().filter(|c| c.vars == k).count();
    report.info("messages", target::MESSAGES.to_string(), n.to_string());
    report.pct("no variable (simple)", 79.0, by_vars(0), n);
    report.info(
        "  of which placeholder-free (parser-gate subset)",
        "",
        format!(
            "{:.2} % ({})",
            percent(classes.iter().filter(|c| c.placeholder_free).count(), n),
            classes.iter().filter(|c| c.placeholder_free).count()
        ),
    );
    report.info(
        "  of which with inline markup",
        "a few",
        classes.iter().filter(|c| c.markup).count().to_string(),
    );
    report.pct("1 variable", 14.7, by_vars(1), n);
    report.pct(
        "  of which .match plural (share of all)",
        0.9,
        classes.iter().filter(|c| c.select).count(),
        n,
    );
    report.pct("2 variables", 5.0, by_vars(2), n);
    report.pct("3 variables", 1.1, by_vars(3), n);
    report.pct("4 variables", 0.2, by_vars(4), n);
    report.info(
        "5+ variables",
        "0",
        classes.iter().filter(|c| c.vars > 4).count().to_string(),
    );

    let mut lens: Vec<usize> = messages.iter().map(|(_, s)| s.len()).collect();
    lens.sort_unstable();
    let m = mean(&lens);
    report.check(
        "text mean (bytes of source)",
        format!("{:.1} ±5 %", target::MEAN_LEN),
        format!("{m:.2}"),
        (m - target::MEAN_LEN).abs() <= target::MEAN_LEN * target::MEAN_TOLERANCE,
    );
    if !lens.is_empty() {
        let median = if lens.len().is_multiple_of(2) {
            mean(&lens[lens.len() / 2 - 1..=lens.len() / 2])
        } else {
            mean(&lens[lens.len() / 2..=lens.len() / 2])
        };
        let p90 = lens[(9 * lens.len()).div_ceil(10) - 1];
        report.info(
            "text median",
            format!("{:.0}", target::MEDIAN_LEN),
            format!("{median:.1}"),
        );
        report.info(
            "text p90 (nearest rank)",
            target::P90_LEN.to_string(),
            p90.to_string(),
        );
        report.info(
            "text max",
            target::MAX_LEN.to_string(),
            lens.last().copied().unwrap_or(0).to_string(),
        );
    }
    #[allow(clippy::cast_precision_loss)]
    let total_kb = lens.iter().sum::<usize>() as f64 / 1000.0;
    report.info("text total", "≈ 43 KB", format!("{total_kb:.1} KB"));

    let ids: Vec<usize> = messages.iter().map(|(id, _)| id.chars().count()).collect();
    let id_mean = mean(&ids);
    report.check(
        "id mean (chars, full dotted id)",
        format!("{:.1} ±5 %", target::ID_MEAN),
        format!("{id_mean:.2}"),
        (id_mean - target::ID_MEAN).abs() <= target::ID_MEAN * 0.05,
    );
    let kebab = messages.iter().filter(|(id, _)| is_kebab_id(id)).count();
    report.check(
        "ids dotted kebab-case",
        n.to_string(),
        kebab.to_string(),
        kebab == n,
    );
}

/// Rows for the `.mf2` files: file count and comment share per locale.
pub fn resources(report: &mut Report, files: &Files, tags: &[&str], expected_files: usize) {
    for tag in tags {
        let prefix = format!("locales/{tag}/");
        let mut count = 0usize;
        let (mut comment, mut total) = (0usize, 0usize);
        for (path, bytes) in files.iter() {
            if !path.starts_with(&prefix) {
                continue;
            }
            count += 1;
            total += bytes.len();
            for line in bytes.split_inclusive(|&b| b == b'\n') {
                if line.first() == Some(&b'#') {
                    comment += line.len();
                }
            }
        }
        report.check(
            format!("source files ({tag})"),
            expected_files.to_string(),
            count.to_string(),
            count == expected_files,
        );
        let share = percent(comment, total);
        let label = format!("comment share of source bytes ({tag})");
        let target_pct = target::COMMENT_SHARE * 100.0;
        if *tag == "en" {
            report.check(
                label,
                format!("{target_pct:.0} % ±1 pp"),
                format!("{share:.2} %"),
                (share - target_pct).abs() <= target::PCT_TOLERANCE,
            );
        } else {
            report.info(
                label,
                format!("≈ {target_pct:.0} %"),
                format!("{share:.2} %"),
            );
        }
    }
}

/// Rows for the call-site plan.
pub fn sites(report: &mut Report, wl: &Workload, plan: &SitePlan) {
    let m = plan.sites.len();
    report.info("call sites", target::SITES.to_string(), m.to_string());
    let count = |f: &dyn Fn(Shape) -> bool| plan.sites.iter().filter(|s| f(s.shape)).count();
    for (label, pct, shapes) in [
        ("  non-view String", 45.0, &[Shape::String][..]),
        ("  text child", 20.0, &[Shape::Child][..]),
        ("  HTML attribute", 8.0, &[Shape::Attr][..]),
        (
            "  reactive-text prop (TextProp + Signal<String>)",
            8.0,
            &[Shape::TextProp, Shape::SignalProp][..],
        ),
        ("  String prop", 4.0, &[Shape::StringProp][..]),
        (
            "  deferred label (static table)",
            8.0,
            &[Shape::Deferred][..],
        ),
        ("  if/else branch", 7.0, &[Shape::IfElse][..]),
    ] {
        report.pct(label, pct, count(&|s| shapes.contains(&s)), m);
    }
    let with_args: Vec<_> = plan.sites.iter().filter(|s| s.mode.has_args()).collect();
    #[allow(clippy::cast_precision_loss)]
    let arg_target = ARG_SITES as f64 / 10_000.0;
    report.pct("sites passing arguments", arg_target, with_args.len(), m);
    let by_mode = |mode: Mode| with_args.iter().filter(|s| s.mode == mode).count();
    report.info(
        "  plain / signal argument / enclosing move || .get()",
        "",
        format!(
            "{} / {} / {}",
            by_mode(Mode::Plain),
            by_mode(Mode::Signal),
            by_mode(Mode::Get)
        ),
    );
    let by_n = |k: usize| {
        with_args
            .iter()
            .filter(|s| wl.messages[s.message].vars.len() == k)
            .count()
    };
    report.info(
        "  with 1 / 2 / 3 arguments",
        "1–3",
        format!("{} / {} / {}", by_n(1), by_n(2), by_n(3)),
    );
    let referenced: BTreeSet<usize> = plan.sites.iter().map(|s| s.message).collect();
    report.info(
        "messages referenced by a site",
        "",
        format!("{} of {}", referenced.len(), wl.messages.len()),
    );
    report.info(
        "components / lazy routes",
        "",
        format!("{} / {}", plan.components, plan.routes),
    );
}
