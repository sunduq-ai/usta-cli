//! Clock port — testability hook for "now".

/// Source of wall-clock time. Tests inject a fixed clock; production uses the
/// system clock.
pub trait Clock: Send + Sync {
    /// Returns the current time as RFC 3339.
    fn now_rfc3339(&self) -> String;
}
