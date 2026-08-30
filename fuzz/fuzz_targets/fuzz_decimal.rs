#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_decimal;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_decimal(text) {
            let canonical = parsed.to_string();
            let reparsed = parse_decimal(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
