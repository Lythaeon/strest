#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        if let Ok(version) = strest::fuzzing::parse_tls_version_input(input) {
            debug_assert!(matches!(
                version,
                strest::args::TlsVersion::V1_0
                    | strest::args::TlsVersion::V1_1
                    | strest::args::TlsVersion::V1_2
                    | strest::args::TlsVersion::V1_3
            ));
        }
    }
});
