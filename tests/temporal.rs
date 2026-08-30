use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sql_scalar_text::{parse_date, parse_time, parse_timestamp, parse_timestamp_tz};

#[derive(Debug)]
enum Temporal {
    Timestamp(NaiveDateTime),
    TimestampTz(DateTime<Utc>),
    Date(NaiveDate),
    Time(NaiveTime),
}

fn naive(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|date| date.and_hms_milli_opt(hour, minute, second, millisecond))
        .expect("valid corpus timestamp")
}

fn utc(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
) -> DateTime<Utc> {
    naive(year, month, day, hour, minute, second, millisecond).and_utc()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid corpus date")
}

fn time(hour: u32, minute: u32, second: u32, millisecond: u32) -> NaiveTime {
    NaiveTime::from_hms_milli_opt(hour, minute, second, millisecond).expect("valid corpus time")
}

fn accepted() -> Vec<(&'static str, Temporal)> {
    let midnight = Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0));
    let midnight_half = Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 500));
    let naive_midnight = Temporal::Timestamp(naive(2026, 1, 1, 0, 0, 0, 0));
    let naive_half = Temporal::Timestamp(naive(2026, 1, 1, 0, 0, 0, 500));
    vec![
        ("2026-01-01 00:00:00+00", midnight),
        (
            "2026-01-01 00:00:00+00:00",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2026-01-01 00:00:00+0000",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2026-01-01 00:00:00Z",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2026-01-01T00:00:00Z",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2026-01-01T00:00:00+00:00",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2025-12-31 22:00:00-02",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        (
            "2026-01-01 02:30:00+02:30",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 0)),
        ),
        ("2026-01-01 00:00:00.5+00", midnight_half),
        (
            "2026-01-01T00:00:00.5Z",
            Temporal::TimestampTz(utc(2026, 1, 1, 0, 0, 0, 500)),
        ),
        ("2026-01-01 00:00:00", naive_midnight),
        (
            "2026-01-01T00:00:00",
            Temporal::Timestamp(naive(2026, 1, 1, 0, 0, 0, 0)),
        ),
        ("2026-01-01 00:00:00.5", naive_half),
        (
            "2026-01-01T00:00:00.5",
            Temporal::Timestamp(naive(2026, 1, 1, 0, 0, 0, 500)),
        ),
        ("2026-01-01", Temporal::Date(date(2026, 1, 1))),
        ("00:00:00", Temporal::Time(time(0, 0, 0, 0))),
        ("12:34:56.789", Temporal::Time(time(12, 34, 56, 789))),
    ]
}

#[test]
fn parses_temporal_spelling_corpus_to_exact_values() {
    for (text, expected) in accepted() {
        match expected {
            Temporal::Timestamp(expected) => {
                assert_eq!(parse_timestamp(text), Some(expected), "{text}");
            }
            Temporal::TimestampTz(expected) => {
                assert_eq!(parse_timestamp_tz(text), Some(expected), "{text}");
            }
            Temporal::Date(expected) => {
                assert_eq!(parse_date(text), Some(expected), "{text}");
            }
            Temporal::Time(expected) => {
                assert_eq!(parse_time(text), Some(expected), "{text}");
            }
        }
    }
}

#[test]
fn refuses_temporal_negative_corpus() {
    for text in [
        "2026-01-01 00:00:00",
        "2026-01-01",
        "nope",
        "",
        "2026-01-01 00:00:00+24",
        "2026-01-01 00:00:00+24:00",
        "2026-01-01 00:00:00+00:60",
        "2026-01-01 00:00:00+00:99",
        "2026-01-01 00:00:00-24",
        "2026-01-01 00:00:00-00:60",
        "2026-01-01 00:00:00+//",
        "2026-01-01 00:00:00+//:00",
        "2026-01-01 00:00:00+-0",
        "2026-01-01 00:00:00++00",
    ] {
        assert_eq!(parse_timestamp_tz(text), None, "{text}");
    }
    for text in ["2026-01-01 00:00:00+00", "2026-01-01", "nope", ""] {
        assert_eq!(parse_timestamp(text), None, "{text}");
    }
    for text in ["20260101", "nope", ""] {
        assert_eq!(parse_date(text), None, "{text}");
    }
    for text in ["12:34", "nope", ""] {
        assert_eq!(parse_time(text), None, "{text}");
    }
}
