#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use chrono::{
    DateTime, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
};

/// Parse a timestamp with a space or `T` separator and optional fractional seconds.
pub fn parse_timestamp(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S"))
        .ok()
}

/// Parse an RFC 3339 or PostgreSQL timestamp with an explicit offset.
pub fn parse_timestamp_tz(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.with_timezone(&Utc));
    }

    let bytes = text.as_bytes();
    if bytes.len() < 11 || bytes[10] != b' ' {
        return None;
    }

    for len in [1, 3, 5, 6] {
        if bytes.len() >= len {
            let offset_start = bytes.len() - len;
            if let Some(offset) = parse_pg_offset(&bytes[offset_start..]) {
                let naive_dt = parse_timestamp(&text[..offset_start])?;
                if let LocalResult::Single(dt) = offset.from_local_datetime(&naive_dt) {
                    return Some(dt.with_timezone(&Utc));
                }
            }
        }
    }

    None
}

/// Parse a `YYYY-MM-DD` date.
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Parse a time with optional fractional seconds.
pub fn parse_time(text: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(text, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M:%S"))
        .ok()
}

/// Parse `t`, `f`, `1`, or `0` as a boolean.
pub fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "t" | "1" => Some(true),
        "f" | "0" => Some(false),
        _ => None,
    }
}

/// Parse PostgreSQL `\x`-prefixed bytea hex with no intermediate allocation.
pub fn parse_pg_bytea_hex(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'\\' || bytes[1] != b'x' {
        return None;
    }

    let hex_part = &bytes[2..];
    if !hex_part.len().is_multiple_of(2) {
        return None;
    }

    let mut result = Vec::with_capacity(hex_part.len() / 2);
    for i in (0..hex_part.len()).step_by(2) {
        let high = hex_digit(hex_part[i])?;
        let low = hex_digit(hex_part[i + 1])?;
        result.push((high << 4) | low);
    }
    Some(result)
}

/// Parse integer text with an optional leading sign.
pub fn parse_i64(text: &str) -> Option<i64> {
    text.parse::<i64>().ok()
}

/// Parse a 64-bit float, including PostgreSQL infinity and `NaN` spellings.
pub fn parse_f64(text: &str) -> Option<f64> {
    match text {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        _ => text.parse::<f64>().ok(),
    }
}

/// Parse an arbitrary-precision decimal.
#[cfg(feature = "decimal")]
pub fn parse_decimal(text: &str) -> Option<bigdecimal::BigDecimal> {
    use core::str::FromStr;
    bigdecimal::BigDecimal::from_str(text).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_two_digits(bytes: &[u8]) -> Option<i32> {
    if bytes.len() < 2 || !bytes[0].is_ascii_digit() || !bytes[1].is_ascii_digit() {
        return None;
    }
    Some(i32::from(bytes[0] - b'0') * 10 + i32::from(bytes[1] - b'0'))
}

fn parse_pg_offset(bytes: &[u8]) -> Option<FixedOffset> {
    match bytes.len() {
        1 if bytes[0] == b'Z' => FixedOffset::east_opt(0),
        3 => {
            let sign = match bytes[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let h = parse_two_digits(&bytes[1..])?;
            if h > 23 {
                return None;
            }
            FixedOffset::east_opt(sign * h * 3600)
        }
        5 => {
            let sign = match bytes[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let h = parse_two_digits(&bytes[1..])?;
            let m = parse_two_digits(&bytes[3..])?;
            if h > 23 || m > 59 {
                return None;
            }
            FixedOffset::east_opt(sign * (h * 3600 + m * 60))
        }
        6 if bytes[3] == b':' => {
            let sign = match bytes[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let h = parse_two_digits(&bytes[1..3])?;
            let m = parse_two_digits(&bytes[4..6])?;
            if h > 23 || m > 59 {
                return None;
            }
            FixedOffset::east_opt(sign * (h * 3600 + m * 60))
        }
        _ => None,
    }
}
