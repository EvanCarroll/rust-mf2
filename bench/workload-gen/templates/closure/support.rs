//! Support module of the `closure` template: the shared lookup functions of
//! the closure-per-site stack. Message ids travel as strings and arguments as
//! `(name, value)` pairs, as in the stack this control reproduces.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CATALOG: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Installs one message (`id → text`).
pub fn install(id: &str, text: &str) {
    CATALOG.with(|c| {
        c.borrow_mut().insert(id.to_owned(), text.to_owned());
    });
}

/// The text of `id`, or the id itself when the catalog lacks it.
#[inline(never)]
pub fn lookup(id: &'static str) -> String {
    CATALOG
        .with(|c| c.borrow().get(id).cloned())
        .unwrap_or_else(|| id.to_owned())
}

/// `lookup`, then substitutes `{$name}` for each argument.
#[inline(never)]
pub fn lookup_args(id: &'static str, args: &[(&'static str, String)]) -> String {
    let mut text = lookup(id);
    for (name, value) in args {
        let key = ["{$", name, "}"].concat();
        text = text.replace(&key, value);
    }
    text
}

/// Client boot: loads `id=text` lines from `<html data-catalog>` so that the
/// catalog is opaque to the optimiser, as a fetched catalog would be.
pub fn boot() {
    #[cfg(feature = "hydrate")]
    {
        let data = leptos::prelude::document()
            .document_element()
            .and_then(|e| e.get_attribute("data-catalog"));
        if let Some(data) = data {
            for line in data.lines() {
                if let Some((id, text)) = line.split_once('=') {
                    install(id, text);
                }
            }
        }
    }
}
