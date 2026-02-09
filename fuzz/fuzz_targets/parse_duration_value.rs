#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        if let Ok(duration) = strest::fuzzing::parse_duration_value_input(input) {
            debug_assert!(duration.as_millis() > 0);
        }
    }
});
