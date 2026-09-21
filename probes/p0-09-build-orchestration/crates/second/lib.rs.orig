//! P0.9: a second crate in the application's workspace. It depends on the
//! i18n crate only (not on the facade or the proc-macro) and calls `tr!` in
//! a `static`, in plain code and in a view.

use leptos::prelude::*;
use p09_i18n::__mf2::Tr;
use p09_i18n::tr;

mod extra;

/// Descriptions are data: a static table of messages.
pub static LABELS: &[Tr] = &[tr!("activity-available"), tr!("admin.audio-notification")];

/// Server-rendered report (served at `/second`): text and `MsgId` per line.
pub fn report(n: i64, who: &'static str) -> String {
    let lines = [
        (LABELS[0].to_string(), LABELS[0].id().0),
        {
            let t = tr!("actions-joined-busy", title = who);
            (t.to_string(), t.id().0)
        },
        {
            let t = tr!("admin.labels.character", reason = "moderation", tag = who);
            (t.to_string(), t.id().0)
        },
        {
            let t = tr!("app.form.less-busy-background-connection", count = n);
            (t.to_string(), t.id().0)
        },
    ];
    let mut out = String::new();
    for (text, id) in lines.into_iter().chain(extra::lines(who)) {
        out.push_str(&id.to_string());
        out.push('\t');
        out.push_str(&text);
        out.push('\n');
    }
    out
}

/// A component using `tr!` as a text child.
#[component]
pub fn SecondBadge() -> impl IntoView {
    view! { <span class="badge">{tr!("activity-available")}</span> }
}

/// Client-side code of the second crate, run from the app's hydrate boot.
pub fn client_boot() {
    #[cfg(feature = "hydrate")]
    leptos::logging::log!("p09-second: {} labels; first MsgId {}", LABELS.len(), LABELS[0].id().0);
}
