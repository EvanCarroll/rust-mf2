//! A4 probe: time and allocations of variants (a)–(d), per case and per
//! frame (`plans/18-phase-10-work-order.md` A4).
//!
//! ```text
//! ambient-bench check            every variant renders every message alike
//! ambient-bench allocs           allocations and bytes, exact, per case
//! ambient-bench time [ROUNDS]    ns per case, variants interleaved
//! ```
//!
//! Every variant formats in `fr` (not the source locale). Timing runs the
//! variants round-robin, each round a batch of iterations per variant, the
//! starting variant rotating, so drift and load hit every variant alike;
//! it reports the median of the rounds and their 10th–90th percentiles.

#[macro_use]
mod frame;
mod alloc;
mod variants;

use std::hint::black_box;
use std::io::Write as _;
use std::time::Instant;

use ambient::{Method, Theme};
use mf2_native::NativeI18n;
use mf2_ratatui::MarkupStyles;
use probe_i18n::tr;
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::text::Line;

use crate::frame::{MESSAGES, STATE};
use crate::variants::{PORT, frame_a, frame_b, frame_b_copy, frame_d, line_copy, t, t_line};

#[global_allocator]
static COUNTING: alloc::Counting = alloc::Counting;

/// The styles every variant gives the corpus's markup names.
fn styles() -> [(&'static str, Style); 7] {
    [
        ("ok", Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ("host", Style::new().add_modifier(Modifier::UNDERLINED)),
        ("key", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ("warn", Style::new().fg(Color::Yellow)),
        ("err", Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ("dim", Style::new().fg(Color::DarkGray)),
        ("b", Style::new().add_modifier(Modifier::BOLD)),
    ]
}

fn markup_styles() -> MarkupStyles {
    styles()
        .into_iter()
        .fold(MarkupStyles::new(), |s, (name, style)| s.with(name, style))
}

fn theme() -> Theme {
    styles()
        .into_iter()
        .fold(Theme::with_defaults(), |t, (name, style)| t.style(name, style))
}

/// One variant of one case: a name and one iteration.
type Run<'a> = (&'static str, Box<dyn FnMut() + 'a>);

fn methods() -> Vec<(&'static str, Method)> {
    let mut m = vec![("b", Method::RangePool), ("b-utf8", Method::RangeUtf8)];
    if cfg!(feature = "seam") {
        m.push(("b-seam", Method::Seam));
    }
    m
}

/// Every case, each with its variants.
fn cases<'a>(a: &'a NativeI18n, styles: &'a MarkupStyles) -> Vec<(&'static str, Vec<Run<'a>>)> {
    let mut simple_string: Vec<Run<'a>> = vec![
        ("a", Box::new(move || drop(black_box(a.format(&tr!("app.title")))))),
        ("b", Box::new(|| drop(black_box(ambient::to_string::<false>(&tr!("app.title")))))),
        ("c", Box::new(|| drop(black_box(ambient::to_string::<true>(&tr!("app.title")))))),
        ("d", Box::new(|| drop(black_box(t(&tr!("app.title")))))),
    ];
    simple_string.shrink_to_fit();
    let simple_static: Vec<Run<'a>> = vec![
        ("b", Box::new(|| drop(black_box(ambient::to_cow::<false>(&tr!("app.title")))))),
        ("c", Box::new(|| drop(black_box(ambient::to_cow::<true>(&tr!("app.title")))))),
    ];
    let one_arg: Vec<Run<'a>> = vec![
        ("a", Box::new(move || drop(black_box(a.format(&tr!("status.round", n = 42u32)))))),
        ("b", Box::new(|| drop(black_box(ambient::to_string::<false>(&tr!("status.round", n = 42u32)))))),
        ("c", Box::new(|| drop(black_box(ambient::to_string::<true>(&tr!("status.round", n = 42u32)))))),
        ("d", Box::new(|| drop(black_box(t(&tr!("status.round", n = 42u32)))))),
    ];
    // `println!("{}", …)`'s path, into a writer that counts the bytes (not
    // `io::sink()`, whose `write_fmt` never formats): 1.x formats a `String`
    // and prints it; the design streams through `Display`.
    let print: Vec<Run<'a>> = vec![
        ("a", Box::new(move || {
            let mut w = Count(0);
            let _ = writeln!(w, "{}", a.format(&tr!("status.target", host = "example.org")));
            black_box(w.0);
        })),
        ("b", Box::new(|| {
            let mut w = Count(0);
            let _ = writeln!(w, "{}", ambient::Show(&tr!("status.target", host = "example.org")));
            black_box(w.0);
        })),
        ("d", Box::new(|| {
            let mut w = Count(0);
            let _ = writeln!(w, "{}", t(&tr!("status.target", host = "example.org")));
            black_box(w.0);
        })),
    ];
    let string_arg: Vec<Run<'a>> = vec![
        ("a", Box::new(move || drop(black_box(a.format(&tr!("status.target", host = "example.org")))))),
        ("b", Box::new(|| drop(black_box(ambient::to_string::<false>(&tr!("status.target", host = "example.org")))))),
        ("b-display", Box::new(|| drop(black_box(ambient::Show(&tr!("status.target", host = "example.org")).to_string())))),
        ("d", Box::new(|| drop(black_box(t(&tr!("status.target", host = "example.org")))))),
    ];
    let host = STATE.host;
    let mut markup: Vec<Run<'a>> = vec![
        ("a", Box::new(move || drop(black_box(mf2_ratatui::line(a, &tr!("status.connected", host = host), styles))))),
        ("b-copy", Box::new(move || drop(black_box(line_copy::<false>(&tr!("status.connected", host = host), styles))))),
    ];
    for (name, method) in methods() {
        markup.push((name, Box::new(move || drop(black_box(ambient::line::<false>(&tr!("status.connected", host = host), method))))));
    }
    markup.push(("c", Box::new(move || drop(black_box(ambient::line::<true>(&tr!("status.connected", host = host), Method::RangePool))))));
    markup.push(("d", Box::new(move || drop(black_box(t_line(&tr!("status.connected", host = host), styles))))));

    let lookup: Vec<Run<'a>> = vec![
        ("a", Box::new(move || {
            black_box(black_box(a).formatter().map(|f| f.catalog().as_bytes().len()));
        })),
        ("b", Box::new(|| {
            black_box(ambient::lookup_cost::<false>());
        })),
        ("c", Box::new(|| {
            black_box(ambient::lookup_cost::<true>());
        })),
        ("d", Box::new(|| {
            black_box(PORT.with(|p| p.borrow().as_ref().and_then(|i| i.formatter().map(|f| f.catalog().as_bytes().len()))));
        })),
    ];

    let mut frames: Vec<Run<'a>> = Vec::new();
    frames.push(("a", frame_run(move |out| frame_a(a, styles, &STATE, out))));
    frames.push(("b-copy", frame_run(move |out| frame_b_copy(styles, &STATE, out))));
    for (name, method) in methods() {
        frames.push((name, frame_run(move |out| frame_b::<false>(method, &STATE, out))));
    }
    frames.push(("c", frame_run(|out| frame_b::<true>(Method::RangePool, &STATE, out))));
    frames.push(("d", frame_run(move |out| frame_d(styles, &STATE, out))));

    vec![
        ("lookup: catalog + settings", lookup),
        ("simple → String", simple_string),
        ("simple → &'static str", simple_static),
        ("one argument → String", one_arg),
        ("one string argument → String", string_arg),
        ("one string argument → println!", print),
        ("markup → Line", markup),
        ("frame (112 messages → Line)", frames),
    ]
}

/// A writer that keeps nothing but the count of bytes written.
struct Count(usize);

impl std::io::Write for Count {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += black_box(buf).len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A frame run: the lines go into a buffer the harness reuses, so that the
/// count is the frame's own.
fn frame_run<'a>(mut body: impl FnMut(&mut Vec<Line<'static>>) + 'a) -> Box<dyn FnMut() + 'a> {
    let mut out: Vec<Line<'static>> = Vec::with_capacity(MESSAGES);
    Box::new(move || {
        out.clear();
        body(&mut out);
        black_box(&out);
    })
}

fn setup() -> NativeI18n {
    ambient::install(&probe_i18n::CORPUS).expect("the corpus installs");
    ambient::set_locale("fr").expect("fr is supported");
    ambient::set_theme(theme());
    let mut a = NativeI18n::embedded(&probe_i18n::CORPUS).expect("the corpus loads");
    a.set_locale("fr").expect("fr is supported");
    let mut d = NativeI18n::embedded(&probe_i18n::CORPUS).expect("the corpus loads");
    d.set_locale("fr").expect("fr is supported");
    PORT.with(|p| *p.borrow_mut() = Some(d));
    a
}

fn main() {
    let a = setup();
    let styles = markup_styles();
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => check(&a, &styles),
        Some("allocs") => allocs(&a, &styles),
        Some("pools") => {
            let mut best = u128::MAX;
            let mut bytes = 0;
            for _ in 0..101 {
                let start = Instant::now();
                bytes = black_box(ambient::pools_cost());
                best = best.min(start.elapsed().as_nanos());
            }
            println!("R2 one-time: {bytes} B of string pools validated in {best} ns (best of 101; load {})", load());
        }
        Some("breakdown") => {
            let _ = variants::breakdown(&a, &styles, &STATE); // warm
            let rows = variants::breakdown(&a, &styles, &STATE);
            let mut groups: std::collections::BTreeMap<(&str, u64, u64), usize> =
                std::collections::BTreeMap::new();
            for &(kind, x, y) in &rows {
                *groups.entry((kind, x, y)).or_default() += 1;
            }
            println!("| kind | a allocations | b allocations | messages |");
            println!("|---|---:|---:|---:|");
            for ((kind, x, y), n) in groups {
                println!("| {kind} | {x} | {y} | {n} |");
            }
            let (ta, tb) = rows.iter().fold((0, 0), |(p, q), &(_, x, y)| (p + x, q + y));
            println!("| total | {ta} | {tb} | {} |", rows.len());
            let heavy: Vec<String> = rows
                .iter()
                .enumerate()
                .filter(|(_, (_, _, y))| *y >= 3)
                .map(|(i, (_, x, y))| format!("#{i}: a {x}, b {y}"))
                .collect();
            println!("messages with 3 or more in b (frame order): {}", heavy.join("; "));
        }
        Some("time") => {
            let rounds = args.next().and_then(|r| r.parse().ok()).unwrap_or(31);
            time(&a, &styles, rounds, args.next());
        }
        Some("time-mt") => {
            let threads = args.next().and_then(|r| r.parse().ok()).unwrap_or(8);
            let rounds = args.next().and_then(|r| r.parse().ok()).unwrap_or(15);
            time_mt(threads, rounds);
        }
        _ => eprintln!("usage: ambient-bench check | allocs | time [ROUNDS] | time-mt [THREADS] [ROUNDS]"),
    }
}

/// Each message as characters and their styles, for comparing variants
/// that split spans differently.
fn flatten(line: &Line<'_>) -> Vec<(char, Style)> {
    line.spans
        .iter()
        .flat_map(|span| {
            let style = line.style.patch(span.style);
            span.content.chars().map(move |c| (c, style))
        })
        .collect()
}

fn check(a: &NativeI18n, styles: &MarkupStyles) {
    let mut reference = Vec::with_capacity(MESSAGES);
    frame_a(a, styles, &STATE, &mut reference);
    let mut variants: Vec<(&str, Vec<Line<'static>>)> = Vec::new();
    let mut out = Vec::new();
    frame_b_copy(styles, &STATE, &mut out);
    variants.push(("b-copy", std::mem::take(&mut out)));
    for (name, method) in methods() {
        frame_b::<false>(method, &STATE, &mut out);
        variants.push((name, std::mem::take(&mut out)));
    }
    frame_b::<true>(Method::RangePool, &STATE, &mut out);
    variants.push(("c", std::mem::take(&mut out)));
    frame_d(styles, &STATE, &mut out);
    variants.push(("d", std::mem::take(&mut out)));
    let mut bad = 0;
    for (name, lines) in &variants {
        assert_eq!(lines.len(), MESSAGES, "{name}");
        for (i, (x, y)) in reference.iter().zip(lines).enumerate() {
            if flatten(x) != flatten(y) {
                bad += 1;
                eprintln!("{name} differs at message {i}:\n  a: {x:?}\n  {name}: {y:?}");
            }
        }
    }
    // Borrowed, really: every text part of every markup line of `b` points
    // into the executable (the catalogs' bytes), not the heap.
    let (_, b) = &variants[1];
    let borrowed = b
        .iter()
        .flat_map(|l| &l.spans)
        .filter(|s| matches!(s.content, std::borrow::Cow::Borrowed(_)))
        .count();
    let owned = b
        .iter()
        .flat_map(|l| &l.spans)
        .filter(|s| matches!(s.content, std::borrow::Cow::Owned(_)))
        .count();
    println!("sample (fr): {:?}", reference.get(97).map(flatten).map(|v| v.into_iter().map(|(c, _)| c).collect::<String>()));
    println!("b: {borrowed} borrowed spans, {owned} owned spans in the frame");
    println!(
        "check: {} variants × {MESSAGES} messages against a: {}",
        variants.len(),
        if bad == 0 { "identical text and styles".to_owned() } else { format!("{bad} differ") }
    );
    std::process::exit(i32::from(bad != 0));
}

fn allocs(a: &NativeI18n, styles: &MarkupStyles) {
    let mut cases = cases(a, styles);
    println!("| case | variant | allocations | bytes | (second run) |");
    println!("|---|---|---:|---:|---|");
    for (case, runs) in &mut cases {
        for (name, run) in runs.iter_mut() {
            run(); // warm: thread-locals, the scratch, the settings copy
            let _ = alloc::take();
            run();
            let first = alloc::take();
            run();
            let second = alloc::take();
            let same = if first == second { "same" } else { "DIFFERS" };
            println!("| {case} | {name} | {} | {} | {same} |", first.0, first.1);
        }
    }
}

/// `filter`: only the cases whose name contains it.
fn time(a: &NativeI18n, styles: &MarkupStyles, rounds: usize, filter: Option<String>) {
    let mut cases = cases(a, styles);
    if let Some(filter) = filter {
        cases.retain(|(case, _)| case.contains(filter.as_str()));
    }
    println!("rounds: {rounds}; load: {}", load());
    println!("| case | variant | median ns | p10 | p90 | vs first |");
    println!("|---|---|---:|---:|---:|---:|");
    for (case, runs) in &mut cases {
        // A batch long enough to be ≈ 2 ms for the first variant.
        let (_, first) = &mut runs[0];
        let t0 = Instant::now();
        let mut probe_iters = 0u64;
        while t0.elapsed().as_micros() < 2_000 {
            first();
            probe_iters += 1;
        }
        let batch = probe_iters.max(1);
        let n = runs.len();
        let mut samples: Vec<Vec<f64>> = vec![Vec::with_capacity(rounds); n];
        for round in 0..rounds {
            for k in 0..n {
                let v = (round + k) % n;
                let run = &mut runs[v].1;
                let start = Instant::now();
                for _ in 0..batch {
                    run();
                }
                #[allow(clippy::cast_precision_loss)]
                samples[v].push(start.elapsed().as_nanos() as f64 / batch as f64);
            }
        }
        let mut base = None;
        for (v, (name, _)) in runs.iter().enumerate() {
            let s = &mut samples[v];
            s.sort_by(f64::total_cmp);
            let pick = |q: f64| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
                let i = ((s.len() - 1) as f64 * q).round() as usize;
                s[i]
            };
            let (med, p10, p90) = (pick(0.5), pick(0.1), pick(0.9));
            let base = *base.get_or_insert(med);
            println!("| {case} | {name} | {med:.1} | {p10:.1} | {p90:.1} | {:.2}× |", med / base);
        }
    }
    println!("load after: {}", load());
}

/// `threads` threads drawing frames at once, per variant: what the
/// settings lock (c) costs when every core formats. Each round starts the
/// threads together (a barrier) and each thread times its own frames; the
/// round's figure is the median thread's ns per frame. Variants rotate.
fn time_mt(threads: usize, rounds: usize) {
    const FRAMES: usize = 40;
    let variants: [&str; 4] = ["a", "b", "c", "d"];
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); variants.len()];
    println!("threads: {threads}; rounds: {rounds}; {FRAMES} frames per thread per round; load: {}", load());
    for round in 0..rounds {
        for k in 0..variants.len() {
            let v = (round + k) % variants.len();
            let barrier = std::sync::Barrier::new(threads);
            let mut per_thread: Vec<f64> = std::thread::scope(|scope| {
                let workers: Vec<_> = (0..threads)
                    .map(|_| {
                        let barrier = &barrier;
                        scope.spawn(move || {
                            let styles = markup_styles();
                            let mut i18n = NativeI18n::embedded(&probe_i18n::CORPUS).expect("loads");
                            i18n.set_locale("fr").expect("fr");
                            if v == 3 {
                                let mut d = NativeI18n::embedded(&probe_i18n::CORPUS).expect("loads");
                                d.set_locale("fr").expect("fr");
                                PORT.with(|p| *p.borrow_mut() = Some(d));
                            }
                            let mut out = Vec::with_capacity(MESSAGES);
                            let frame = |out: &mut Vec<Line<'static>>| {
                                out.clear();
                                match v {
                                    0 => frame_a(&i18n, &styles, &STATE, out),
                                    1 => frame_b::<false>(Method::RangePool, &STATE, out),
                                    2 => frame_b::<true>(Method::RangePool, &STATE, out),
                                    _ => frame_d(&styles, &STATE, out),
                                }
                                black_box(&*out);
                            };
                            frame(&mut out); // warm
                            barrier.wait();
                            let start = Instant::now();
                            for _ in 0..FRAMES {
                                frame(&mut out);
                            }
                            #[allow(clippy::cast_precision_loss)]
                            let ns = start.elapsed().as_nanos() as f64 / FRAMES as f64;
                            ns
                        })
                    })
                    .collect();
                workers.into_iter().map(|w| w.join().expect("worker")).collect()
            });
            per_thread.sort_by(f64::total_cmp);
            samples[v].push(per_thread[per_thread.len() / 2]);
        }
    }
    println!("| variant | median ns per frame | p10 | p90 | vs a |");
    println!("|---|---:|---:|---:|---:|");
    let mut base = None;
    for (v, name) in variants.iter().enumerate() {
        let s = &mut samples[v];
        s.sort_by(f64::total_cmp);
        let pick = |q: f64| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
            let i = ((s.len() - 1) as f64 * q).round() as usize;
            s[i]
        };
        let (med, p10, p90) = (pick(0.5), pick(0.1), pick(0.9));
        let base = *base.get_or_insert(med);
        println!("| {name} | {med:.0} | {p10:.0} | {p90:.0} | {:.2}× |", med / base);
    }
    println!("load after: {}", load());
}

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}
