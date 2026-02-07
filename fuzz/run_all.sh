#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MAX_TIME="${FUZZ_MAX_TIME:-600}"

if [[ $# -gt 0 ]]; then
  TARGETS=("$@")
else
  TARGETS=(
    cli_args
    config_json
    config_toml
    histogram_base64
    load_profile
    metrics_log
    metrics_range
    parse_duration_arg
    parse_duration_value
    parse_header
    parse_tls_version
    positive_numbers
    render_template
    scenario_config
    scenario_request
  )
fi

for target in "${TARGETS[@]}"; do
  cargo +nightly fuzz run "$target" -- -max_total_time="$MAX_TIME"
done
