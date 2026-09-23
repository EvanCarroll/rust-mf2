//! Markup → elements (`plans/04-leptos-integration.md` §7).
//!
//! `{#kbd}…{/kbd}` is how a sentence carries an inline element without baking
//! word order into code — the feature the prior-art audit found missing
//! everywhere else (§11, item 1). A [`TrRich`] formats **to parts** and this
//! module turns those parts into a fragment: text parts become text, an
//! `open…close` span calls its handler with the inner fragment as children,
//! and standalone markup calls its handler with none.
//!
//! **Pairing is ours, not the spec's.** MF2 does not require markup to be
//! paired, and the suite tests lone opens and closes, so the renderer pairs
//! with a stack and never fails: a close with no open is dropped, and an open
//! that is never closed is closed at the end of the pattern. `mf2 check`
//! reports both as the `unpaired-markup` warning, where a corpus-wide
//! question belongs.
//!
//! **Names never reach the wasm.** A part's name is hashed
//! ([`markup_key`]) and matched against the keys the call site's handlers
//! were built with, exactly as `TrRich::handler` does (B6). The stack matches
//! opens to closes by that same hash, so nothing here allocates a name.

use alloc::string::String;
use alloc::vec::Vec;

use mf2_catalog::{Catalog, markup_key};
use mf2_runtime::{ErrorSink, MarkupKind, MarkupPart, NoErrors, Part, PartSink};
use tachys::view::any_view::{AnyView, IntoAny};

use crate::markup::{FlatHandler, NestingHandler};
use crate::state::{self, TextUse};
use crate::tr::{MarkupHandler, TrRich};

/// The fragment `rich` renders to against `catalog`.
pub(crate) fn fragment(rich: &TrRich, catalog: &Catalog) -> Vec<AnyView> {
    let Some(formatter) = state::formatter_for(catalog, TextUse::Displayed) else {
        return Vec::new();
    };
    let mut builder = Builder {
        rich,
        text: String::new(),
        stack: Vec::new(),
        root: Vec::new(),
    };
    let mut errors = NoErrors;
    rich.parts(&formatter, &mut builder, &mut errors as &mut dyn ErrorSink);
    builder.finish()
}

/// The fragment against whatever catalog this render reads.
pub(crate) fn active_fragment(rich: &TrRich) -> Vec<AnyView> {
    match crate::catalog::active() {
        Some(catalog) => fragment(rich, &catalog),
        None => Vec::new(),
    }
}

struct Builder<'a> {
    rich: &'a TrRich,
    /// Text since the last markup part, not yet made into a node.
    text: String,
    /// One frame per open markup still waiting for its close: the hash of its
    /// name, and the children collected so far.
    stack: Vec<(u64, Vec<AnyView>)>,
    root: Vec<AnyView>,
}

impl PartSink for Builder<'_> {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(t) => self.text.push_str(t),
            Part::BidiIsolation(i) => self.text.push_str(i.as_str()),
            Part::Expression(e) => e.write(&mut self.text),
            Part::Fallback(source) => {
                self.text.push('{');
                source.write(&mut self.text);
                self.text.push('}');
            }
            Part::Markup(m) => self.markup(&m),
        }
    }
}

impl Builder<'_> {
    /// The handler for a markup name, by the hash of that name.
    fn handler(&self, key: u64) -> Option<&dyn MarkupHandler> {
        self.rich
            .handlers()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, h)| &**h)
    }

    /// Appends a finished view to the innermost open frame, or to the root.
    fn push(&mut self, view: AnyView) {
        match self.stack.last_mut() {
            Some((_, children)) => children.push(view),
            None => self.root.push(view),
        }
    }

    /// Turns the text collected so far into a node.
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let text = core::mem::take(&mut self.text);
        self.push(text.into_any());
    }

    fn markup(&mut self, m: &MarkupPart<'_>) {
        let key = markup_key(m.name());
        // No handler for this name: markup writes no text, so the message
        // renders without it — which is what a call site that supplied no
        // handlers at all asks for, and what L5 asserts.
        let Some(handler) = self.handler(key) else {
            return;
        };
        // A flat handler takes every part as it comes: no pairing, one node
        // each. This is what conformance L6 compares against `expParts`.
        if let Some(flat) = handler.as_any().downcast_ref::<FlatHandler>() {
            let view = flat.call(m);
            self.flush();
            self.push(view);
            return;
        }
        if handler.as_any().downcast_ref::<NestingHandler>().is_none() {
            return;
        }
        match m.kind() {
            MarkupKind::Open => {
                self.flush();
                self.stack.push((key, Vec::new()));
            }
            MarkupKind::Standalone => {
                self.flush();
                self.close(key, Vec::new());
            }
            MarkupKind::Close => {
                self.flush();
                // The innermost open of this name. Opens inside it were
                // never closed, so they close here, innermost first.
                let Some(at) = self.stack.iter().rposition(|(k, _)| *k == key) else {
                    // A close with no open: dropped, as §7 says.
                    return;
                };
                while self.stack.len() > at + 1 {
                    if let Some((inner, children)) = self.stack.pop() {
                        self.close(inner, children);
                    }
                }
                if let Some((_, children)) = self.stack.pop() {
                    self.close(key, children);
                }
            }
        }
    }

    /// Calls the nesting handler for `key` with `children` and appends what
    /// it built.
    fn close(&mut self, key: u64, children: Vec<AnyView>) {
        let view = self
            .handler(key)
            .and_then(|h| h.as_any().downcast_ref::<NestingHandler>())
            .map(|nesting| nesting.call(children.into_any()));
        if let Some(view) = view {
            self.push(view);
        }
    }

    fn finish(mut self) -> Vec<AnyView> {
        self.flush();
        // Opens that were never closed close at the end of the pattern.
        while let Some((key, children)) = self.stack.pop() {
            self.close(key, children);
        }
        self.root
    }
}
