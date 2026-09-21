//! The manifest (plans/05 §3): ids sorted → MsgId, per-message variable slots
//! in ascending bytewise order of the NFC-normalized name, markup names, the
//! functions used across all locales, and `manifest_hash` (plans/02 §3:
//! FNV-1a 64 over a canonical serialization of exactly those).

use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

use crate::error::Error;
use crate::model::{Message, external_vars, function_names, markup_names};

pub struct Manifest {
    pub ids: Vec<String>,
    pub slots: Vec<Vec<String>>,
    pub markup: Vec<Vec<String>>,
    pub functions: Vec<String>,
    pub hash: u64,
}

pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

struct Fnv(u64);

impl Fnv {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn uint(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.bytes(&[byte]);
                return;
            }
            self.bytes(&[byte | 0x80]);
        }
    }
    fn str(&mut self, s: &str) {
        self.uint(s.len() as u64);
        self.bytes(s.as_bytes());
    }
}

impl Manifest {
    /// `source`: the source locale's messages, sorted by id. `all`: every
    /// locale (for the function set).
    pub fn build(source: &[(String, Message)], all: &[&[(String, Message)]]) -> Result<Self, Error> {
        let mut ids = Vec::with_capacity(source.len());
        let mut slots = Vec::with_capacity(source.len());
        let mut markup = Vec::with_capacity(source.len());
        for (id, msg) in source {
            ids.push(id.clone());
            let mut s: Vec<String> = external_vars(msg).iter().map(|v| nfc(v)).collect();
            s.sort();
            s.dedup();
            slots.push(s);
            let mut m = markup_names(msg);
            m.sort();
            markup.push(m);
        }
        let mut sorted = ids.clone();
        sorted.sort();
        if sorted != ids {
            return Err(Error::Other("source ids are not sorted bytewise".into()));
        }
        let mut functions = BTreeSet::new();
        for locale in all {
            for (_, msg) in *locale {
                functions.extend(function_names(msg));
            }
        }
        let functions: Vec<String> = functions.into_iter().collect();

        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        h.uint(ids.len() as u64);
        for ((id, s), m) in ids.iter().zip(&slots).zip(&markup) {
            h.str(id);
            h.uint(s.len() as u64);
            s.iter().for_each(|x| h.str(x));
            h.uint(m.len() as u64);
            m.iter().for_each(|x| h.str(x));
        }
        h.uint(functions.len() as u64);
        functions.iter().for_each(|x| h.str(x));
        Ok(Manifest { ids, slots, markup, functions, hash: h.0 })
    }

    pub fn msg_id(&self, id: &str) -> Option<usize> {
        self.ids.binary_search_by(|x| x.as_str().cmp(id)).ok()
    }

    pub fn slot(&self, msg: usize, name: &str) -> Option<usize> {
        let n = nfc(name);
        self.slots.get(msg)?.iter().position(|s| *s == n)
    }

    pub fn function(&self, name: &str) -> Option<usize> {
        self.functions.iter().position(|f| f == name)
    }
}
