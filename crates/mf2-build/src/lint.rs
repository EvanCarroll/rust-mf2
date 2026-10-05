//! The lints of `mf2 check`: their names, their
//! default levels, and how low `mf2.toml` may set each one.
//!
//! The checks themselves are in [`crate::check`]; this module is what
//! [`Config`](crate::Config) parses `[lints]` against, so a name in a
//! configuration and a name in a report are the same string.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What a lint does when it fires.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Say nothing.
    Allow,
    /// Report it; the build goes on.
    Warn,
    /// Report it; the build fails.
    Error,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Level::Allow => "allow",
            Level::Warn => "warn",
            Level::Error => "error",
        })
    }
}

macro_rules! lints {
    ($(
        $(#[$meta:meta])*
        $variant:ident = ($name:literal, $default:ident, $floor:ident);
    )*) => {
        /// One check `mf2 check` makes.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize, Serialize)]
        #[serde(rename_all = "kebab-case")]
        #[non_exhaustive]
        pub enum Lint {
            $( $(#[$meta])* $variant, )*
        }

        impl Lint {
            /// Every lint, in declaration order.
            pub const ALL: &'static [Lint] = &[$( Lint::$variant, )*];

            /// The name `mf2.toml` and a report use.
            pub fn name(self) -> &'static str {
                match self { $( Lint::$variant => $name, )* }
            }

            /// What it does when `mf2.toml` says nothing.
            pub fn default_level(self) -> Level {
                match self { $( Lint::$variant => Level::$default, )* }
            }

            /// The lowest level `mf2.toml` may set it to. A lint whose floor
            /// is [`Level::Error`] states a rule the rest of the pipeline
            /// relies on — a build that ignored it could not produce a
            /// correct catalog — so it cannot be turned down.
            pub fn floor(self) -> Level {
                match self { $( Lint::$variant => Level::$floor, )* }
            }

            /// The lint named `name`.
            pub fn from_name(name: &str) -> Option<Lint> {
                Lint::ALL.iter().copied().find(|l| l.name() == name)
            }
        }
    };
}

lints! {
    /// A translation has an id the source locale does not.
    ExtraId = ("extra-id", Error, Error);
    /// A translation uses a variable the source message does not declare.
    /// A language that needs more input (grammatical gender, say) gets it by
    /// the *source* declaring it with `.input`, even where its own pattern
    /// ignores it.
    UndeclaredVariable = ("undeclared-variable", Error, Error);
    /// A translation uses a markup name the source message does not.
    UndeclaredMarkup = ("undeclared-markup", Error, Error);
    /// One language's message formats a variable with a date function and
    /// another's shows it bare. Only a date function formats a date — a bare
    /// date/time is a Bad Operand with its fallback (`plan/08` §4.3) — so
    /// one of the two languages would show `{$when}` where the other shows
    /// a date. The variable is compared between each translation and the
    /// source; the date functions are `:datetime`, `:date` and `:time`.
    DateMismatch = ("date-mismatch", Error, Error);
    /// A translation leaves out markup the source message has: a
    /// `{#link}terms{/link}` gone from the French sentence takes the link
    /// away from French readers, and nothing at run time says so. An error by
    /// default, where a dropped placeholder is a warning — a plural variant
    /// routinely drops `{$count}` ("one" says "a message"), and nothing
    /// routinely drops a link; a corpus that drops emphasis on purpose may
    /// turn it down.
    DroppedMarkup = ("dropped-markup", Error, Allow);
    /// A `select` option is not a literal, so nothing can tell which rules a
    /// message selects by until it runs. An error by default: the catalog
    /// then has to carry both plural rule sets, and the message reports a Bad
    /// Option anyway. A corpus that means it may turn it down.
    DynamicSelect = ("dynamic-select", Error, Allow);
    /// A well-known option has a literal value it cannot take. An error by
    /// default, though the runtime would report Bad Option on its own — the
    /// suite has messages that do exactly that on purpose.
    BadOptionValue = ("bad-option-value", Error, Allow);
    /// A function this build has no formatter for: `:number`, `:integer`
    /// and `:offset` without a number formatter on every side the build
    /// formats on, `:percent`, `:currency` and `:unit` without one that
    /// writes the language's own form (`plain` does not), `:datetime`,
    /// `:date` and `:time` without a date formatter on every side
    /// (`plan/08` §3.3; `number` or `datetime` alone is none). A translation
    /// can never silently add formatting code to the wasm. The message names
    /// the features to write, the families of the frameworks that are on,
    /// and what each formatter costs.
    GatedFunction = ("gated-function", Error, Error);
    /// An entry marked `@do-not-translate` differs from the source's.
    DoNotTranslate = ("do-not-translate", Error, Allow);
    /// Two entries of one locale have the same id, so one of them would be
    /// silently dropped.
    DuplicateId = ("duplicate-id", Error, Error);
    /// A file declares an `@locale` other than the directory it sits in.
    /// An error by default — it is nearly always a copy that was never
    /// finished — but a corpus that keeps one locale's files under another
    /// tag on purpose may turn it down.
    LocaleMismatch = ("locale-mismatch", Error, Allow);
    /// A function no registered crate provides. An error by default and
    /// configurable, since custom functions are legal.
    UnknownFunction = ("unknown-function", Error, Allow);

    /// An id the source locale has and a translation does not: it falls back.
    MissingTranslation = ("missing-translation", Warn, Allow);
    /// A placeholder with no function can receive a number, and a side this
    /// build formats on has no number formatter, so a number prints there in
    /// plain digits. The build cannot see what an application passes, so a
    /// placeholder that only receives text raises it too. A side whose
    /// formatter is `plain` chose plain digits, and does not raise it.
    PlainNumbers = ("plain-numbers", Warn, Allow);
    /// A function family is on for this build and no message can use it:
    /// a date formatter (or `datetime` alone) with no `:datetime`, `:date`
    /// or `:time`, or a number formatter (or `number` alone) with
    /// nothing that formats or selects on a number. Raised once per family
    /// for the whole corpus. In a workspace another crate may have turned the
    /// feature on, so the message says "on for this build". A family's
    /// feature on without its framework (`axum-datetime-icu` without `axum`)
    /// raises it too.
    UnusedFeature = ("unused-feature", Warn, Allow);
    /// More than one number formatter, or more than one date formatter, of
    /// one side is on: a build formats with one, the strongest (`builtin`,
    /// then `intl`, then `plain`; ICU4X, then `Intl`, then ISO), and the
    /// others format nothing (`plan/08` §3.2). Raised once per side and
    /// domain for the whole corpus, naming the one that formats.
    SeveralFormatters = ("several-formatters", Warn, Allow);
    /// Markup opened and not closed, or closed and not opened.
    UnpairedMarkup = ("unpaired-markup", Warn, Allow);
    /// A plural `.match` that does not mention every category the *target*
    /// locale has.
    MissingPluralCategory = ("missing-plural-category", Warn, Allow);
    /// Source text that is not in NFC.
    NonNfcSource = ("non-nfc-source", Warn, Allow);
    /// A placeholder the source has and a translation dropped.
    DroppedPlaceholder = ("dropped-placeholder", Warn, Allow);
    /// An option a built-in function does not define — `dateStyle` on
    /// `:datetime`, say. MF2 ignores it, so the message formats as though it
    /// were not there, and nothing at run time says so.
    UnknownOption = ("unknown-option", Warn, Allow);
    /// An id no `tr!` in the workspace names.
    UnusedId = ("unused-id", Warn, Allow);
    /// Unpaired bidi isolates in literal text.
    SuspiciousBidi = ("suspicious-bidi", Warn, Allow);
    /// A `:currency` whose `currency` option is not a literal, so the catalog
    /// must carry every currency CLDR has; listing the codes it can hold in
    /// `mf2.toml` `[locale_data]` carries only those and silences this.
    DynamicCurrency = ("dynamic-currency", Warn, Allow);
    /// The same for `:unit`.
    DynamicUnit = ("dynamic-unit", Warn, Allow);
    /// A variable, option, function, markup or attribute name that is not a
    /// Unicode identifier under MF2's profile of UAX #31, uses a character
    /// UTS #39's General Security Profile does not allow, or mixes scripts —
    /// `syntax.md` asks linters to warn on exactly these.
    NonstandardName = ("nonstandard-name", Warn, Allow);
}

impl fmt::Display for Lint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::{Level, Lint};

    #[test]
    fn names_are_unique_and_kebab_case() {
        let mut names: Vec<&str> = Lint::ALL.iter().map(|l| l.name()).collect();
        names.sort_unstable();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique, "two lints share a name");
        for name in names {
            assert!(
                name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                    && !name.starts_with('-')
                    && !name.ends_with('-'),
                "{name} is not kebab-case"
            );
            assert_eq!(Lint::from_name(name).map(Lint::name), Some(name));
        }
    }

    #[test]
    fn a_default_is_never_below_its_floor() {
        for &lint in Lint::ALL {
            assert!(
                lint.default_level() >= lint.floor(),
                "{lint} defaults below its floor"
            );
        }
    }

    #[test]
    fn a_lint_that_cannot_be_turned_down_is_an_error() {
        for &lint in Lint::ALL {
            if lint.floor() == Level::Error {
                assert_eq!(lint.default_level(), Level::Error, "{lint}");
            }
        }
    }
}
