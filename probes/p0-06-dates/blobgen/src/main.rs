//! P0.6 probe: build per-locale ICU4X data blobs restricted to the markers a
//! wasm binary actually requests (`icu::markers_for_bin`), exported from the
//! compiled data that ships inside the ICU4X data crates (CLDR 48.2.1). Nothing
//! is downloaded: the source is `icu_*_data`'s baked tables, made iterable with
//! their `ITER` impl macros.

extern crate alloc;

mod error;

use std::collections::BTreeSet;
use std::fs::File;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use icu_provider::prelude::*;
use icu_provider_export::blob_exporter::BlobExporter;
use icu_provider_export::prelude::*;

use crate::error::Error;

/// The export source: ICU4X compiled data, iterable.
pub struct Src;

const _: () = {
    // The public `impl_*` macros call hidden `__impl_*` macros unqualified.
    use icu_datetime_data::*;
    use icu_decimal_data::*;
    // Baked data refers to the locale fallbacker under this name.
    use icu_locale::fallback as icu_locale_fallback;
    icu_datetime_data::make_provider!(Src);
    icu_datetime_data::impl_datetime_names_dayperiod_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_names_weekday_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_names_month_gregorian_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_names_year_gregorian_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_patterns_date_gregorian_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_patterns_time_v1!(Src, ITER);
    icu_datetime_data::impl_datetime_patterns_glue_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_essentials_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_specific_long_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_specific_short_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_generic_long_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_generic_short_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_standard_long_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_locations_root_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_locations_override_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_cities_root_v1!(Src, ITER);
    icu_datetime_data::impl_timezone_names_cities_override_v1!(Src, ITER);
    // The `ITER` arm of this macro does not compile in icu_time_data 2.3.0
    // (it names `std::collections::BtreeSet`); iterable impl written below.
    icu_time_data::impl_timezone_periods_v1!(Src);
    icu_decimal_data::impl_decimal_symbols_v1!(Src, ITER);
    icu_decimal_data::impl_decimal_digits_v1!(Src, ITER);
};

/// `TimezonePeriodsV1` is a singleton (locale-independent) marker.
impl IterableDataProvider<icu::time::provider::TimezonePeriodsV1> for Src {
    fn iter_ids(&self) -> Result<BTreeSet<DataIdentifierCow<'_>>, DataError> {
        Ok(BTreeSet::from([DataIdentifierCow::default()]))
    }
}

/// `Src` with the skeleton attributes of the pattern markers restricted to a
/// set — models a corpus whose `:date`/`:time` options the build knows. (The
/// export driver's attribute filter cannot do this: these markers have no
/// attribute domain.)
pub struct Filt {
    keep: Option<BTreeSet<String>>,
}

fn is_pattern_marker(m: DataMarkerInfo) -> bool {
    m.id.name().starts_with("DatetimePatterns")
}

macro_rules! sources {
    ($($m:ty),+ $(,)?) => {
        $(
            impl DataProvider<$m> for Filt {
                fn load(&self, req: DataRequest<'_>) -> Result<DataResponse<$m>, DataError> {
                    DataProvider::<$m>::load(&Src, req)
                }
            }
            impl IterableDataProvider<$m> for Filt {
                fn iter_ids(&self) -> Result<BTreeSet<DataIdentifierCow<'_>>, DataError> {
                    let ids = IterableDataProvider::<$m>::iter_ids(&Src)?;
                    Ok(match (&self.keep, is_pattern_marker(<$m>::INFO)) {
                        (Some(keep), true) => ids
                            .into_iter()
                            .filter(|id| keep.contains(&id.marker_attributes.to_string()))
                            .collect(),
                        _ => ids,
                    })
                }
            }
        )+
        icu_provider::export::make_exportable_provider!(Src, [$($m),+,]);
        icu_provider::export::make_exportable_provider!(Filt, [$($m),+,]);
    };
}

sources!(
    icu::datetime::provider::names::DatetimeNamesDayperiodV1,
        icu::datetime::provider::names::DatetimeNamesWeekdayV1,
        icu::datetime::provider::names::DatetimeNamesMonthGregorianV1,
        icu::datetime::provider::names::DatetimeNamesYearGregorianV1,
        icu::datetime::provider::semantic_skeletons::DatetimePatternsDateGregorianV1,
        icu::datetime::provider::semantic_skeletons::DatetimePatternsTimeV1,
        icu::datetime::provider::semantic_skeletons::DatetimePatternsGlueV1,
        icu::datetime::provider::time_zones::TimezoneNamesEssentialsV1,
        icu::datetime::provider::time_zones::TimezoneNamesSpecificLongV1,
        icu::datetime::provider::time_zones::TimezoneNamesSpecificShortV1,
        icu::datetime::provider::time_zones::TimezoneNamesGenericLongV1,
        icu::datetime::provider::time_zones::TimezoneNamesGenericShortV1,
        icu::datetime::provider::time_zones::TimezoneNamesStandardLongV1,
        icu::datetime::provider::time_zones::TimezoneNamesLocationsRootV1,
        icu::datetime::provider::time_zones::TimezoneNamesLocationsOverrideV1,
        icu::datetime::provider::time_zones::TimezoneNamesCitiesRootV1,
        icu::datetime::provider::time_zones::TimezoneNamesCitiesOverrideV1,
        icu::time::provider::TimezonePeriodsV1,
        icu::decimal::provider::DecimalSymbolsV1,
        icu::decimal::provider::DecimalDigitsV1
);


#[derive(Parser)]
#[command(about = "P0.6: per-locale ICU4X blobs restricted to a binary's markers")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the data markers a (wasm) binary requests at runtime.
    Markers { bin: PathBuf },
    /// Print the attribute ids of one marker for one locale (skeleton names).
    Attrs { marker: String, locale: String },
    /// Export one locale's blob with exactly the markers `bin` requests.
    Export {
        #[arg(long)]
        bin: PathBuf,
        #[arg(long)]
        locale: String,
        #[arg(long)]
        out: PathBuf,
        /// Keep only these `datetime` marker attributes (comma-separated) —
        /// models a corpus whose `:date`/`:time` options the build knows.
        #[arg(long)]
        attrs: Option<String>,
        /// Keep only this marker (for per-marker size breakdowns).
        #[arg(long)]
        only: Option<String>,
    },
}

fn markers(bin: &PathBuf) -> Result<BTreeSet<DataMarkerInfo>, Error> {
    let bytes = std::fs::read(bin)?;
    Ok(icu::markers_for_bin(&bytes)?)
}

fn main() -> Result<(), Error> {
    match Cli::parse().cmd {
        Cmd::Markers { bin } => {
            for m in markers(&bin)? {
                println!("{}", m.id.name());
            }
        }
        Cmd::Attrs { marker, locale } => {
            use icu::datetime::provider::semantic_skeletons as sk;
            let loc: DataLocale = locale.parse()?;
            let ids = match marker.as_str() {
                "DatetimePatternsDateGregorianV1" => {
                    IterableDataProvider::<sk::DatetimePatternsDateGregorianV1>::iter_ids(&Src)?
                }
                "DatetimePatternsTimeV1" => {
                    IterableDataProvider::<sk::DatetimePatternsTimeV1>::iter_ids(&Src)?
                }
                "DatetimePatternsGlueV1" => {
                    IterableDataProvider::<sk::DatetimePatternsGlueV1>::iter_ids(&Src)?
                }
                _ => BTreeSet::new(),
            };
            let mut attrs: Vec<String> = ids
                .into_iter()
                .filter(|id| id.locale == loc)
                .map(|id| id.marker_attributes.to_string())
                .collect();
            attrs.sort();
            println!("{}", attrs.join(" "));
            let p = icu::time::provider::TimezonePeriodsV1::INFO;
            println!("TimezonePeriodsV1 singleton={}", p.is_singleton);
            for m in icu_provider::export::ExportableProvider::supported_markers(&Src) {
                println!("{} domain={:?}", m.id.name(), m.attributes_domain);
            }
        }
        Cmd::Export {
            bin,
            locale,
            out,
            attrs,
            only,
        } => {
            let mut wanted = markers(&bin)?;
            if let Some(o) = only {
                wanted.retain(|m| m.id.name() == o);
            }
            let supported = icu_provider::export::ExportableProvider::supported_markers(&Src);
            if let Some(missing) = wanted.iter().find(|m| !supported.contains(m)) {
                return Err(Error::MissingMarker(missing.id.name().to_owned()));
            }
            let loc: DataLocale = locale.parse()?;
            let fallbacker = icu_locale::LocaleFallbacker::new().static_to_owned();
            // `und` too: attribute-keyed, locale-less markers (DecimalDigitsV1,
            // keyed by numbering system) live only there.
            let driver = ExportDriver::new(
                [DataLocaleFamily::single(loc), DataLocaleFamily::single(DataLocale::default())],
                DeduplicationStrategy::None.into(),
                fallbacker,
            )
            .with_markers(wanted);
            let src = Filt {
                keep: attrs.map(|l| l.split(',').map(str::to_owned).collect()),
            };
            let exporter = BlobExporter::new_with_sink(Box::new(File::create(&out)?));
            driver.export(&src, exporter)?;
        }
    }
    Ok(())
}
