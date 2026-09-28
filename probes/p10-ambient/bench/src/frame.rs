//! One frame of the trippy-shaped UI: all 112 messages of the corpus, each
//! built as `tr!` builds it and handed to `emit!` as `plain` (text) or
//! `rich` (markup, to a styled `Line`). Written once; each variant defines
//! its own `emit!` before expanding `frame!`, so every variant's frame is
//! monomorphic code over the same calls.

/// The values a frame formats — what a live trace would show.
pub struct State {
    pub host: &'static str,
    pub addr: &'static str,
    pub iface: &'static str,
    pub user: &'static str,
    pub state: &'static str,
    pub msg: &'static str,
    pub path: &'static str,
    pub name: &'static str,
    pub city: &'static str,
    pub country: &'static str,
    pub what: &'static str,
    pub version: &'static str,
    pub detail: &'static str,
    pub round: u32,
    pub sent: u32,
    pub hosts: u32,
    pub flows: u32,
    pub errors: u32,
    pub hops: u32,
    pub ttl: u32,
    pub port: u32,
    pub asn: u32,
    pub bytes: u32,
    pub ago: u32,
    pub secs: f64,
    pub rate: f64,
    pub pct: f64,
    pub ms: f64,
}

pub const STATE: State = State {
    host: "example.org",
    addr: "93.184.215.14",
    iface: "eth0",
    user: "root",
    state: "Running",
    msg: "the route changed",
    path: "10.0.0.1 → 93.184.215.14",
    name: "EDGECAST",
    city: "Los Angeles",
    country: "US",
    what: "the report",
    version: "0.13.0",
    detail: "raw sockets need CAP_NET_RAW",
    round: 42,
    sent: 1204,
    hosts: 17,
    flows: 3,
    errors: 0,
    hops: 12,
    ttl: 64,
    port: 33434,
    asn: 15133,
    bytes: 1500,
    ago: 1,
    secs: 83.5,
    rate: 9.75,
    pct: 12.5,
    ms: 23.456,
};

/// Every message of the corpus, once: 97 `plain`, 15 `rich`.
macro_rules! frame {
    ($st:expr) => {{
        use probe_i18n::tr;
        let st: &$crate::frame::State = $st;
        emit!(plain, tr!("app.title"));
        emit!(plain, tr!("app.subtitle"));
        emit!(plain, tr!("tab.hops"));
        emit!(plain, tr!("tab.chart"));
        emit!(plain, tr!("tab.map"));
        emit!(plain, tr!("tab.flows"));
        emit!(plain, tr!("tab.settings"));
        emit!(plain, tr!("tab.help"));
        emit!(plain, tr!("col.hop"));
        emit!(plain, tr!("col.host"));
        emit!(plain, tr!("col.loss"));
        emit!(plain, tr!("col.sent"));
        emit!(plain, tr!("col.recv"));
        emit!(plain, tr!("col.last"));
        emit!(plain, tr!("col.avg"));
        emit!(plain, tr!("col.best"));
        emit!(plain, tr!("col.worst"));
        emit!(plain, tr!("col.stddev"));
        emit!(plain, tr!("col.status"));
        emit!(plain, tr!("col.jitter"));
        emit!(plain, tr!("col.javg"));
        emit!(plain, tr!("col.jmax"));
        emit!(plain, tr!("col.jint"));
        emit!(plain, tr!("col.asn"));
        emit!(plain, tr!("col.country"));
        emit!(plain, tr!("col.dest"));
        emit!(plain, tr!("col.port"));
        emit!(plain, tr!("col.proto"));
        emit!(plain, tr!("col.target"));
        emit!(plain, tr!("col.flow"));
        emit!(plain, tr!("label.target"));
        emit!(plain, tr!("label.protocol"));
        emit!(plain, tr!("label.family"));
        emit!(plain, tr!("label.privileged"));
        emit!(plain, tr!("label.unprivileged"));
        emit!(plain, tr!("label.status"));
        emit!(plain, tr!("label.running"));
        emit!(plain, tr!("label.paused"));
        emit!(plain, tr!("label.frozen"));
        emit!(plain, tr!("label.completed"));
        emit!(plain, tr!("label.awaiting"));
        emit!(plain, tr!("label.no-response"));
        emit!(plain, tr!("label.unknown"));
        emit!(plain, tr!("label.ipv4"));
        emit!(plain, tr!("label.ipv6"));
        emit!(plain, tr!("label.icmp"));
        emit!(plain, tr!("label.udp"));
        emit!(plain, tr!("label.tcp"));
        emit!(plain, tr!("settings.title"));
        emit!(plain, tr!("settings.tui"));
        emit!(plain, tr!("settings.trace"));
        emit!(plain, tr!("settings.dns"));
        emit!(plain, tr!("settings.geoip"));
        emit!(plain, tr!("settings.bindings"));
        emit!(plain, tr!("settings.theme"));
        emit!(plain, tr!("settings.columns"));
        emit!(plain, tr!("help.title"));
        emit!(plain, tr!("help.body"));
        emit!(plain, tr!("lang.en"));
        emit!(plain, tr!("lang.fr"));
        emit!(plain, tr!("lang.de"));
        emit!(plain, tr!("lang.menu"));
        emit!(plain, tr!("lang.quit"));
        emit!(plain, tr!("status.target", host = st.host));
        emit!(plain, tr!("status.round", n = st.round));
        emit!(plain, tr!("status.elapsed", secs = st.secs));
        emit!(plain, tr!("status.rate", rate = st.rate));
        emit!(plain, tr!("status.bytes", n = st.bytes));
        emit!(plain, tr!("status.ttl", ttl = st.ttl));
        emit!(plain, tr!("status.port", port = st.port));
        emit!(plain, tr!("status.src", addr = st.addr));
        emit!(plain, tr!("status.dest", addr = st.addr));
        emit!(plain, tr!("status.iface", iface = st.iface));
        emit!(plain, tr!("status.user", user = st.user));
        emit!(plain, tr!("status.probes", n = st.sent));
        emit!(plain, tr!("status.hosts", n = st.hosts));
        emit!(plain, tr!("status.flows", n = st.flows));
        emit!(plain, tr!("status.rounds", n = st.round));
        emit!(plain, tr!("status.errors", n = st.errors));
        emit!(plain, tr!("status.hops", n = st.hops));
        emit!(plain, tr!("status.timeouts", n = st.errors));
        emit!(plain, tr!("status.replies", n = st.sent));
        emit!(plain, tr!("status.ago", n = st.ago));
        emit!(plain, tr!("status.addrs", n = st.hosts));
        emit!(plain, tr!("host.addr", host = st.host, addr = st.addr));
        emit!(plain, tr!("host.asn", asn = st.asn, name = st.name));
        emit!(plain, tr!("host.geo", city = st.city, country = st.country));
        emit!(plain, tr!("row.loss", pct = st.pct));
        emit!(plain, tr!("row.ms", ms = st.ms));
        emit!(plain, tr!("err.resolve", host = st.host));
        emit!(plain, tr!("err.permission", detail = st.detail));
        emit!(plain, tr!("err.socket", detail = st.detail));
        emit!(plain, tr!("info.saved", path = st.path));
        emit!(plain, tr!("info.copied", what = st.what));
        emit!(plain, tr!("info.version", version = st.version));
        emit!(plain, tr!("info.locale", name = st.name));
        emit!(plain, tr!("chart.axis", ms = st.hops));
        emit!(rich, tr!("status.connected", host = st.host));
        emit!(rich, tr!("status.line", state = st.state, host = st.host, n = st.sent));
        emit!(rich, tr!("status.warn", msg = st.msg));
        emit!(rich, tr!("status.error", msg = st.msg));
        emit!(rich, tr!("hint.help"));
        emit!(rich, tr!("hint.quit"));
        emit!(rich, tr!("hint.freeze"));
        emit!(rich, tr!("hint.chart"));
        emit!(rich, tr!("hint.map"));
        emit!(rich, tr!("hint.settings"));
        emit!(rich, tr!("hint.language"));
        emit!(rich, tr!("hint.bar"));
        emit!(rich, tr!("hint.keys"));
        emit!(rich, tr!("title.trace", host = st.host));
        emit!(rich, tr!("flow.item", n = st.flows, path = st.path));
    }};
}

/// The messages a frame formats.
pub const MESSAGES: usize = 112;
