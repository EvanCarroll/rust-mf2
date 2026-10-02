//! The system's time zone without an IANA name (plans/19-native-and-terminal.md
//! §5): a zone that follows the system's daylight-saving rules, never the
//! offset in force at start-up, as 1.x froze it. `TZ` is read when the
//! process starts, so the test runs this binary again with
//! `TZ=EST5EDT,M3.2.0,M11.1.0`, the rule of the US Eastern zone, and the
//! child formats instants on both sides of the March change and in winter
//! and summer.

use std::process::Command;
use std::time::{Duration, SystemTime};

use mf2::{CatalogFile, Compiled, Corpus, Dir, Function, IntoArg, Registry, native, tr_args1};

const RULE: &str = "EST5EDT,M3.2.0,M11.1.0";
const CHILD: &str = "MF2_SYSTEM_ZONE_CHILD";

static FUNCTIONS: [(&str, &dyn Function); 1] = [("time", &mf2::fn_datetime::TIME)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

#[test]
fn a_zone_without_a_name_follows_its_rules_across_a_change_of_offset() {
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "in_a_process_whose_zone_is_a_rule",
            "--nocapture",
        ])
        .env("TZ", RULE)
        .env(CHILD, "1")
        .status()
        .expect("the test binary runs again");
    assert!(
        status.success(),
        "the child's assertions failed: see its output above"
    );
}

/// Runs only in the child that the test above starts.
#[test]
fn in_a_process_whose_zone_is_a_rule() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let compiled = mf2::compile_str("{$t :time timePrecision=minute}", "en").expect("compiles");
    let bytes: &'static [u8] = Box::leak(compiled.catalog.as_bytes().to_vec().into_boxed_slice());
    let files: &'static [CatalogFile] =
        Box::leak(Box::new([CatalogFile::new("en", "en.mf2b", Some(bytes))]));
    let corpus: &'static Corpus = Box::leak(Box::new(
        Corpus::new(
            "en",
            compiled.catalog.manifest_hash(),
            &[("en", Dir::Ltr)],
            &REGISTRY,
            files,
        )
        .with_host(&mf2::host_std::ZONES_HOST),
    ));
    native::install(corpus);
    assert_eq!(
        format!("{:?}", native::time_zone()),
        "TimeZone(Rules(\"EST5EDT,M3.2.0,M11.1.0\"))"
    );
    let at = |seconds: u64| {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        tr_args1(Compiled::ID, t.into_arg()).to_string()
    };
    // A frozen offset would show one of each pair an hour off: 10:30 in
    // January for summer's -4, 10:30 in July for winter's -5.
    for (seconds, shown) in [
        (1_768_487_400, "9:30"), // 2026-01-15 14:30Z: 09:30 EST
        (1_784_122_200, "9:30"), // 2026-07-15 13:30Z: 09:30 EDT
        (1_772_951_400, "1:30"), // 2026-03-08 06:30Z: 01:30 EST, before the change
        (1_772_955_000, "3:30"), // 2026-03-08 07:30Z: 03:30 EDT, after it
    ] {
        let text = at(seconds);
        assert!(text.contains(shown), "{seconds}: {text:?}, not {shown}");
        assert!(
            !text.contains("10:30") && !text.contains("2:30"),
            "{seconds}: {text:?}"
        );
    }
}
