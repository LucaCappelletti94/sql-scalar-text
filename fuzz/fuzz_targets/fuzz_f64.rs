#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_f64;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_f64(text) {
            let canonical = if parsed.is_nan() {
                "NaN".to_string()
            } else if parsed.is_infinite() {
                if parsed.is_sign_positive() {
                    "Infinity".to_string()
                } else {
                    "-Infinity".to_string()
                }
            } else {
                format!("{}", parsed)
            };
            let reparsed = parse_f64(&canonical).expect("canonical form must reparse");
            if parsed.is_nan() {
                assert!(reparsed.is_nan(), "NaN must reparse to NaN");
            } else {
                assert_eq!(reparsed, parsed);
            }
        }
    }
});
