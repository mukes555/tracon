pub mod clock;
pub mod event;
pub mod store;

/// Current UTC time in the stored timestamp form (RFC 3339, millisecond
/// precision, Z suffix), the format used across all events.
pub fn now_iso() -> String {
    clock::now_ts()
}
