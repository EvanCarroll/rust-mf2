//! The sides a build formats on, and the families of features each has.
//!
//! A backend's feature is a family, a domain and a backend:
//! `leptos-client-` + `datetime` + `-intl`. The families are the same for
//! every domain, so they are written once, here.

use super::backend::Backend;

/// The two builds that format: a browser build, and native code — a server,
/// a command-line tool, a terminal UI. Each has its own backend for a
/// domain, chosen from the features of its own side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// `wasm32-unknown-unknown`: the families `host-web-` and
    /// `leptos-client-`.
    Browser,
    /// Everything else: the families `host-std-`, `leptos-server-`, `axum-`
    /// and `native-`.
    Native,
}

impl Side {
    /// Both sides, the browser first.
    #[doc(hidden)]
    pub const ALL: [Side; 2] = [Side::Browser, Side::Native];

    /// The families of this side, the framework-free one first.
    #[doc(hidden)]
    pub fn families(self) -> impl Iterator<Item = &'static Family> {
        FAMILIES.iter().filter(move |family| family.side == self)
    }

    /// The side as a message names it.
    #[doc(hidden)]
    pub fn name(self) -> &'static str {
        match self {
            Side::Browser => "the browser",
            Side::Native => "native code",
        }
    }

    /// The side as `mf2 check --format json` names it.
    #[doc(hidden)]
    pub fn key(self) -> &'static str {
        match self {
            Side::Browser => "browser",
            Side::Native => "native",
        }
    }
}

/// What a family is named after.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framework {
    /// No framework: `host-std-`, `host-web-`.
    Host,
    /// Leptos: `leptos-client-`, `leptos-server-`.
    Leptos,
    /// An Axum server: `axum-`.
    Axum,
    /// A command-line tool or a terminal UI: `native-`.
    Native,
}

/// A family of features: a framework and the side it runs on. Its features
/// are its prefix, a domain and a backend's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Family {
    /// The family, without its domain: `leptos-server-`.
    pub prefix: &'static str,
    /// The side its builds run on.
    pub side: Side,
    /// What it is named after.
    #[doc(hidden)]
    pub framework: Framework,
}

impl Family {
    /// The family's features of `B`'s domain, without a backend:
    /// `leptos-server-datetime-`.
    #[doc(hidden)]
    pub fn stem<B: Backend>(&self) -> String {
        format!("{}{}-", self.prefix, B::DOMAIN)
    }

    /// The family's feature for `backend`: `leptos-server-datetime-icu`.
    #[doc(hidden)]
    pub fn feature<B: Backend>(&self, backend: B) -> String {
        format!("{}{}-{}", self.prefix, B::DOMAIN, backend.name())
    }

    /// The family's feature for its side's recommended backend.
    #[doc(hidden)]
    pub fn recommended<B: Backend>(&self) -> String {
        self.feature(B::recommended(self.side))
    }
}

/// Every family. Within a side the framework-free one comes first, and the
/// order is the one the tools list features in.
pub const FAMILIES: [Family; 6] = [
    Family {
        prefix: "host-std-",
        side: Side::Native,
        framework: Framework::Host,
    },
    Family {
        prefix: "host-web-",
        side: Side::Browser,
        framework: Framework::Host,
    },
    Family {
        prefix: "leptos-client-",
        side: Side::Browser,
        framework: Framework::Leptos,
    },
    Family {
        prefix: "leptos-server-",
        side: Side::Native,
        framework: Framework::Leptos,
    },
    Family {
        prefix: "axum-",
        side: Side::Native,
        framework: Framework::Axum,
    },
    Family {
        prefix: "native-",
        side: Side::Native,
        framework: Framework::Native,
    },
];

/// Every feature of `B`'s domain: the domain's own, then each family's
/// backends, the weakest first. The order the tools list features in.
#[doc(hidden)]
pub fn domain_features<B: Backend>() -> Vec<String> {
    let mut features = vec![B::DOMAIN.to_owned()];
    for family in &FAMILIES {
        features.extend(
            B::offered(family.side)
                .iter()
                .rev()
                .map(|&backend| family.feature(backend)),
        );
    }
    features
}

/// A kind of application, and the prefixes of the families its builds write.
/// What the tools name when no framework says which build this is: a line is
/// each family's feature for its side's recommended backend
/// ([`kind_lines`]).
#[doc(hidden)]
pub const KINDS: [(&str, &[&str]); 4] = [
    (
        "a server-rendered Leptos application",
        &["leptos-client-", "leptos-server-"],
    ),
    ("a command-line tool or a terminal UI", &["native-"]),
    ("an Axum server", &["axum-"]),
    ("no framework", &["host-std-", "host-web-"]),
];

/// What the tools write for `B`'s domain for each kind of application
/// ([`KINDS`]): the kind, and each of its families' recommended feature.
#[doc(hidden)]
pub fn kind_lines<B: Backend>() -> Vec<(&'static str, Vec<String>)> {
    KINDS
        .iter()
        .map(|(what, prefixes)| {
            let features = FAMILIES
                .iter()
                .filter(|family| prefixes.contains(&family.prefix))
                .map(Family::recommended::<B>)
                .collect();
            (*what, features)
        })
        .collect()
}

/// A family of a framework that is on, and whether this build is one of its
/// builds. A Leptos server's build script also reads the browser's family,
/// which changes no code in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Active {
    /// The family.
    pub family: &'static Family,
    /// Whether this build is one of the family's, and so needs a backend of
    /// its side for every domain its messages use.
    #[doc(hidden)]
    pub builds_here: bool,
}
