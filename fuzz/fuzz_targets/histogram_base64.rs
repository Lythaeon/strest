#![no_main]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let max_len = 4096;
    let input = if data[0] % 2 == 0 {
        let payload = data.get(1..).unwrap_or(&[]);
        let capped = if payload.len() > max_len {
            &payload[..max_len]
        } else {
            payload
        };
        STANDARD.encode(capped)
    } else {
        let capped = if data.len() > max_len { &data[..max_len] } else { data };
        String::from_utf8_lossy(capped).to_string()
    };

    if let Ok(histogram) = strest::metrics::LatencyHistogram::decode_base64(&input) {
        if let Ok(encoded) = histogram.encode_base64() {
            debug_assert!(!encoded.is_empty());
        }
    }
});
