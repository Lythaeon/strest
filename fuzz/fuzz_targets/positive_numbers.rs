#![no_main]

use libfuzzer_sys::fuzz_target;

fn take_numeric_string(data: &[u8], cursor: &mut usize, max_len: usize) -> String {
    if *cursor >= data.len() {
        return String::new();
    }
    let len = usize::from(data[*cursor]) % (max_len.saturating_add(1));
    *cursor = cursor.saturating_add(1);
    let end = (*cursor).saturating_add(len).min(data.len());
    let slice = &data[*cursor..end];
    *cursor = end;

    let mut out = String::with_capacity(slice.len());
    for &byte in slice {
        let ch = match byte % 12 {
            0..=9 => char::from(b'0' + (byte % 10)),
            10 => '-',
            _ => '+',
        };
        out.push(ch);
    }
    out
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut cursor = 0usize;
    let first = take_numeric_string(data, &mut cursor, 24);
    let second = take_numeric_string(data, &mut cursor, 24);

    if let Ok(value) = strest::fuzzing::parse_positive_u64_input(&first) {
        debug_assert!(value >= 1);
    }
    if let Ok(value) = strest::fuzzing::parse_positive_usize_input(&second) {
        debug_assert!(value >= 1);
    }
});
