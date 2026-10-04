//! The formatter cache (`cache`; `plan/08` §5.2): [`CachedBlob`]'s data.
//!
//! Without it every date placeholder copies the catalog's `icu.blob` into a
//! provider and builds its formatter, once for `supports` and once for
//! `format`. With it each thread keeps, per blob, the provider, the IANA
//! parser (built on the first zone style) and the formatters it built, one
//! per language, shape and variant; `supports` and `format` share them.
//!
//! A catalog is known by its blob's bytes, never by their address: a
//! catalog dropped and another loaded at the same address with other bytes
//! gets its own provider. Two catalogs with the same blob share one, which
//! formats the same text. Per thread because ICU4X's data is neither `Send`
//! nor `Sync` here; hence `std`. Both lists are bounded, the oldest entry
//! going first.
//!
//! Panic-free: a thread-local already destroyed, or a call made while the
//! cache is borrowed (a sink that formats a date), formats as [`Blob`] does.
//!
//! [`CachedBlob`]: super::CachedBlob
//! [`Blob`]: super::Blob

extern crate std;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::any::Any;
use core::cell::RefCell;

use icu_provider_blob::BlobDataProvider;
use icu_time::zone::iana::IanaParser;
use mf2_runtime::{
    DateStyle, DateTimeOptions, FnContext, FormatError, Sink, TimePrecision, ZoneStyle,
};

use super::{Blob, Data, Source, Variant, blob, iana, provider};
use crate::plan::Plan;

/// The blobs a thread keeps (a server loads one catalog per language; a
/// reload replaces one).
const CATALOGS: usize = 16;

/// The formatters kept per blob (the shapes of a corpus, in its language).
const FORMATTERS: usize = 64;

std::thread_local! {
    static CACHE: RefCell<VecDeque<Catalog>> = const { RefCell::new(VecDeque::new()) };
}

/// [`CachedBlob`](super::CachedBlob)'s [`Data::run`]: formats through the
/// thread's cache.
pub(super) fn run<V: Variant>(
    cx: &FnContext<'_>,
    plan: &Plan<'_>,
    mut out: Option<&mut dyn Sink>,
) -> Result<(), FormatError> {
    let bytes = blob(cx)?;
    let cached = CACHE.try_with(|cache| {
        let mut cache = cache.try_borrow_mut().ok()?;
        Some(
            Catalog::find(&mut cache, bytes)
                .and_then(|catalog| catalog.run::<V>(cx.locale(), plan, reborrow(&mut out))),
        )
    });
    match cached {
        Ok(Some(result)) => result,
        _ => Blob::run::<V>(cx, plan, out),
    }
}

/// `out` for one call, leaving it for the fallback.
fn reborrow<'r>(out: &'r mut Option<&mut dyn Sink>) -> Option<&'r mut dyn Sink> {
    match out {
        Some(sink) => {
            let sink: &mut dyn Sink = &mut **sink;
            Some(sink)
        }
        None => None,
    }
}

/// One blob's provider and what was built from it.
struct Catalog {
    /// The bytes the provider was built from: the key.
    blob: Box<[u8]>,
    provider: BlobDataProvider,
    /// Built on the first plan with a zone style.
    zones: Option<IanaParser>,
    formatters: VecDeque<Formatter>,
}

impl Catalog {
    /// The entry of `bytes` in `cache`, made (and the oldest dropped) if
    /// there is none.
    fn find<'c>(
        cache: &'c mut VecDeque<Catalog>,
        bytes: &[u8],
    ) -> Result<&'c mut Catalog, FormatError> {
        if let Some(i) = cache.iter().position(|c| *c.blob == *bytes) {
            return cache.get_mut(i).ok_or(FormatError::UnsupportedOperation);
        }
        let provider = provider(bytes)?;
        if cache.len() >= CATALOGS {
            cache.pop_front();
        }
        cache.push_back(Catalog {
            blob: Box::from(bytes),
            provider,
            zones: None,
            formatters: VecDeque::new(),
        });
        cache.back_mut().ok_or(FormatError::UnsupportedOperation)
    }

    /// Formats `plan` with variant `V` in `locale`, building what is
    /// missing. A formatter that fails to build is not kept.
    fn run<V: Variant>(
        &mut self,
        locale: &str,
        plan: &Plan<'_>,
        out: Option<&mut dyn Sink>,
    ) -> Result<(), FormatError> {
        let o = plan.options;
        let src = Source::Buffer(&self.provider);
        let found = self.formatters.iter().position(|f| f.is::<V>(locale, o));
        let entry = match found {
            Some(i) => self.formatters.get(i),
            None => {
                let built = V::build(&src, locale, o)?;
                if self.formatters.len() >= FORMATTERS {
                    self.formatters.pop_front();
                }
                self.formatters
                    .push_back(Formatter::new(locale, o, Box::new(built)));
                self.formatters.back()
            }
        };
        let f = entry
            .and_then(|e| e.formatter.downcast_ref::<V::Formatter>())
            .ok_or(FormatError::UnsupportedOperation)?;
        let zones = match o.time_zone_style {
            Some(_) => {
                if self.zones.is_none() {
                    self.zones = Some(iana(&src)?);
                }
                self.zones.as_ref()
            }
            None => None,
        };
        V::write(f, plan, zones, out)
    }
}

/// A formatter and what it was built for: the language and the options
/// that reach ICU4X's formatter (the zone itself is the input's).
struct Formatter {
    locale: Box<str>,
    date: Option<DateStyle>,
    time: Option<TimePrecision>,
    time_zone_style: Option<ZoneStyle>,
    hour12: Option<bool>,
    calendar: Option<Box<str>>,
    /// A variant's [`Variant::Formatter`].
    formatter: Box<dyn Any>,
}

impl Formatter {
    fn new(locale: &str, o: &DateTimeOptions<'_>, formatter: Box<dyn Any>) -> Formatter {
        Formatter {
            locale: Box::from(locale),
            date: o.date,
            time: o.time,
            time_zone_style: o.time_zone_style,
            hour12: o.hour12,
            calendar: o.calendar.map(Box::from),
            formatter,
        }
    }

    /// Whether this is variant `V`'s formatter of `locale` and the shape of
    /// `o`.
    fn is<V: Variant>(&self, locale: &str, o: &DateTimeOptions<'_>) -> bool {
        self.formatter.is::<V::Formatter>()
            && *self.locale == *locale
            && self.date == o.date
            && self.time == o.time
            && self.time_zone_style == o.time_zone_style
            && self.hour12 == o.hour12
            && self.calendar.as_deref() == o.calendar
    }
}
