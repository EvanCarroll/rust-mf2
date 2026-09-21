//! The manifest (A3): `manifest_hash` of the reference workload reproduces
//! P0.7's figure, and `manifest.mf2m` round-trips and rejects damage.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mf2_catalog::{Manifest, ManifestError};

/// The manifest of the committed reference workload, built the way
/// `mf2-build` will: ids sorted, slots and markup from `analyze`, the
/// functions every message uses.
fn workload_manifest() -> Manifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/corpora/workload-1600.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let corpus: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
    let mut m = Manifest::default();
    let mut functions = BTreeSet::new();
    for (id, src) in &corpus {
        let parsed = mf2_syntax::parse_model(src);
        let model = parsed.message.unwrap();
        let a = mf2_syntax::analyze(&model);
        m.ids.push(id.clone());
        m.slots
            .push(a.externals.iter().map(|n| n.nfc.to_string()).collect());
        m.markup
            .push(a.markup.iter().map(|n| n.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|n| n.nfc.to_string()));
    }
    m.functions = functions.into_iter().collect();
    m
}

#[test]
fn workload_hash_matches_p07() {
    let m = workload_manifest();
    assert_eq!(m.ids.len(), 1600);
    assert_eq!(m.functions, ["integer"]);
    m.validate().unwrap();
    assert_eq!(m.hash(), 0x43e0_dc12_eeb0_5ef1);
}

#[test]
fn file_round_trips() {
    let m = workload_manifest();
    let file = m.write();
    assert_eq!(&file[..4], b"MF2M");
    let back = Manifest::read(&file).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.hash(), m.hash());
    assert_eq!(
        m.msg_id(&m.ids[17]).map(mf2_catalog::MsgId::index),
        Some(17)
    );
    assert_eq!(m.msg_id("no such id"), None);
}

#[test]
fn damage_is_rejected() {
    let m = workload_manifest();
    let file = m.write();
    assert_eq!(Manifest::read(b"MF2X"), Err(ManifestError::Magic));
    let mut v = file.clone();
    v[5] = 2;
    assert_eq!(Manifest::read(&v), Err(ManifestError::Version));
    let mut v = file.clone();
    v[6] ^= 1;
    assert_eq!(Manifest::read(&v), Err(ManifestError::Hash));
    let mut v = file.clone();
    v.push(0);
    assert_eq!(Manifest::read(&v), Err(ManifestError::Trailing));
    for len in [0, 3, 5, 13, 14, 15, file.len() / 2, file.len() - 1] {
        assert!(Manifest::read(&file[..len]).is_err(), "prefix {len}");
    }
    // A content change without a matching hash.
    let mut v = file.clone();
    let at = v.len() - 3;
    v[at] ^= 0x01;
    assert!(Manifest::read(&v).is_err());
    // Out-of-order lists, with a correct hash.
    let mut bad = m.clone();
    bad.ids.swap(0, 1);
    assert!(matches!(
        Manifest::read(&bad.write()),
        Err(ManifestError::Invalid(_))
    ));
    let mut bad = m;
    bad.functions = vec!["number".into(), "integer".into()];
    assert!(matches!(
        Manifest::read(&bad.write()),
        Err(ManifestError::Invalid(_))
    ));
}
