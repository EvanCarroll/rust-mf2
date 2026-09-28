//! What one frame shows: a trace in progress. Plain data, the same for both
//! renderers; nothing in it is translated.

/// The languages the example ships, in the order the language menu lists
/// them and the benchmark draws them.
pub const LOCALES: [&str; 4] = ["en", "de", "es", "fr"];

/// A trace in progress.
#[derive(Debug)]
pub struct Model {
    pub version: &'static str,
    pub source: &'static str,
    pub destination: &'static str,
    pub protocol: &'static str,
    pub hops: &'static [Hop],
    /// The hop the details panel shows (an index into `hops`).
    pub selected: usize,
    pub flows: &'static [Flow],
    pub sent: usize,
    pub failed: usize,
    pub elapsed_minutes: usize,
    /// How long the display was frozen, in the event log.
    pub frozen_seconds: usize,
    pub dns_pending: usize,
    /// Hops after this one are hidden.
    pub privacy_ttl: u8,
    pub interval_ms: usize,
    pub max_ttl: u8,
    pub timeout_ms: usize,
    pub zoom: usize,
    /// The host the event log could not resolve.
    pub unresolved: &'static str,
    /// The error the event log reports.
    pub error: &'static str,
}

/// One hop of the trace.
#[derive(Debug)]
pub struct Hop {
    pub ttl: u8,
    pub host: Host,
    /// The share of probes lost, 0 to 1.
    pub loss: f64,
    pub sent: usize,
    pub received: usize,
    pub last: f64,
    pub average: f64,
    pub best: f64,
    pub worst: f64,
    pub deviation: f64,
    pub jitter: f64,
    /// The autonomous system, or `None` while the lookup is pending.
    pub asn: Option<(usize, &'static str)>,
    /// City and country, or `None` when unknown.
    pub location: Option<(&'static str, &'static str)>,
    pub addresses: usize,
}

/// What the host column shows for a hop.
#[derive(Debug, Clone, Copy)]
pub enum Host {
    Name(&'static str),
    NoResponse,
    Resolving,
    LookupFailed,
    LookupTimedOut,
    /// After the privacy limit.
    Hidden,
}

/// One flow of the trace.
#[derive(Debug)]
pub struct Flow {
    pub id: usize,
    pub running: bool,
}

impl Model {
    /// The selected hop.
    #[must_use]
    pub fn selected_hop(&self) -> &Hop {
        &self.hops[self.selected]
    }

    /// The share of probes that failed, 0 to 1.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn failure_rate(&self) -> f64 {
        self.failed as f64 / self.sent as f64
    }

    /// The name of the selected hop's host, for the details panel.
    #[must_use]
    pub fn selected_name(&self) -> &'static str {
        match self.selected_hop().host {
            Host::Name(name) => name,
            _ => "",
        }
    }
}

const fn hop(
    ttl: u8,
    host: Host,
    loss: f64,
    received: usize,
    rtt: [f64; 6],
    asn: Option<(usize, &'static str)>,
    location: Option<(&'static str, &'static str)>,
) -> Hop {
    let [last, average, best, worst, deviation, jitter] = rtt;
    Hop {
        ttl,
        host,
        loss,
        sent: 1204,
        received,
        last,
        average,
        best,
        worst,
        deviation,
        jitter,
        asn,
        location,
        addresses: 1,
    }
}

/// The trace every frame draws.
pub static SAMPLE: Model = Model {
    version: "0.2.0",
    source: "192.168.1.10",
    destination: "example.org",
    protocol: "ICMP",
    hops: &[
        hop(
            1,
            Host::Name("router.lan"),
            0.0,
            1204,
            [0.9, 1.1, 0.6, 4.2, 0.4, 0.2],
            Some((64_512, "Home")),
            None,
        ),
        hop(
            2,
            Host::Name("100.64.0.1"),
            0.0,
            1204,
            [8.4, 9.0, 7.7, 21.3, 1.8, 0.9],
            Some((7_922, "Comcast")),
            Some(("Denver", "US")),
        ),
        hop(
            3,
            Host::Resolving,
            0.001,
            1203,
            [9.9, 10.4, 8.8, 24.0, 2.0, 1.1],
            None,
            Some(("Denver", "US")),
        ),
        hop(4, Host::NoResponse, 1.0, 0, [0.0; 6], None, None),
        hop(
            5,
            Host::Name("be-33.core1.den.example.net"),
            0.0,
            1204,
            [12.2, 12.8, 11.5, 30.1, 2.2, 1.0],
            Some((174, "Cogent")),
            Some(("Denver", "US")),
        ),
        hop(
            6,
            Host::LookupFailed,
            0.002,
            1202,
            [25.6, 26.3, 24.9, 48.8, 3.1, 1.6],
            Some((174, "Cogent")),
            Some(("Chicago", "US")),
        ),
        hop(
            7,
            Host::LookupTimedOut,
            0.0,
            1204,
            [31.0, 31.7, 30.2, 55.0, 2.7, 1.2],
            Some((3_356, "Level 3")),
            Some(("Chicago", "US")),
        ),
        hop(8, Host::NoResponse, 1.0, 0, [0.0; 6], None, None),
        hop(
            9,
            Host::Name("ae-1.edge2.nyc.example.net"),
            0.0,
            1204,
            [44.1, 44.9, 43.2, 71.4, 3.3, 1.4],
            Some((3_356, "Level 3")),
            Some(("New York", "US")),
        ),
        hop(
            10,
            Host::Name("example.org"),
            0.0,
            1204,
            [45.0, 45.6, 44.0, 70.2, 3.0, 1.3],
            Some((15_133, "Edgecast")),
            Some(("New York", "US")),
        ),
        hop(
            11,
            Host::Hidden,
            0.0,
            1204,
            [45.2, 45.9, 44.1, 69.8, 3.1, 1.3],
            Some((15_133, "Edgecast")),
            None,
        ),
        hop(
            12,
            Host::Hidden,
            0.0,
            1204,
            [45.3, 46.0, 44.2, 69.9, 3.1, 1.3],
            Some((15_133, "Edgecast")),
            Some(("New York", "US")),
        ),
    ],
    selected: 4,
    flows: &[
        Flow {
            id: 1,
            running: true,
        },
        Flow {
            id: 2,
            running: false,
        },
    ],
    sent: 14_448,
    failed: 2_411,
    elapsed_minutes: 20,
    frozen_seconds: 3,
    dns_pending: 2,
    privacy_ttl: 10,
    interval_ms: 1000,
    max_ttl: 64,
    timeout_ms: 3000,
    zoom: 2,
    unresolved: "unknown.example",
    error: "connection refused",
};

/// A round-trip time as the table shows it, in both renderers.
#[must_use]
pub fn ms(value: f64) -> String {
    format!("{value:.1}")
}

/// A loss as the table shows it, in both renderers.
#[must_use]
pub fn percent(share: f64) -> String {
    format!("{:.1}%", share * 100.0)
}
