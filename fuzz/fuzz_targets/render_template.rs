#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let mut vars = BTreeMap::new();
        vars.insert("seq".to_owned(), "1".to_owned());
        vars.insert("timestamp_ms".to_owned(), "0".to_owned());
        vars.insert("timestamp_s".to_owned(), "0".to_owned());
        let seed = input.chars().take(32).collect::<String>();
        vars.insert("user".to_owned(), seed.clone());
        vars.insert("input".to_owned(), seed);
        let _ = strest::fuzzing::render_template_input(input, &vars);
    }
});
