//! The `timeZone` semantics (datetime.md, "Date and Time Override
//! Options"): which zone an expression shows its value in, and the value
//! moved there — a floating value takes the zone without conversion, a
//! value with an offset or a zone is converted, named zones through
//! [`Host::zone_offset`].
//!
//! **Invariant**: a value a date/time function resolved is *in* the zone its
//! `options.time_zone` names — `None` being the formatting context's zone.
//! So a later expression that inherits the option has nothing to convert,
//! and one with its own `timeZone` converts from where the value is.

use mf2_runtime::{DateTime, DateTimeOptions, ErrorSink, FnContext, FormatError, Host, ZoneOption};

/// Milliseconds per day.
const DAY_MS: i64 = 86_400_000;

/// The wall time of `d`, in milliseconds since the epoch, read as UTC.
pub(crate) fn wall_ms(d: &DateTime<'_>) -> i64 {
    d.date.days_since_epoch() * DAY_MS + d.time.ms_of_day()
}

/// `zone`'s offset at the instant `t`, if the host knows it (and it is
/// less than a day).
fn offset_at(host: &dyn Host, zone: &str, t: i64) -> Option<i32> {
    host.zone_offset(zone, t)
        .filter(|o| o.unsigned_abs() < 86_400)
}

/// The instant a wall time `wall` (read as UTC) is in the named `zone`,
/// and the zone's offset there. The offsets a day before and a day after
/// bracket any transition near it (real zones change at most once in two
/// days); an offset `o` fits when the zone has it at `wall - o`. Of two
/// that fit (an overlap: the clock set back) the larger, the earlier
/// instant; if none fits (a gap: the clock set forward) the offset before
/// the gap, so the wall time moves forward by the gap's length — java.time's
/// and Temporal's `compatible` choice. `None`: the host has no data for
/// `zone`. At most six host calls.
pub(crate) fn search(host: &dyn Host, zone: &str, wall: i64) -> Option<(i64, i32)> {
    let fits = |o: i32| offset_at(host, zone, wall - i64::from(o) * 1000) == Some(o);
    let before = offset_at(host, zone, wall - DAY_MS)?;
    let after = offset_at(host, zone, wall + DAY_MS)?;
    let (hi, lo) = if before >= after {
        (before, after)
    } else {
        (after, before)
    };
    let chosen = if fits(hi) {
        hi
    } else if hi != lo && fits(lo) {
        lo
    } else if hi == lo {
        // Two transitions inside the window: the offset in between, if it fits.
        let mid = offset_at(host, zone, wall - i64::from(before) * 1000)?;
        if fits(mid) { mid } else { before }
    } else {
        before
    };
    let instant = wall - i64::from(chosen) * 1000;
    Some((instant, offset_at(host, zone, instant)?))
}

/// Where a value's wall time is.
enum At<'s> {
    /// Nowhere yet: floating.
    Floating,
    /// At a known UTC offset: an instant.
    Offset(i32),
    /// In a named zone whose offset is not known (the host has no data).
    Named(&'s str),
}

/// Whether `d` is a value a date/time function resolved into the
/// formatting context's zone (the invariant above).
fn in_context(d: &DateTime<'_>) -> bool {
    d.options.is_resolved() && d.options.time_zone.is_none()
}

/// Where `d` is: at its offset, else in its zone, else — resolved into the
/// formatting context's zone without learning its offset — in `context`,
/// else floating.
fn at<'s>(d: &DateTime<'s>, context: ZoneOption<'s>) -> At<'s> {
    match (d.offset, d.zone) {
        (Some(o), _) => At::Offset(o),
        (None, Some(z)) => At::Named(z),
        (None, None) if in_context(d) => match context {
            ZoneOption::Offset(o) => At::Offset(o),
            ZoneOption::Named(n) => At::Named(n),
            ZoneOption::Utc | ZoneOption::Input => At::Offset(0),
        },
        (None, None) => At::Floating,
    }
}

/// The named zone `d` is in, if any (see [`at`]).
fn zone_name<'s>(d: &DateTime<'s>, context: ZoneOption<'s>) -> Option<&'s str> {
    match (d.zone, context) {
        (Some(z), _) => Some(z),
        (None, ZoneOption::Named(n)) if in_context(d) => Some(n),
        _ => None,
    }
}

/// What `timeZone=input` means for a value.
enum Input<'a> {
    /// Its zone, as a `timeZone` value.
    Zone(ZoneOption<'a>),
    /// The formatting context's zone, which a date/time function put it in.
    Context,
    /// It has none: floating.
    Floating,
}

/// `timeZone=input` for `d`: the zone it is in.
fn own_zone<'a>(d: &DateTime<'a>) -> Input<'a> {
    if d.options.is_resolved() {
        match d.options.time_zone {
            None => return Input::Context,
            Some(ZoneOption::Input) => {}
            Some(z) => return Input::Zone(z),
        }
    }
    match (d.zone, d.offset) {
        (Some(z), _) => Input::Zone(ZoneOption::Named(z)),
        (None, Some(0)) => Input::Zone(ZoneOption::Utc),
        (None, Some(o)) => Input::Zone(ZoneOption::Offset(o)),
        (None, None) => Input::Floating,
    }
}

/// The value at the instant `t`, shown at `offset`, in `zone`.
fn moved<'a>(
    t: i64,
    offset: i32,
    zone: Option<&'a str>,
    options: DateTimeOptions<'a>,
) -> Result<DateTime<'a>, FormatError> {
    let mut d = DateTime::from_epoch_ms(t + i64::from(offset) * 1000)
        .ok_or(FormatError::BadOperand)?
        .with_offset(offset)
        .ok_or(FormatError::BadOption)?;
    d.zone = zone;
    d.options = options;
    Ok(d)
}

/// Moves `d` into the zone `option` names — the expression's own
/// `timeZone`, else the operand's (inherited), else (`None`) the formatting
/// context's — and returns the `timeZone` the resolved value records (the
/// zone it is now in; `None`: the context's).
///
/// * `input` is the operand's own zone; on a floating operand it is a
///   *Bad Operand*, and the context's zone is used.
/// * A floating value takes the zone without conversion; placed in a named
///   zone, its offset comes from [`search`] (a wall time in a gap moves
///   forward), and stays unknown if the host has no zone data — which is no
///   error: the wall time is what shows.
/// * A value in another zone is converted: to UTC or an offset by
///   arithmetic, to a named zone through `Host::zone_offset`. When the
///   instant or the target's offset cannot be known — the host has no data
///   for a named zone involved — the conversion the spec says SHOULD happen
///   cannot: *Bad Option* and a fallback value (`Err`), as datetime.md
///   allows, rather than a wall time shown in the wrong zone.
/// * A result past `Date`'s year limit: *Bad Operand*, fallback.
pub(crate) fn place<'a>(
    cx: &FnContext<'_>,
    d: &mut DateTime<'a>,
    option: Option<ZoneOption<'a>>,
    errs: &mut dyn ErrorSink,
) -> Result<Option<ZoneOption<'a>>, FormatError> {
    let host = cx.host();
    let context = cx.time_zone().as_option();
    let (target, stored) = match option {
        Some(ZoneOption::Input) => match own_zone(d) {
            Input::Zone(z) => (z, Some(z)),
            Input::Context => (context, None),
            Input::Floating => {
                errs.error(FormatError::BadOperand);
                (context, None)
            }
        },
        Some(z) => (z, Some(z)),
        None => (context, None),
    };
    // The zone name the value keeps (the context's is re-read when it
    // formats: it does not live as long as the value).
    let keep = match stored {
        Some(ZoneOption::Named(n)) => Some(n),
        _ => None,
    };
    let wall = wall_ms(d);
    if let ZoneOption::Named(n) = target
        && zone_name(d, context) == Some(n)
    {
        // Already there; learn the offset if the host can tell. A value with
        // an offset is an instant, and its wall time is the zone's only at
        // the zone's offset: an instant labelled with a zone it was never
        // converted to (`DateTime::in_zone` on a UTC instant) is moved to it.
        match d.offset {
            None => {
                if let Some((t, o)) = search(host, n, wall) {
                    *d = moved(t, o, d.zone, d.options)?;
                }
            }
            Some(o) => {
                let t = wall - i64::from(o) * 1000;
                if let Some(at) = offset_at(host, n, t)
                    && at != o
                {
                    *d = moved(t, at, d.zone, d.options)?;
                }
            }
        }
        if keep.is_some() {
            d.zone = keep;
        }
        return Ok(stored);
    }
    let instant = match at(d, context) {
        At::Floating => {
            match target {
                ZoneOption::Offset(o) => d.offset = Some(o),
                ZoneOption::Named(n) => match search(host, n, wall) {
                    Some((t, o)) => *d = moved(t, o, None, d.options)?,
                    None => d.offset = None,
                },
                ZoneOption::Utc | ZoneOption::Input => d.offset = Some(0),
            }
            d.zone = keep;
            return Ok(stored);
        }
        At::Offset(o) => wall - i64::from(o) * 1000,
        At::Named(z) => search(host, z, wall).ok_or(FormatError::BadOption)?.0,
    };
    let offset = match target {
        ZoneOption::Offset(o) => o,
        ZoneOption::Named(n) => offset_at(host, n, instant).ok_or(FormatError::BadOption)?,
        ZoneOption::Utc | ZoneOption::Input => 0,
    };
    *d = moved(instant, offset, keep, d.options)?;
    Ok(stored)
}
