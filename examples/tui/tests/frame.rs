//! Both renderers draw the whole frame in every language.

use demo_tui::model::{LOCALES, SAMPLE};
use demo_tui::prelude::*;
use demo_tui::{AREA, to_text, ui, upstream};
use ratatui::buffer::Buffer;

/// The frame in `tag`, drawn on this thread in that language.
fn mf2_frame(tag: &str) -> String {
    demo_tui::install();
    mf2::ratatui::set_theme(ui::theme());
    with_locale(tag.parse().unwrap(), || {
        let mut buf = Buffer::empty(AREA);
        ui::draw(&SAMPLE, AREA, &mut buf);
        to_text(&buf)
    })
}

fn upstream_frame(tag: &str) -> String {
    upstream::set_locale(tag);
    let mut buf = Buffer::empty(AREA);
    upstream::draw(&SAMPLE, AREA, &mut buf);
    to_text(&buf)
}

#[test]
fn every_language_draws_its_own_text() {
    let expected = [
        (
            "en",
            "Target: 192.168.1.10",
            "Discovered 12 hops and 2 unique flows",
        ),
        (
            "de",
            "Ziel: 192.168.1.10",
            "12 Hops und 2 eindeutige Flows entdeckt",
        ),
        (
            "es",
            "Destino: 192.168.1.10",
            "Descubiertos 12 saltos y 2 flujos únicos",
        ),
        (
            "fr",
            "Cible : 192.168.1.10",
            "Découverts : 12 sauts et 2 flux uniques",
        ),
    ];
    for (tag, target, discovered) in expected {
        let frame = mf2_frame(tag);
        assert!(frame.contains(target), "{tag}:\n{frame}");
        assert!(frame.contains(discovered), "{tag}:\n{frame}");
        // A message that failed to format would show MF2's `{…}` fallback.
        assert!(!frame.contains('{'), "{tag}:\n{frame}");
    }
}

#[test]
fn numbers_and_plurals_follow_the_language() {
    assert!(mf2_frame("en").contains("2,411 of 14,448 probes failed (16.7%)"));
    assert!(mf2_frame("de").contains("2.411 von 14.448 Proben fehlgeschlagen (16,7\u{a0}%)"));
    // Spanish groups only from five digits on.
    assert!(mf2_frame("es").contains("2411 de 14.448 sondas fallaron (16,7\u{a0}%)"));
    // French agrees the verb with the failed probes, not the total.
    assert!(
        mf2_frame("fr").contains("2\u{202f}411 sondes sur 14\u{202f}448 ont échoué (16,7\u{a0}%)")
    );
}

#[test]
fn the_baseline_draws_every_language() {
    for tag in LOCALES {
        let frame = upstream_frame(tag);
        assert!(frame.contains("192.168.1.10"), "{tag}:\n{frame}");
    }
    // The baseline's key hints: bolded by slicing where the word starts with
    // the key, else the key in brackets before the word.
    assert!(upstream_frame("en").contains("help  settings"));
    assert!(upstream_frame("fr").contains("[h]aide  [s]paramètres"));
}
