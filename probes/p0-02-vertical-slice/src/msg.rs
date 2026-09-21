//! What the generated i18n module would contain: one `Tr` constant per message
//! id and the manifest hash. The id *strings* appear only inside a `const`
//! expression, so they never reach the wasm (checked by the canary grep in
//! RESULT.md).

use tr::{Tr, fnv1a64, tr};

pub const APP_TITLE: Tr = tr(0);
pub const NAV_HOME: Tr = tr(1);
pub const NAV_LAZY: Tr = tr(2);
pub const NAV_STREAM_OOO: Tr = tr(3);
pub const NAV_STREAM_INORDER: Tr = tr(4);
pub const NAV_STREAM_BLOCKED: Tr = tr(5);
pub const NAV_STREAM_ASYNC: Tr = tr(6);
pub const WELCOME: Tr = tr(7);
pub const LANGUAGE_LABEL: Tr = tr(8);
pub const AUTONYM_EN: Tr = tr(9);
pub const AUTONYM_AR: Tr = tr(10);
pub const SEARCH_LABEL: Tr = tr(11);
pub const SEARCH_PLACEHOLDER: Tr = tr(12);
pub const PROP_DEMO: Tr = tr(13);
pub const TOGGLE_BUTTON: Tr = tr(14);
pub const OPTION_A: Tr = tr(15);
pub const OPTION_B: Tr = tr(16);
pub const SERVER_FN_BUTTON: Tr = tr(17);
pub const LAZY_HEADING: Tr = tr(18);
pub const LAZY_BODY: Tr = tr(19);
pub const LAZY_TITLE_ATTR: Tr = tr(20);
pub const STREAM_HEADING: Tr = tr(21);
pub const STREAM_BODY: Tr = tr(22);
pub const LOADING: Tr = tr(23);
pub const NOT_FOUND: Tr = tr(24);
pub const CANARY: Tr = tr(25);
pub const NAV_P010_TEXT: Tr = tr(26);
pub const NAV_P010_STRUCT: Tr = tr(27);
pub const MAIN_NAV_LABEL: Tr = tr(28);
pub const SIGNAL_DEMO: Tr = tr(29);
pub const SERVER_GREETING: Tr = tr(30);
pub const STREAM_BODY_2: Tr = tr(31);

/// Message ids in `MsgId` order — build-side input, `const`-only.
pub const IDS: &[&str] = &[
    "app-title",
    "nav-home",
    "nav-lazy",
    "nav-stream-ooo",
    "nav-stream-inorder",
    "nav-stream-blocked",
    "nav-stream-async",
    "welcome-text",
    "language-label",
    "autonym-en",
    "autonym-ar",
    "search-label",
    "search-placeholder",
    "prop-demo",
    "toggle-button",
    "option-a",
    "option-b",
    "server-fn-button",
    "lazy-heading",
    "lazy-body",
    "lazy-title-attr",
    "stream-heading",
    "stream-body",
    "loading-text",
    "not-found",
    "canary-msg-id-qx7",
    "nav-p010-text",
    "nav-p010-struct",
    "main-nav-label",
    "signal-demo",
    "server-greeting",
    "stream-body-second",
];

/// FNV-1a 64 over the NUL-joined id list, evaluated at compile time.
pub const MANIFEST_HASH: u64 = fnv1a64(MANIFEST_SRC);
const MANIFEST_SRC: &[u8] = b"app-title\0nav-home\0nav-lazy\0nav-stream-ooo\0nav-stream-inorder\0\
nav-stream-blocked\0nav-stream-async\0welcome-text\0language-label\0autonym-en\0autonym-ar\0\
search-label\0search-placeholder\0prop-demo\0toggle-button\0option-a\0option-b\0server-fn-button\0\
lazy-heading\0lazy-body\0lazy-title-attr\0stream-heading\0stream-body\0loading-text\0not-found\0\
canary-msg-id-qx7\0nav-p010-text\0nav-p010-struct\0main-nav-label\0signal-demo\0server-greeting\0\
stream-body-second\0";
