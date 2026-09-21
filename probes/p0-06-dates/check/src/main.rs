//! Native checks for P0.6. Run from the probe root after `scripts/blobs.sh`.
use mf2dt::{ErrSink, Error, Func, OptValue, Overrides, TimeZoneOpt};

#[derive(Default)]
struct Errs(Vec<&'static str>);
impl ErrSink for Errs {
    fn push(&mut self, e: Error) {
        self.0.push(match e {
            Error::BadOperand => "bad-operand",
            Error::BadOption => "bad-option",
            Error::UnsupportedOperation => "unsupported-operation",
        });
    }
}

fn opts(s: &str) -> Vec<(&str, OptValue<'_>)> {
    s.split_whitespace()
        .filter_map(|t| t.split_once('='))
        .map(|(k, v)| match v.strip_prefix('$') {
            Some(x) => (k, OptValue::Variable(x)),
            None => (k, OptValue::Literal(v)),
        })
        .collect()
}

/// One suite case, hand-lowered: an optional `.local` producing an operand
/// (function + options) and the placeholder's function + options.
struct Case {
    src: &'static str,
    operand: Option<&'static str>,
    local: Option<(Func, &'static str)>,
    func: Func,
    opts: &'static str,
    exp_errors: &'static [&'static str],
}

const CASES: &[Case] = &[
    // functions/date.json
    Case { src: "{:date}", operand: None, local: None, func: Func::Date, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{horse :date}", operand: Some("horse"), local: None, func: Func::Date, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{|2006-01-02| :date}", operand: Some("2006-01-02"), local: None, func: Func::Date, opts: "", exp_errors: &[] },
    Case { src: "{|2006-01-02T15:04:06| :date}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::Date, opts: "", exp_errors: &[] },
    Case { src: "{|2006-01-02| :date length=long}", operand: Some("2006-01-02"), local: None, func: Func::Date, opts: "length=long", exp_errors: &[] },
    Case { src: ".local $d = {|2006-01-02| :date length=long} {{{$d}}}", operand: Some("2006-01-02"), local: None, func: Func::Date, opts: "length=long", exp_errors: &[] },
    Case { src: ".local $d = {|2006-01-02| :datetime dateLength=long timePrecision=second} {{{$d :date}}}", operand: Some("2006-01-02"), local: Some((Func::DateTime, "dateLength=long timePrecision=second")), func: Func::Date, opts: "", exp_errors: &[] },
    // functions/time.json
    Case { src: "{:time}", operand: None, local: None, func: Func::Time, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{horse :time}", operand: Some("horse"), local: None, func: Func::Time, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{|2006-01-02T15:04:06| :time}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::Time, opts: "", exp_errors: &[] },
    Case { src: "{|2006-01-02T15:04:06| :time precision=second}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::Time, opts: "precision=second", exp_errors: &[] },
    Case { src: ".local $t = {|2006-01-02T15:04:06| :time precision=second} {{{$t}}}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::Time, opts: "precision=second", exp_errors: &[] },
    Case { src: ".local $t = {|2006-01-02T15:04:06| :datetime dateLength=long timePrecision=second} {{{$t :time}}}", operand: Some("2006-01-02T15:04:06"), local: Some((Func::DateTime, "dateLength=long timePrecision=second")), func: Func::Time, opts: "", exp_errors: &[] },
    // functions/datetime.json
    Case { src: "{:datetime}", operand: None, local: None, func: Func::DateTime, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{$x :datetime} (x = true)", operand: Some("true"), local: None, func: Func::DateTime, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{horse :datetime}", operand: Some("horse"), local: None, func: Func::DateTime, opts: "", exp_errors: &["bad-operand"] },
    Case { src: "{|2006-01-02T15:04:06| :datetime}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::DateTime, opts: "", exp_errors: &[] },
    Case { src: "{|2006-01-02T15:04:06| :datetime dateLength=long}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::DateTime, opts: "dateLength=long", exp_errors: &[] },
    Case { src: "{|2006-01-02T15:04:06| :datetime timePrecision=second}", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::DateTime, opts: "timePrecision=second", exp_errors: &[] },
    Case { src: "{$dt :datetime} (dt = datetime 2006-01-02T15:04:06)", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::DateTime, opts: "", exp_errors: &[] },
];

type Fmt<'a> = dyn Fn(&str, &mf2dt::Plan<'_>, &mf2dt::Zoned<'_>, &mut String) -> Result<(), Error> + 'a;

/// Evaluate one case with a backend; returns (output, errors).
fn eval(c: &Case, locale: &str, fmt: &Fmt<'_>) -> (String, Vec<&'static str>) {
    let mut e = Errs::default();
    let Some(src) = c.operand else {
        e.push(Error::BadOperand);
        return ("{:fallback}".into(), e.0);
    };
    let Some(v) = mf2dt::parse_operand(src) else {
        e.push(Error::BadOperand);
        return (format!("{{|{src}|}}"), e.0);
    };
    // A `.local` date value passes on only its override options.
    let inherited = match c.local {
        Some((f, o)) => mf2dt::resolve(f, &opts(o), Overrides::default(), &mut e).ov,
        None => Overrides::default(),
    };
    let plan = mf2dt::resolve(c.func, &opts(c.opts), inherited, &mut e);
    let z = mf2dt::resolve_zone(&v, plan.ov.time_zone, TimeZoneOpt::Utc, &mut e);
    let mut out = String::new();
    if let Err(err) = fmt(locale, &plan, &z, &mut out) {
        e.push(err);
    }
    (out, e.0)
}

fn blob(path: &str) -> dt_icu::OneLocale<icu_provider_blob::BlobDataProvider> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let inner = icu_provider_blob::BlobDataProvider::try_new_from_blob(bytes.into_boxed_slice()).expect("blob");
    // `out/blobs/<kind>-<loc>.postcard`
    let loc = path.rsplit('-').next().and_then(|s| s.strip_suffix(".postcard")).unwrap_or("und");
    dt_icu::OneLocale { inner, locale: loc.parse().expect("locale") }
}

fn main() {
    // 1. Suite cases, compiled data, en-US.
    let compiled = |l: &str, p: &mf2dt::Plan<'_>, z: &mf2dt::Zoned<'_>, o: &mut String| {
        dt_icu::format_fixed_compiled(l, p, z, o)
    };
    let mut pass = 0;
    println!("## suite functions/{{date,time,datetime}}.json — datetime-icu (compiled data, en-US)\n");
    for c in CASES {
        let (out, errs) = eval(c, "en-US", &compiled);
        let ok = errs == c.exp_errors;
        pass += usize::from(ok);
        println!("{} `{}` → `{}` errors={:?}", if ok { "PASS" } else { "FAIL" }, c.src, out, errs);
    }
    println!("\n{pass}/{} error expectations met (the suite has no `exp` for successful date output)\n", CASES.len());

    // 2. Blob ≡ compiled for a matrix of options, per panel locale.
    let matrix: Vec<(Func, String)> = {
        let mut m = Vec::new();
        for f in ["weekday", "day-weekday", "month-day", "month-day-weekday", "year-month-day", "year-month-day-weekday"] {
            for l in ["long", "medium", "short"] {
                m.push((Func::Date, format!("fields={f} length={l}")));
                for p in ["hour", "minute", "second"] {
                    m.push((Func::DateTime, format!("dateFields={f} dateLength={l} timePrecision={p}")));
                }
            }
        }
        for p in ["hour", "minute", "second"] {
            for h in ["", " hour12=true", " hour12=false"] {
                m.push((Func::Time, format!("precision={p}{h}")));
            }
            for z in ["long", "short"] {
                m.push((Func::Time, format!("precision={p} timeZoneStyle={z}")));
                m.push((Func::Time, format!("precision={p} timeZoneStyle={z} timeZone=+05:30")));
            }
        }
        m
    };
    for loc in ["en", "ar", "ja", "ru"] {
        let fixed = blob(&format!("out/blobs/fixed-{loc}.postcard"));
        let nozone = blob(&format!("out/blobs/nozone-{loc}.postcard"));
        let (mut same, mut diff, mut nz_same, mut nz_n) = (0, 0, 0, 0);
        for (f, o) in &matrix {
            let c = Case { src: "", operand: Some("2006-01-02T15:04:06Z"), local: None, func: *f, opts: Box::leak(o.clone().into_boxed_str()), exp_errors: &[] };
            let a = eval(&c, loc, &compiled);
            let b = eval(&c, loc, &|l, p, z, out| dt_icu::format_fixed_buffer(&fixed, l, p, z, out));
            if a == b { same += 1 } else { diff += 1; println!("DIFF {loc} {o}: {a:?} vs {b:?}") }
            if !o.contains("timeZoneStyle") {
                nz_n += 1;
                let n = eval(&c, loc, &|l, p, z, out| dt_icu::format_fixed_nozone_buffer(&nozone, l, p, z, out));
                if a == n { nz_same += 1 } else { println!("NZDIFF {loc} {o}: {a:?} vs {n:?}") }
            }
        }
        println!("{loc}: blob(fixed) ≡ compiled on {same}/{} option sets ({diff} differ); blob(nozone) ≡ compiled on {nz_same}/{nz_n}", matrix.len());
    }

    // 3. Locale key: blob exported for `en`, requested as `en-US`.
    let en = blob("out/blobs/fixed-en.postcard");
    let c = Case { src: "", operand: Some("2006-01-02T15:04:06"), local: None, func: Func::DateTime, opts: "", exp_errors: &[] };
    println!("\nrequest en-US against an `en` blob: {:?}", eval(&c, "en-US", &|l, p, z, out| dt_icu::format_fixed_buffer(&en, l, p, z, out)));
    // 4. Attribute-restricted blob (`ym0d` only).
    let d1 = blob("out/blobs/date1-en.postcard");
    for o in ["", "length=long", "fields=weekday"] {
        let c = Case { src: "", operand: Some("2006-01-02"), local: None, func: Func::Date, opts: o, exp_errors: &[] };
        println!("date1-en blob, :date {o:<16} → {:?}", eval(&c, "en", &|l, p, z, out| dt_icu::format_fixed_nozone_buffer(&d1, l, p, z, out)));
    }

    // 5. Output table for RESULT.md (compiled data) — same cases as
    //    scripts/intl-sample.cjs, same row format, so the two can be diffed.
    println!("\n## output sample (datetime-icu, compiled data = CLDR 48.2.1)\n");
    for (f, o) in [
        (Func::Date, ""), (Func::Date, "length=long"), (Func::Date, "length=short"),
        (Func::Date, "fields=month-day-weekday length=long"), (Func::Time, ""),
        (Func::Time, "precision=second hour12=false"), (Func::DateTime, ""),
        (Func::DateTime, "timeZoneStyle=long"), (Func::DateTime, "timeZoneStyle=short timeZone=+05:30"),
        (Func::DateTime, "timeZone=America/New_York timeZoneStyle=long"),
        (Func::Time, "timeZone=Asia/Kolkata"),
        (Func::Date, "calendar=japanese length=long"),
        (Func::Time, "timeZone=Mars/Olympus"),
    ] {
        let c = Case { src: "", operand: Some("2006-01-02T15:04:06Z"), local: None, func: f, opts: o, exp_errors: &[] };
        let any = |l: &str, p: &mf2dt::Plan<'_>, z: &mf2dt::Zoned<'_>, out: &mut String| dt_icu::format_any_compiled(l, p, z, out);
        let row: Vec<String> = ["en-US", "ar", "ja", "ru"]
            .iter()
            .map(|l| {
                let (t, e) = eval(&c, l, &any);
                let bits = e.iter().fold(0u8, |a, e| a | match *e { "bad-operand" => 1, "bad-option" => 2, _ => 4 });
                if bits == 0 { t } else { format!("{t}⟨err {bits}⟩") }
            })
            .collect();
        let fname = match f { Func::DateTime => "{$d :datetime", Func::Date => "{$d :date", Func::Time => "{$d :time" };
        println!("| `{fname} {o}}}` | {} |", row.join(" | "));
    }
}
