use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Timestamp helpers (R3.5)
// ============================================================================

/// Return the current time as Unix seconds since the epoch.
pub(super) fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format a Unix timestamp as an ISO 8601 UTC datetime string.
///
/// Uses a simple decomposition that is accurate for any timestamp in the
/// range 1970-2100 without pulling in `chrono` or `time`.
pub(super) fn format_unix_timestamp(secs: u64) -> String {
    // Days since Unix epoch
    let mut days = secs / 86400;
    let time_secs = secs % 86400;
    let hh = time_secs / 3600;
    let mm = (time_secs % 3600) / 60;
    let ss = time_secs % 60;

    // Compute year, month, day using the Gregorian algorithm
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &mdays in &month_days {
        if days < mdays {
            break;
        }
        days -= mdays;
        month += 1;
    }
    let day = days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hh, mm, ss
    )
}

#[inline]
fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
