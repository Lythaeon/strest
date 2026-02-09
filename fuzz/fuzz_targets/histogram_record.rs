#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let Ok(mut left) = strest::metrics::LatencyHistogram::new() else {
        return;
    };
    let Ok(mut right) = strest::metrics::LatencyHistogram::new() else {
        return;
    };

    let mut index = 0usize;
    let mut count = 0usize;
    let mut left_count = 0u64;
    let mut right_count = 0u64;
    while index + 8 <= data.len() && count < 256 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&data[index..index + 8]);
        let value = u64::from_le_bytes(bytes);

        if count % 2 == 0 {
            if left.record(value).is_ok() {
                left_count = left_count.saturating_add(1);
            }
        } else {
            if right.record(value).is_ok() {
                right_count = right_count.saturating_add(1);
            }
        }

        index = index.saturating_add(8);
        count = count.saturating_add(1);
    }

    if left.merge(&right).is_ok() {
        debug_assert_eq!(left.count(), left_count.saturating_add(right_count));
    }
    let _ = left.percentiles();
    let _ = left.count();

    if let Ok(encoded) = left.encode_base64() {
        let _ = strest::metrics::LatencyHistogram::decode_base64(&encoded);
    }
});
