//! Criterion: B9 (native reference) and B10.
//!   cargo bench -p p08-bench                          # UTF-8: one pass over the pool at load
//!   cargo bench -p p08-bench --features utf8-per-access  # UTF-8: per string on access

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use p03_rt::{Arg, BidiStrategy, Catalog, Formatter, MsgId, NoErrors};
use p08_bench::fixtures;

fn benches(c: &mut Criterion) {
    let fx = fixtures();
    let mut g = c.benchmark_group("load");
    for f in &fx {
        g.bench_function(format!("Catalog::new/{}", f.tag), |b| {
            b.iter_batched(|| f.bytes.clone(), |bytes| black_box(Catalog::new(bytes, f.hash).is_ok()), BatchSize::SmallInput)
        });
    }
    for f in &fx {
        g.bench_function(format!("from_utf8(pool)/{}", f.tag), |b| b.iter(|| black_box(std::str::from_utf8(black_box(&f.bytes[f.pool_start..])).is_ok())));
    }
    g.bench_function("clone(bytes)/en (setup cost, excluded above)", |b| b.iter(|| black_box(fx[0].bytes.clone())));
    g.finish();

    for f in fx.iter().filter(|f| f.tag == "en" || f.tag == "ar-XB") {
        let cat = Catalog::new(f.bytes.clone(), f.hash).unwrap_or_else(|_| panic!("load"));
        let fmt = Formatter::new(&cat, BidiStrategy::Default);
        let mut g = c.benchmark_group(format!("format/{}", f.tag));
        let ids = &f.simple;
        let mut i = 0;
        g.bench_function("simple (fast path, all simple ids)", |b| {
            b.iter(|| {
                i += 1;
                if i == ids.len() {
                    i = 0;
                }
                black_box(fmt.simple(MsgId(black_box(ids[i]))).map(str::len))
            })
        });
        let mut out = String::with_capacity(512);
        let mut i = 0;
        g.bench_function("simple via write() into a reused String", |b| {
            b.iter(|| {
                i += 1;
                if i == ids.len() {
                    i = 0;
                }
                out.clear();
                fmt.write(MsgId(black_box(ids[i])), &[], &mut out, &mut NoErrors);
                black_box(out.len())
            })
        });
        let p1 = &f.pattern1;
        let args = [Arg::Str("Ada Lovelace")];
        let mut i = 0;
        g.bench_function("1-arg pattern, reused String", |b| {
            b.iter(|| {
                i += 1;
                if i == p1.len() {
                    i = 0;
                }
                out.clear();
                fmt.write(MsgId(black_box(p1[i])), black_box(&args), &mut out, &mut NoErrors);
                black_box(out.len())
            })
        });
        let mut i = 0;
        g.bench_function("1-arg pattern, new String::with_capacity(128)", |b| {
            b.iter(|| {
                i += 1;
                if i == p1.len() {
                    i = 0;
                }
                let mut s = String::with_capacity(128);
                fmt.write(MsgId(black_box(p1[i])), black_box(&args), &mut s, &mut NoErrors);
                black_box(s)
            })
        });
        let sel = &f.select;
        let mut i = 0;
        let mut n = 0i64;
        g.bench_function("select (plural on $count), reused String", |b| {
            b.iter(|| {
                i += 1;
                if i == sel.len() {
                    i = 0;
                    n = (n + 1) % 30;
                }
                out.clear();
                fmt.write(MsgId(black_box(sel[i])), black_box(&[Arg::Int(n)]), &mut out, &mut NoErrors);
                black_box(out.len())
            })
        });
        g.finish();
    }
}

criterion_group!(load_lookup, benches);
criterion_main!(load_lookup);
