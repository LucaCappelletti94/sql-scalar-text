#![no_main]

use core::fmt::Write as FmtWrite;
use libfuzzer_sys::fuzz_target;
use sql_scalar_text::parse_pg_bytea_hex;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        if let Some(parsed) = parse_pg_bytea_hex(text) {
            let mut canonical = String::with_capacity(2 + parsed.len() * 2);
            canonical.push_str(r"\x");
            for byte in &parsed {
                let _ = write!(canonical, "{:02x}", byte);
            }
            let reparsed = parse_pg_bytea_hex(&canonical).expect("canonical form must reparse");
            assert_eq!(reparsed, parsed);
        }
    }
});
