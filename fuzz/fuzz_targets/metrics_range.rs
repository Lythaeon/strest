#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        if let Ok(range) = strest::fuzzing::parse_metrics_range_input(input) {
            let start = *range.0.start();
            let end = *range.0.end();
            debug_assert!(start <= end);
        }
    }
});
