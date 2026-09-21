//! P0.8 fixtures: the four workload catalogs in P0.7's recommended layout,
//! built in memory, plus the MsgIds of each benchmark class.

use p03_rt::{Catalog, Entry, MsgId};
use p07_enc::write::Layout;
use p07_enc::{LOCALES, default_corpus_dir, load_corpus};

pub struct Fixture {
    pub tag: String,
    pub bytes: Vec<u8>,
    pub hash: u64,
    /// Offset of the STRINGS pool (last section) in `bytes`.
    pub pool_start: usize,
    pub simple: Vec<u32>,
    /// `pattern` messages with exactly one argument slot.
    pub pattern1: Vec<u32>,
    pub select: Vec<u32>,
}

fn pool_start(b: &[u8]) -> usize {
    let n = u16::from_le_bytes([b[28], b[29]]) as usize;
    let last = 30 + (n - 1) * 10;
    u32::from_le_bytes(b[last + 2..last + 6].try_into().unwrap()) as usize
}

pub fn fixtures() -> Vec<Fixture> {
    let corpus = load_corpus(&default_corpus_dir(), &LOCALES).expect("corpus");
    let m = &corpus.manifest;
    corpus
        .locales
        .iter()
        .map(|l| {
            let (bytes, _) = l.write(m, Layout::RECOMMENDED).expect("write");
            let cat = Catalog::new(bytes.clone(), m.hash).unwrap_or_else(|_| panic!("load"));
            let (mut simple, mut pattern1, mut select) = (Vec::new(), Vec::new(), Vec::new());
            for i in 0..cat.len() {
                match cat.get(MsgId(i)) {
                    Entry::Simple(_) => simple.push(i),
                    Entry::Pattern(_) if m.slots[i as usize].len() == 1 => pattern1.push(i),
                    Entry::Select(_) => select.push(i),
                    _ => {}
                }
            }
            Fixture { tag: l.tag.clone(), pool_start: pool_start(&bytes), bytes, hash: m.hash, simple, pattern1, select }
        })
        .collect()
}
