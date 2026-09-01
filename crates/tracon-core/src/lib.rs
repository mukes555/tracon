pub mod event;
pub mod store;

/// Current UTC time as an RFC 3339 string, the timestamp format used across all events.
pub fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
