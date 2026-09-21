//! The generator's knobs (plans/06-size-and-perf.md §2: N, M, L, R, K, seed).

use crate::shape::target;

/// The seed of the committed corpus (`bench/corpora/workload-1600.json`).
pub const DEFAULT_SEED: u64 = 1;

/// Default number of components the call sites are spread over.
pub const DEFAULT_COMPONENTS: usize = 60;

/// Default number of lazy routes.
pub const DEFAULT_ROUTES: usize = 3;

/// Every size knob of the workload. The defaults reproduce the reference
/// workload of plans/06 §2.
#[derive(Debug, Clone, PartialEq, Eq, clap::Args)]
pub struct Knobs {
    /// Random seed; the same seed and knobs give byte-identical output.
    #[arg(long, default_value_t = DEFAULT_SEED)]
    pub seed: u64,

    /// N: messages per locale.
    #[arg(short = 'n', long, default_value_t = target::MESSAGES)]
    pub messages: usize,

    /// M: call sites in the generated app.
    #[arg(short = 'm', long, default_value_t = target::SITES)]
    pub sites: usize,

    /// L: real locales, counting the source locale `en` (pseudo-locales are extra).
    #[arg(short = 'l', long, default_value_t = 2)]
    pub locales: usize,

    /// Leave out the pseudo-locales `en-XA` and `ar-XB`.
    #[arg(long)]
    pub no_pseudo: bool,

    /// R: lazy (`#[lazy_route]`) routes, besides the eager home route.
    #[arg(short = 'r', long, default_value_t = DEFAULT_ROUTES)]
    pub routes: usize,

    /// K: components the call sites are spread over.
    #[arg(short = 'k', long, default_value_t = DEFAULT_COMPONENTS)]
    pub components: usize,

    /// Source files per locale.
    #[arg(long, default_value_t = target::FILES)]
    pub files: usize,

    /// Messages that format a variable with `:number` (taken from the
    /// variable-carrying messages; the variable-count shape is unchanged).
    #[arg(long, default_value_t = 0)]
    pub number: usize,

    /// Messages that format a variable with `:datetime`.
    #[arg(long, default_value_t = 0)]
    pub datetime: usize,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            messages: target::MESSAGES,
            sites: target::SITES,
            locales: 2,
            no_pseudo: false,
            routes: DEFAULT_ROUTES,
            components: DEFAULT_COMPONENTS,
            files: target::FILES,
            number: 0,
            datetime: 0,
        }
    }
}

impl Knobs {
    /// One line describing the knobs, written into generated output.
    pub fn summary(&self) -> String {
        format!(
            "seed={} messages={} sites={} locales={} pseudo={} routes={} components={} files={} number={} datetime={}",
            self.seed,
            self.messages,
            self.sites,
            self.locales,
            !self.no_pseudo,
            self.routes,
            self.components,
            self.files,
            self.number,
            self.datetime
        )
    }
}
