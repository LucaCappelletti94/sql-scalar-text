#![no_main]

#[path = "pg_year.rs"]
mod pg_year;

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_timestamp;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_timestamp(text) {
            let (year, era) = pg_year::pg_year(&parsed);
            let canonical = format!("{year:04}-{}{era}", parsed.format("%m-%d %H:%M:%S%.9f"));
            let reparsed = parse_timestamp(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
