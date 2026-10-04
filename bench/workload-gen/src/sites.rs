//! The call-site plan: M sites in the seven positions, spread
//! over K components and R + 1 routes, 21 % of them passing arguments.

use crate::error::Error;
use crate::model::Workload;
use crate::rng::Rng;
use crate::shape::{apportion, share};

/// Share of call sites passing arguments (ppm).
pub const ARG_SITES: u64 = 210_000;

/// Largest number of lazy routes (the router's view tuple stays small).
pub const MAX_ROUTES: usize = 24;

/// Call-site position. `TextProp` and `SignalProp` together are the 8 %
/// "reactive-text prop" shape, split evenly between the two
/// prop types Leptos offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    /// Non-view code needing a `String`: match arms, function returns, error
    /// values set from event handlers (45 %).
    String,
    /// Text child in a view (20 %).
    Child,
    /// HTML attribute value (8 %).
    Attr,
    /// Component prop `#[prop(into)] text: TextProp` (4 %).
    TextProp,
    /// Component prop `#[prop(into)] text: Signal<String>` (4 %).
    SignalProp,
    /// Component prop `#[prop(into)] text: String` (4 %).
    StringProp,
    /// Deferred label: an entry in a `static` table (8 %).
    Deferred,
    /// One branch of an `if`/`else` between two messages in a view (7 %;
    /// every construct counts as two sites).
    IfElse,
}

impl Shape {
    /// Every shape, in template order.
    pub const ALL: [Self; 8] = [
        Self::String,
        Self::Child,
        Self::Attr,
        Self::TextProp,
        Self::SignalProp,
        Self::StringProp,
        Self::Deferred,
        Self::IfElse,
    ];

    /// Key in `template.toml` (`[site.<key>]`) and `sites.json`.
    pub fn key(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Child => "child",
            Self::Attr => "attr",
            Self::TextProp => "text_prop",
            Self::SignalProp => "signal_prop",
            Self::StringProp => "string_prop",
            Self::Deferred => "deferred",
            Self::IfElse => "if_else",
        }
    }

    /// Share of all sites (ppm), of the reference shape.
    pub fn share(self) -> u64 {
        match self {
            Self::String => 450_000,
            Self::Child => 200_000,
            Self::Attr | Self::Deferred => 80_000,
            Self::TextProp | Self::SignalProp | Self::StringProp => 40_000,
            Self::IfElse => 70_000,
        }
    }

    /// Argument modes a site of this shape can have.
    pub fn modes(self) -> &'static [Mode] {
        match self {
            Self::String | Self::StringProp | Self::IfElse => &[Mode::None, Mode::Plain],
            Self::Child => &[Mode::None, Mode::Rich, Mode::Plain, Mode::Signal, Mode::Get],
            Self::Attr | Self::TextProp | Self::SignalProp => {
                &[Mode::None, Mode::Plain, Mode::Signal, Mode::Get]
            }
            Self::Deferred => &[Mode::None],
        }
    }

    fn reactive(self) -> bool {
        matches!(
            self,
            Self::Child | Self::Attr | Self::TextProp | Self::SignalProp
        )
    }
}

/// How a site passes arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// No arguments.
    None,
    /// No arguments; the message has markup (child sites only).
    Rich,
    /// Plain values (`n`, `who`, `when`).
    Plain,
    /// Signal-valued: the signal itself is the argument.
    Signal,
    /// Signal-valued: an enclosing `move ||` closure reads `.get()`.
    Get,
}

impl Mode {
    /// Key in `template.toml` and `sites.json`.
    pub fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Rich => "rich",
            Self::Plain => "plain",
            Self::Signal => "signal",
            Self::Get => "get",
        }
    }

    /// Whether the site passes arguments.
    pub fn has_args(self) -> bool {
        matches!(self, Self::Plain | Self::Signal | Self::Get)
    }
}

/// One call site.
#[derive(Debug, Clone)]
pub struct Site {
    /// Position.
    pub shape: Shape,
    /// Argument mode.
    pub mode: Mode,
    /// Message index.
    pub message: usize,
    /// Component index.
    pub component: usize,
    /// For `IfElse`: the other branch's site index.
    pub partner: Option<usize>,
}

/// All call sites of the app.
#[derive(Debug, Clone)]
pub struct SitePlan {
    /// Sites; site 0 is the canary site.
    pub sites: Vec<Site>,
    /// K.
    pub components: usize,
    /// R.
    pub routes: usize,
}

impl SitePlan {
    /// Route of component `k` (0 = eager home route, 1..=R lazy).
    pub fn route_of(&self, component: usize) -> usize {
        component % (self.routes + 1)
    }

    /// Plans the sites for `wl`.
    pub fn generate(wl: &Workload) -> Result<Self, Error> {
        let knobs = &wl.knobs;
        let m = knobs.sites;
        let k = knobs.components;
        if m < 20 {
            return Err(Error::Knobs("--sites must be at least 20".into()));
        }
        if k == 0 {
            return Err(Error::Knobs("--components must be at least 1".into()));
        }
        if knobs.routes > MAX_ROUTES {
            return Err(Error::Knobs(format!(
                "--routes must be at most {MAX_ROUTES}"
            )));
        }
        let weights: Vec<u64> = Shape::ALL.iter().map(|s| s.share()).collect();
        let mut counts = apportion(m, &weights);
        let child = Shape::ALL
            .iter()
            .position(|&s| s == Shape::Child)
            .expect("child");
        let if_else = Shape::ALL
            .iter()
            .position(|&s| s == Shape::IfElse)
            .expect("if_else");
        if counts[if_else] % 2 == 1 {
            counts[if_else] -= 1;
            counts[child] += 1;
        }
        counts[child] -= 1; // the canary site

        // Units: single sites, or if/else pairs (which must share a component
        // and an argument mode so both branches have one type).
        let mut units: Vec<(Shape, usize)> = Vec::new();
        for (&shape, &count) in Shape::ALL.iter().zip(&counts) {
            if shape == Shape::IfElse {
                units.extend(std::iter::repeat_n((shape, 2), count / 2));
            } else {
                units.extend(std::iter::repeat_n((shape, 1), count));
            }
        }
        let mut rng = Rng::stream(knobs.seed, "units", 0);
        rng.shuffle(&mut units);

        let mut budget = share(m, ARG_SITES).saturating_sub(1);
        let mut reactive_turn = 0usize;
        let mut sites: Vec<Site> = Vec::with_capacity(m);
        let canary = wl
            .messages
            .iter()
            .position(|msg| msg.canary)
            .expect("canary");
        sites.push(Site {
            shape: Shape::Child,
            mode: Mode::Plain,
            message: canary,
            component: 0,
            partner: None,
        });
        for (u, &(shape, size)) in units.iter().enumerate() {
            let args = shape != Shape::Deferred && budget >= size;
            let mode = if !args {
                Mode::None
            } else if shape.reactive() {
                budget -= size;
                reactive_turn += 1;
                [Mode::Plain, Mode::Signal, Mode::Get][(reactive_turn - 1) % 3]
            } else {
                budget -= size;
                Mode::Plain
            };
            let component = (u + 1) % k;
            let first = sites.len();
            for i in 0..size {
                sites.push(Site {
                    shape,
                    mode,
                    message: usize::MAX,
                    component,
                    partner: (size == 2).then_some(if i == 0 { first + 1 } else { first }),
                });
            }
        }

        assign_messages(wl, &mut sites)?;
        Ok(Self {
            sites,
            components: k,
            routes: knobs.routes,
        })
    }
}

fn assign_messages(wl: &Workload, sites: &mut [Site]) -> Result<(), Error> {
    let seed = wl.knobs.seed;
    let pool = |label: &str, keep: &dyn Fn(usize) -> bool| {
        let mut v: Vec<usize> = (0..wl.messages.len()).filter(|&j| keep(j)).collect();
        Rng::stream(seed, label, 0).shuffle(&mut v);
        v
    };
    let msgs = &wl.messages;
    let simple = pool("pool-simple", &|j| {
        msgs[j].vars.is_empty() && msgs[j].markup.is_empty() && !msgs[j].canary
    });
    let markup = pool("pool-markup", &|j| {
        msgs[j].vars.is_empty() && !msgs[j].markup.is_empty()
    });
    let with_args = pool("pool-args", &|j| {
        (1..=3).contains(&msgs[j].vars.len()) && !msgs[j].canary
    });
    if simple.is_empty() || with_args.is_empty() {
        return Err(Error::Knobs("too few messages for the call sites".into()));
    }
    let (mut si, mut mi, mut ai) = (0usize, 0usize, 0usize);
    for site in sites.iter_mut().skip(1) {
        if site.mode.has_args() {
            site.message = with_args[ai % with_args.len()];
            ai += 1;
        } else if site.shape == Shape::Child && mi < markup.len() {
            site.message = markup[mi];
            site.mode = Mode::Rich;
            mi += 1;
        } else {
            site.message = simple[si % simple.len()];
            si += 1;
        }
    }
    Ok(())
}
