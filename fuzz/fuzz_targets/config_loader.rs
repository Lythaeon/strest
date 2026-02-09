#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let suffix = match data[0] % 4 {
        0 => ".toml",
        1 => ".json",
        2 => ".txt",
        _ => "",
    };

    let payload = if data.len() > 1_000_000 {
        &data[..1_000_000]
    } else {
        data
    };
    let content = String::from_utf8_lossy(payload).to_string();

    let Ok(mut file) = tempfile::Builder::new().suffix(suffix).tempfile() else {
        return;
    };

    if file.write_all(content.as_bytes()).is_err() {
        return;
    }

    let path = file.path().to_path_buf();
    let result = strest::fuzzing::load_config_file_input(&path);
    if result.is_ok() {
        debug_assert!(suffix == ".toml" || suffix == ".json");
    }
});
