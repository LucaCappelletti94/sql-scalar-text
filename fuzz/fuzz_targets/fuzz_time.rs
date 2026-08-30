#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_time;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_time(text) {
            let canonical = format!("{}", parsed.format("%H:%M:%S%.9f"));
            let reparsed = parse_time(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
