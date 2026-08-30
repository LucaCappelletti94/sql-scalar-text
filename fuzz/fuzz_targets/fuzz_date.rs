#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_date;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_date(text) {
            let canonical = format!("{}", parsed.format("%Y-%m-%d"));
            let reparsed = parse_date(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
