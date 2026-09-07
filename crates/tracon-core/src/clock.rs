//! Timestamp formatting shared by the store and its callers. Every stored
//! timestamp is UTC with a fixed millisecond precision and a Z suffix, so
//! plain string comparison orders rows correctly and cutoff strings built
//! here compare cleanly against them.

use std::sync::OnceLock;

use time::format_description::well_known::Rfc3339;
use time::format_description::{self, OwnedFormatItem};
use time::{Duration, OffsetDateTime, Time, UtcOffset};

/// Events stamped further ahead than this are treated as clock errors and
/// re-stamped with the current time.
pub const FUTURE_TOLERANCE: Duration = Duration::minutes(5);

fn stored_format() -> &'static OwnedFormatItem {
    static FORMAT: OnceLock<OwnedFormatItem> = OnceLock::new();
    FORMAT.get_or_init(|| {
        format_description::parse_owned::<2>(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
        )
        .expect("static timestamp format is valid")
    })
}

/// The canonical stored form of an instant: UTC, three fractional digits, Z.
pub fn format_ts(instant: OffsetDateTime) -> String {
    instant
        .to_offset(UtcOffset::UTC)
        .format(stored_format())
        .unwrap_or_default()
}

pub fn now_ts() -> String {
    format_ts(OffsetDateTime::now_utc())
}

/// Bring an incoming timestamp into the stored form. Unparseable strings
/// and timestamps from the future (a skewed agent clock) would sort into
/// the wrong place forever, so they are replaced with `now`.
pub fn normalize_ts(raw: &str, now: OffsetDateTime) -> String {
    let Ok(parsed) = OffsetDateTime::parse(raw, &Rfc3339) else {
        return format_ts(now);
    };
    let too_far_ahead = parsed > now + FUTURE_TOLERANCE;
    if too_far_ahead {
        return format_ts(now);
    }
    format_ts(parsed)
}

/// Bring a caller-supplied cutoff into the stored form so it compares
/// exactly against stored rows: "10:00:00Z" sorts after "10:00:00.000Z"
/// as plain strings and would silently skip rows on that very second.
/// Strings that do not parse are passed through untouched.
pub fn canonical_cutoff(raw: &str) -> String {
    match OffsetDateTime::parse(raw, &Rfc3339) {
        Ok(parsed) => format_ts(parsed),
        Err(_) => raw.to_string(),
    }
}

/// The user's UTC offset right now, or UTC when the platform cannot say
/// (some Unix setups refuse the lookup once the process is multi-threaded).
pub fn local_offset() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}

/// The instant at which "today" began on the user's wall clock, as UTC.
/// A pure function of (now, offset) so the day boundary can be tested
/// without touching the process time zone.
pub fn local_day_start(now: OffsetDateTime, offset: UtcOffset) -> OffsetDateTime {
    now.to_offset(offset)
        .replace_time(Time::MIDNIGHT)
        .to_offset(UtcOffset::UTC)
}

/// Stored-form cutoff for "N minutes ago".
pub fn minutes_ago(minutes: i64) -> String {
    format_ts(OffsetDateTime::now_utc() - Duration::minutes(minutes))
}

/// Stored-form cutoff for "N days ago".
pub fn days_ago(days: i64) -> String {
    format_ts(OffsetDateTime::now_utc() - Duration::days(days))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn stored_form_is_utc_with_three_digits_and_z() {
        let with_offset = datetime!(2026-09-08 12:30:15.5 +02:00);
        assert_eq!(format_ts(with_offset), "2026-09-08T10:30:15.500Z");
        assert_eq!(
            format_ts(datetime!(2026-09-08 00:00:00 UTC)),
            "2026-09-08T00:00:00.000Z"
        );
    }

    #[test]
    fn normalize_rewrites_garbage_and_future_stamps_to_now() {
        let now = datetime!(2026-09-08 10:00:00 UTC);
        assert_eq!(normalize_ts("not a time", now), "2026-09-08T10:00:00.000Z");
        assert_eq!(normalize_ts("", now), "2026-09-08T10:00:00.000Z");
        assert_eq!(
            normalize_ts("2026-09-08T10:06:00Z", now),
            "2026-09-08T10:00:00.000Z"
        );
        // Within tolerance stays as sent.
        assert_eq!(
            normalize_ts("2026-09-08T10:04:00Z", now),
            "2026-09-08T10:04:00.000Z"
        );
    }

    #[test]
    fn normalize_converts_offsets_to_utc() {
        let now = datetime!(2026-09-08 10:00:00 UTC);
        assert_eq!(
            normalize_ts("2026-09-08T11:30:00+02:00", now),
            "2026-09-08T09:30:00.000Z"
        );
        assert_eq!(
            normalize_ts("2026-09-08T09:30:00.123456789Z", now),
            "2026-09-08T09:30:00.123Z"
        );
    }

    #[test]
    fn cutoffs_are_canonicalized_but_garbage_passes_through() {
        assert_eq!(
            canonical_cutoff("2026-09-08T10:00:00Z"),
            "2026-09-08T10:00:00.000Z"
        );
        assert_eq!(canonical_cutoff("later"), "later");
    }

    #[test]
    fn day_starts_at_local_midnight() {
        let now = datetime!(2026-09-08 10:00:00 UTC);
        let plus_two = UtcOffset::from_hms(2, 0, 0).unwrap();
        let start = local_day_start(now, plus_two);
        assert_eq!(format_ts(start), "2026-09-07T22:00:00.000Z");

        let just_after_midnight = datetime!(2026-09-08 00:00:01 +02:00);
        assert!(just_after_midnight >= start);
        let just_before_midnight = datetime!(2026-09-07 23:59:59 +02:00);
        assert!(just_before_midnight < start);
    }

    #[test]
    fn day_start_crosses_the_date_line_westward() {
        let now = datetime!(2026-09-08 03:00:00 UTC);
        let minus_seven = UtcOffset::from_hms(-7, 0, 0).unwrap();
        // 03:00Z is still 20:00 on the 7th at UTC-7.
        assert_eq!(
            format_ts(local_day_start(now, minus_seven)),
            "2026-09-07T07:00:00.000Z"
        );
    }
}
