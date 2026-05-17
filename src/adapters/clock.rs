//! Clock adapters.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::ports::clock::Clock;

/// System wall-clock adapter. Uses UTC.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// Construct.
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        // Avoid pulling `chrono`/`time` for one timestamp: format manually
        // as `YYYY-MM-DDTHH:MM:SSZ` from the epoch seconds.
        format_unix_secs(now.as_secs())
    }
}

/// Format a UNIX-epoch second count as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Algorithm cribbed from the public-domain "civil_from_days" by Howard
/// Hinnant; correct for any signed 64-bit second count.
fn format_unix_secs(secs: u64) -> String {
    let secs_per_day: u64 = 86_400;
    let days = (secs / secs_per_day) as i64;
    let rem = secs % secs_per_day;
    let h = rem / 3_600;
    let m = (rem % 3_600) / 60;
    let s = rem % 60;

    // Days since 1970-01-01 → civil date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y_int = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m_civ = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m_civ <= 2 { y_int + 1 } else { y_int };

    format!(
        "{y:04}-{m_civ:02}-{d:02}T{h:02}:{m:02}:{s:02}Z",
        y = y,
        m_civ = m_civ,
        d = d,
        h = h,
        m = m,
        s = s,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch_zero() {
        assert_eq!(format_unix_secs(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn formats_y2k() {
        // 2000-01-01T00:00:00Z is 946_684_800 seconds since epoch.
        assert_eq!(format_unix_secs(946_684_800), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn now_round_trip_smoke() {
        let s = SystemClock.now_rfc3339();
        assert!(s.ends_with('Z'));
        assert_eq!(s.len(), 20);
    }
}
