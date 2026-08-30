use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use core::fmt::Write as _;
use proptest::prelude::*;
use sql_scalar_text::{
    parse_bool, parse_date, parse_f64, parse_i64, parse_pg_bytea_hex, parse_time, parse_timestamp,
    parse_timestamp_tz,
};

fn date_strategy() -> impl Strategy<Value = NaiveDate> {
    (1970i32..=2100, 1u32..=366u32).prop_filter_map("valid calendar date", |(year, doy)| {
        NaiveDate::from_yo_opt(year, doy)
    })
}

fn time_strategy() -> impl Strategy<Value = NaiveTime> {
    (0u32..24, 0u32..60, 0u32..60, 0u32..1_000_000)
        .prop_map(|(h, m, s, us)| NaiveTime::from_hms_micro_opt(h, m, s, us).unwrap())
}

fn datetime_strategy() -> impl Strategy<Value = NaiveDateTime> {
    (date_strategy(), time_strategy()).prop_map(|(d, t)| d.and_time(t))
}

fn offset_secs_strategy() -> impl Strategy<Value = i32> {
    (-1439i32..=1439).prop_map(|m| m * 60)
}

fn whole_hour_offset_secs_strategy() -> impl Strategy<Value = i32> {
    (-23i32..=23).prop_map(|h| h * 3600)
}

fn strip_frac_zeros(s: &str) -> String {
    if let Some(dot) = s.rfind('.') {
        let trimmed = s[dot + 1..].trim_end_matches('0');
        if trimmed.is_empty() {
            s[..dot].to_owned()
        } else {
            format!("{}.{trimmed}", &s[..dot])
        }
    } else {
        s.to_owned()
    }
}

fn fmt_local(utc: chrono::DateTime<Utc>, offset: i32) -> String {
    let fo = FixedOffset::east_opt(offset).unwrap();
    let local = utc.with_timezone(&fo);
    strip_frac_zeros(&local.format("%Y-%m-%d %H:%M:%S%.6f").to_string())
}

fn offset_parts(offset: i32) -> (char, u32, u32) {
    let sign = if offset >= 0 { '+' } else { '-' };
    let abs = offset.unsigned_abs();
    let h = abs / 3600;
    let m = (abs % 3600) / 60;
    (sign, h, m)
}

proptest! {
    #[test]
    fn timestamp_pg_layout_roundtrip(dt in datetime_strategy()) {
        let text = strip_frac_zeros(&dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string());
        let got = parse_timestamp(&text)
            .unwrap_or_else(|| panic!("parse_timestamp: {text:?}"));
        prop_assert_eq!(got, dt);
    }

    #[test]
    fn timestamp_t_separator_roundtrip(dt in datetime_strategy()) {
        let text = strip_frac_zeros(&dt.format("%Y-%m-%dT%H:%M:%S%.6f").to_string());
        let got = parse_timestamp(&text)
            .unwrap_or_else(|| panic!("parse_timestamp T: {text:?}"));
        prop_assert_eq!(got, dt);
    }

    #[test]
    fn timestamp_tz_z_roundtrip(dt in datetime_strategy()) {
        let utc = Utc.from_utc_datetime(&dt);
        let local = strip_frac_zeros(&utc.format("%Y-%m-%dT%H:%M:%S%.6f").to_string());
        let text = format!("{local}Z");
        let got = parse_timestamp_tz(&text)
            .unwrap_or_else(|| panic!("parse_timestamp_tz Z: {text:?}"));
        prop_assert_eq!(got, utc);
    }

    #[test]
    fn timestamp_tz_plus_hh_roundtrip(dt in datetime_strategy(), offset in whole_hour_offset_secs_strategy()) {
        let utc = Utc.from_utc_datetime(&dt);
        let local = fmt_local(utc, offset);
        let (sign, h, _m) = offset_parts(offset);
        let text = format!("{local}{sign}{h:02}");
        let got = parse_timestamp_tz(&text)
            .unwrap_or_else(|| panic!("parse_timestamp_tz +hh: {text:?}"));
        prop_assert_eq!(got, utc);
    }

    #[test]
    fn timestamp_tz_plus_hhmm_roundtrip(dt in datetime_strategy(), offset in offset_secs_strategy()) {
        let utc = Utc.from_utc_datetime(&dt);
        let local = fmt_local(utc, offset);
        let (sign, h, m) = offset_parts(offset);
        let text = format!("{local}{sign}{h:02}{m:02}");
        let got = parse_timestamp_tz(&text)
            .unwrap_or_else(|| panic!("parse_timestamp_tz +hhmm: {text:?}"));
        prop_assert_eq!(got, utc);
    }

    #[test]
    fn timestamp_tz_plus_hh_colon_mm_roundtrip(dt in datetime_strategy(), offset in offset_secs_strategy()) {
        let utc = Utc.from_utc_datetime(&dt);
        let local = fmt_local(utc, offset);
        let (sign, h, m) = offset_parts(offset);
        let text = format!("{local}{sign}{h:02}:{m:02}");
        let got = parse_timestamp_tz(&text)
            .unwrap_or_else(|| panic!("parse_timestamp_tz +hh:mm: {text:?}"));
        prop_assert_eq!(got, utc);
    }

    #[test]
    fn date_iso_roundtrip(d in date_strategy()) {
        let text = d.format("%Y-%m-%d").to_string();
        let got = parse_date(&text)
            .unwrap_or_else(|| panic!("parse_date: {text:?}"));
        prop_assert_eq!(got, d);
    }

    #[test]
    fn time_iso_roundtrip(t in time_strategy()) {
        let text = strip_frac_zeros(&t.format("%H:%M:%S%.6f").to_string());
        let got = parse_time(&text)
            .unwrap_or_else(|| panic!("parse_time: {text:?}"));
        prop_assert_eq!(got, t);
    }
}

#[test]
fn timestamp_corpus_idempotence() {
    let corpus = [
        "2026-01-01 00:00:00",
        "2026-01-01T00:00:00",
        "2026-01-01 00:00:00.5",
        "2026-01-01T00:00:00.5",
        "2024-01-15 10:30:45",
        "2024-01-15 10:30:45.123456",
        "2023-12-31 23:59:59.999999",
    ];
    for &s in &corpus {
        let v = parse_timestamp(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = strip_frac_zeros(&v.format("%Y-%m-%d %H:%M:%S%.6f").to_string());
        let w = parse_timestamp(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn timestamp_tz_corpus_idempotence() {
    let corpus = [
        "2026-01-01 00:00:00+00",
        "2026-01-01 00:00:00+00:00",
        "2026-01-01 00:00:00+0000",
        "2026-01-01 00:00:00Z",
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00+00:00",
        "2025-12-31 22:00:00-02",
        "2026-01-01 02:30:00+02:30",
        "2026-01-01 00:00:00.5+00",
        "2026-01-01T00:00:00.5Z",
    ];
    for &s in &corpus {
        let v = parse_timestamp_tz(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let formatted = strip_frac_zeros(&v.format("%Y-%m-%dT%H:%M:%S%.9f").to_string());
        let text = format!("{formatted}Z");
        let w = parse_timestamp_tz(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn date_corpus_idempotence() {
    let corpus = ["2026-01-01", "2024-01-15", "2000-02-29", "1970-01-01"];
    for &s in &corpus {
        let v = parse_date(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = v.format("%Y-%m-%d").to_string();
        let w = parse_date(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn time_corpus_idempotence() {
    let corpus = ["00:00:00", "12:34:56.789", "10:30:45", "23:59:59"];
    for &s in &corpus {
        let v = parse_time(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = strip_frac_zeros(&v.format("%H:%M:%S%.6f").to_string());
        let w = parse_time(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn bool_corpus_idempotence() {
    let corpus = [("t", true), ("f", false), ("1", true), ("0", false)];
    for &(s, expected) in &corpus {
        let v = parse_bool(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        assert_eq!(v, expected);
        let canonical = if v { "t" } else { "f" };
        let w = parse_bool(canonical).unwrap_or_else(|| panic!("second parse: {canonical:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn bytea_corpus_idempotence() {
    let corpus = [
        (r"\x", vec![]),
        (r"\x00", vec![0u8]),
        (r"\x0001ff", vec![0u8, 1, 255]),
        (r"\xABCDEF", vec![0xabu8, 0xcd, 0xef]),
    ];
    for (s, bytes) in &corpus {
        let v = parse_pg_bytea_hex(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        assert_eq!(&v, bytes);
        let mut canonical = String::with_capacity(2 + v.len() * 2);
        canonical.push_str("\\x");
        for b in &v {
            let _ = write!(&mut canonical, "{b:02x}");
        }
        let w =
            parse_pg_bytea_hex(&canonical).unwrap_or_else(|| panic!("second parse: {canonical:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn i64_corpus_idempotence() {
    let corpus = [
        "0",
        "-1",
        "+1",
        "9223372036854775807",
        "-9223372036854775808",
    ];
    for &s in &corpus {
        let v = parse_i64(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = format!("{v}");
        let w = parse_i64(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[test]
fn f64_corpus_idempotence() {
    // Rust formats `Infinity` as `inf`, which `parse_f64` accepts.
    let corpus = [
        "0",
        "-1.5",
        "1e20",
        "3.141592653589793",
        "Infinity",
        "-Infinity",
    ];
    for &s in &corpus {
        let v = parse_f64(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = format!("{v}");
        let w = parse_f64(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}

#[cfg(feature = "decimal")]
#[test]
fn decimal_corpus_idempotence() {
    use sql_scalar_text::parse_decimal;

    let corpus = ["0", "-1.5", "+1", "1e20", "12345678901234567890.123456789"];
    for &s in &corpus {
        let v = parse_decimal(s).unwrap_or_else(|| panic!("first parse: {s:?}"));
        let text = v.to_string();
        let w = parse_decimal(&text).unwrap_or_else(|| panic!("second parse: {text:?}"));
        assert_eq!(v, w, "idempotence: {s:?}");
    }
}
