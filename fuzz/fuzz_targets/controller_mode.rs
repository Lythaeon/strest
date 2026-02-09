#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        if let Ok(mode) = input.parse::<strest::args::ControllerMode>() {
            debug_assert!(matches!(
                mode,
                strest::args::ControllerMode::Auto | strest::args::ControllerMode::Manual
            ));
        }
    }
});
