#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_bool;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_bool(text) {
            let canonical = if parsed { "t" } else { "f" };
            let reparsed = parse_bool(canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
