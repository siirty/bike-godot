use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Where "now" comes from. Injected so tests can order saves without sleeping.
pub trait Clock {
    /// The current time.
    fn now(&self) -> SystemTime;
}

/// The real wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

impl<F: Fn() -> SystemTime> Clock for F {
    fn now(&self) -> SystemTime {
        self()
    }
}

/// `YYYY-MM-DDTHH:MM:SS.ffffffZ`, UTC, microsecond precision. Fixed width, so text order is
/// time order: two saves in the same second still sort correctly.
pub fn format_timestamp(t: SystemTime) -> String {
    let micros = t
        .duration_since(UNIX_EPOCH)
        .expect("timestamps are after 1970")
        .as_micros();
    let secs = (micros / 1_000_000) as i64;
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:06}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60,
        micros % 1_000_000
    )
}

/// The inverse of [`format_timestamp`]. `None` for anything else.
pub fn parse_timestamp(text: &str) -> Option<SystemTime> {
    let b = text.as_bytes();
    if b.len() != 27
        || &text[4..5] != "-"
        || &text[7..8] != "-"
        || &text[10..11] != "T"
        || &text[19..20] != "."
        || &text[26..] != "Z"
    {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = &text[r];
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())?
    };
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, s, us) = (n(11..13)?, n(14..16)?, n(17..19)?, n(20..26)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    let secs = days * 86_400 + h * 3600 + mi * 60 + s;
    let t = UNIX_EPOCH
        + Duration::from_secs(u64::try_from(secs).ok()?)
        + Duration::from_micros(us as u64);
    // Reject impossible dates (Feb 30) by round-tripping.
    (format_timestamp(t) == text).then_some(t)
}

// Howard Hinnant's civil-date algorithms.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants_format_as_utc_iso8601() {
        assert_eq!(format_timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000000Z");
        // 2000-02-29T12:34:56.789012Z: a leap day in a century leap year.
        let t = UNIX_EPOCH + Duration::from_micros(951_827_696_789_012);
        assert_eq!(format_timestamp(t), "2000-02-29T12:34:56.789012Z");
        assert_eq!(parse_timestamp("2000-02-29T12:34:56.789012Z"), Some(t));
    }

    #[test]
    fn round_trips_and_rejects_impossible_dates() {
        for micros in [
            0u64,
            1,
            86_399_999_999,
            1_790_000_000_123_456,
            4_102_444_800_000_000,
        ] {
            let t = UNIX_EPOCH + Duration::from_micros(micros);
            assert_eq!(parse_timestamp(&format_timestamp(t)), Some(t));
        }
        for bad in [
            "2001-02-29T00:00:00.000000Z",
            "2026-13-01T00:00:00.000000Z",
            "2026-10-20 00:00:00",
            "",
        ] {
            assert_eq!(parse_timestamp(bad), None, "{bad}");
        }
    }
}
