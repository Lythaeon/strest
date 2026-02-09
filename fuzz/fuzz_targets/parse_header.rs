#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        if let Ok((key, value)) = strest::fuzzing::parse_header_input(input) {
            debug_assert_eq!(key, key.trim());
            debug_assert_eq!(value, value.trim());
        }
    }
});
