//! Wall-clock helpers: message timestamps are Unix epochs (UTC), the UI shows
//! them in the system time zone.

/// Seconds or milliseconds since the epoch -> seconds.
pub fn to_secs(ts: i64) -> i64 {
    if ts > 10_000_000_000 {
        ts / 1000
    } else {
        ts
    }
}

/// Offset of the system time zone from UTC at `ts_sec`, in seconds.
/// Tests run in UTC so expected strings do not depend on the machine.
pub fn offset_secs(ts_sec: i64) -> i64 {
    #[cfg(test)]
    {
        let _ = ts_sec;
        0
    }
    #[cfg(not(test))]
    {
        use chrono::{Local, Offset, TimeZone};
        chrono::DateTime::from_timestamp(ts_sec, 0)
            .map(|dt| Local.offset_from_utc_datetime(&dt.naive_utc()).fix().local_minus_utc() as i64)
            .unwrap_or(0)
    }
}

/// `ts_sec` shifted so UTC-style date math yields local wall-clock values.
pub fn to_local(ts_sec: i64) -> i64 {
    ts_sec + offset_secs(ts_sec)
}

/// Local (hours, minutes) of a second/millisecond timestamp.
pub fn hour_minute(ts: i64) -> (i64, i64) {
    let local = to_local(to_secs(ts));
    (local.div_euclid(3600).rem_euclid(24), local.div_euclid(60).rem_euclid(60))
}

/// Local days since the epoch of a second/millisecond timestamp.
pub fn local_day(ts: i64) -> i64 {
    to_local(to_secs(ts)).div_euclid(86_400)
}

/// Local day index of the current moment.
pub fn today() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    local_day(now)
}
