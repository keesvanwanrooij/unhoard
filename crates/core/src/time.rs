//! Time helpers without a date library: FILETIME conversion and UTC date formatting.

use crate::fs::FileTime;

/// Seconds between 1601-01-01 (FILETIME epoch) and 1970-01-01 (Unix epoch).
const EPOCH_DIFF_SECS: i64 = 11_644_473_600;

/// Converts a FILETIME to seconds since 1970.
pub fn filetime_to_unix(ft: FileTime) -> i64 {
    (ft / 10_000_000) as i64 - EPOCH_DIFF_SECS
}

/// Converts seconds since 1970 to a FILETIME.
pub fn unix_to_filetime(secs: i64) -> FileTime {
    ((secs + EPOCH_DIFF_SECS).max(0) as u64) * 10_000_000
}

/// Splits days since 1970 into (year, month, day), proleptic Gregorian (Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Formats a Unix time as a UTC date, `2026-10-31`.
pub fn fmt_date(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Formats a Unix time as an ISO 8601 UTC timestamp, `2026-10-31T14:03:00Z`.
pub fn fmt_iso(secs: i64) -> String {
    let rem = secs.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        fmt_date(secs),
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The year and month of a Unix time, for grouping photos by month.
pub fn year_month(secs: i64) -> (i64, u32) {
    let (y, m, _) = civil_from_days(secs.div_euclid(86_400));
    (y, m)
}

/// Days since 1970 for a calendar date, the inverse of [`fmt_date`].
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_dates() {
        assert_eq!(fmt_date(0), "1970-01-01");
        assert_eq!(fmt_iso(1_793_448_180), "2026-10-31T12:03:00Z");
        assert_eq!(fmt_date(951_782_400), "2000-02-29");
    }

    #[test]
    fn filetime_round_trips() {
        let t = 1_793_448_180;
        assert_eq!(filetime_to_unix(unix_to_filetime(t)), t);
    }

    #[test]
    fn civil_round_trips() {
        for days in [-1000, 0, 11_000, 20_757, 30_000] {
            let s = fmt_date(days * 86_400);
            let y: i64 = s[0..4].parse().unwrap();
            let m: u32 = s[5..7].parse().unwrap();
            let d: u32 = s[8..10].parse().unwrap();
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }
}
