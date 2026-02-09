use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let max_repeat = u32::from(data[0]);
    let pattern = std::str::from_utf8(&data[1..]).unwrap_or("");
    let _ = strest::fuzzing::compile_rand_regex_input(pattern, max_repeat);
});
