//! One of the message's two markup names handled, the other not: handlers
//! are all or none (`plans/04-leptos-integration.md` §2.1). Handling none is
//! legal — the markup then formats to parts, as the spec says — but handling
//! `#kbd` and forgetting `#b` is an oversight, not a choice.
struct Handler;

impl mf2::MarkupHandler for Handler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    let _ = mf2_i18n_fixture::tr!("help", kbd = Handler);
}
