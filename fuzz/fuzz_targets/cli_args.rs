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
            debug_assert!(parsed.target_duration.get() >= 1);
            if let Some(requests) = parsed.requests {
                debug_assert!(requests.get() >= 1);
            }
            debug_assert!(parsed.log_shards.get() >= 1);
            debug_assert!(parsed.ui_window_ms.get() >= 1);
            debug_assert!(parsed.max_tasks.get() >= 1);
            debug_assert!(parsed.spawn_rate_per_tick.get() >= 1);
            debug_assert!(parsed.tick_interval.get() >= 1);
            debug_assert!(parsed.metrics_max.get() >= 1);
            debug_assert!(parsed.agent_weight.get() >= 1);
            debug_assert!(parsed.min_agents.get() >= 1);
            debug_assert!(parsed.agent_reconnect_ms.get() >= 1);
            debug_assert!(parsed.agent_heartbeat_interval_ms.get() >= 1);
            debug_assert!(parsed.agent_heartbeat_timeout_ms.get() >= 1);
            debug_assert!(parsed.connect_timeout.as_millis() > 0);
            debug_assert!(parsed.ui_fps >= 1);
            if let Some(interval) = parsed.distributed_stream_interval_ms {
                debug_assert!(interval.get() >= 1);
            }
            if let Some(range) = parsed.metrics_range {
                let start = *range.0.start();
                let end = *range.0.end();
                debug_assert!(start <= end);
            }
        }
    }
});
