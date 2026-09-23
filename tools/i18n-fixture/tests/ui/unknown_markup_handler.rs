//! A handler for markup the message does not have (`help` has `#kbd`).
struct Handler;

impl mf2::MarkupHandler for Handler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    let _ = mf2_i18n_fixture::tr!("help", kdb = Handler);
}
