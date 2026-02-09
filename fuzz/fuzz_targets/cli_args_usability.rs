#![no_main]

use clap::Parser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let mut args = Vec::new();
        args.push("strest".to_owned());
        for token in input.split_whitespace().take(64) {
            args.push(token.to_owned());
        }
        let arg_refs: Vec<&str> = args.iter().map(|value| value.as_str()).collect();
        if let Ok(parsed) = strest::args::TesterArgs::try_parse_from(arg_refs) {
            if parsed.data_file.is_some() {
                debug_assert!(parsed.data_lines.is_none());
            }
            if parsed.data_lines.is_some() {
                debug_assert!(parsed.data_file.is_none());
            }
            if parsed.requests.is_some() {
                debug_assert!(parsed.requests.unwrap().get() >= 1);
            }
        }
    }
});
