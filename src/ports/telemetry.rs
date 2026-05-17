//! Telemetry port — OFF by default; only the no-op adapter ships in v0.1.
//!
//! See `docs/NON_GOALS.md` — telemetry is explicitly deferred.

/// Anonymous structured event emitted by use cases.
#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    /// Event name (e.g. `"scaffold.completed"`).
    pub name: &'static str,
    /// Key-value attributes; values are stringly-typed by design.
    pub attrs: Vec<(&'static str, String)>,
}

/// Sink for telemetry events.
pub trait Telemetry: Send + Sync {
    /// Record an event. Must be best-effort: implementations swallow errors.
    fn record(&self, event: TelemetryEvent);
}
