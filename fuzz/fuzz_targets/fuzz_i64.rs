#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_i64;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_i64(text) {
            let canonical = format!("{}", parsed);
            let reparsed = parse_i64(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
