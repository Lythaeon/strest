#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required to run architecture checks."
  exit 1
fi

NON_TEST_GLOBS=(
  -g '*.rs'
  -g '!**/tests/**'
  -g '!**/tests.rs'
  -g '!**/test_*.rs'
  -g '!**/*_test.rs'
)

FAILED=0

list_non_test_rust_files() {
  rg --files src "${NON_TEST_GLOBS[@]}"
}

count_matching_files() {
  local needle="$1"
  local matches

  matches="$(rg -l --fixed-strings "$needle" src "${NON_TEST_GLOBS[@]}" || true)"
  if [[ -z "$matches" ]]; then
    echo "0"
    return 0
  fi

  printf '%s\n' "$matches" | wc -l | tr -d '[:space:]'
}

check_forbidden_crates_in_layer() {
  local layer_dir="$1"
  shift
  local crates=("$@")

  if [[ ! -d "$layer_dir" ]]; then
    echo "skip: ${layer_dir} not present"
    return 0
  fi

  local crate_name
  for crate_name in "${crates[@]}"; do
    local regex="\\b${crate_name}::"
    local matches
    matches="$(rg -n --glob '*.rs' "$regex" "$layer_dir" || true)"
    if [[ -n "$matches" ]]; then
      echo "error: forbidden '${crate_name}' usage detected in ${layer_dir}"
      printf '%s\n' "$matches"
      FAILED=1
    else
      echo "ok: ${layer_dir} has no '${crate_name}' usage"
    fi
  done
}

print_top_module_edges() {
  local edge_tmp
  edge_tmp="$(mktemp)"

  while IFS= read -r file_path; do
    local source_module
    source_module="${file_path#src/}"
    source_module="${source_module%%/*}"

    while IFS= read -r ref; do
      local target_module
      target_module="${ref#crate::}"
      [[ "$target_module" == "$source_module" ]] && continue
      printf '%s -> %s\n' "$source_module" "$target_module" >> "$edge_tmp"
    done < <(
      rg --no-filename '^use ' "$file_path" \
        | rg -o 'crate::[A-Za-z_][A-Za-z0-9_]*' \
        | sort -u
    )
  done < <(list_non_test_rust_files)

  if [[ ! -s "$edge_tmp" ]]; then
    echo "  (none)"
    rm -f "$edge_tmp"
    return 0
  fi

  awk '{count[$0]++} END {for (k in count) printf "%d\t%s\n", count[k], k}' "$edge_tmp" \
    | sort -nr -k1,1 -k2,2 \
    | head -n 10 \
    | awk -F'\t' '{printf "  %s (%s)\n", $2, $1}'

  rm -f "$edge_tmp"
}

echo "Architecture boundary checks"
check_forbidden_crates_in_layer "src/domain" "clap" "reqwest" "tokio" "ratatui" "crossterm"
check_forbidden_crates_in_layer "src/application" "clap"

echo
echo "Coupling baseline metrics"
echo "  non_test_rust_files: $(list_non_test_rust_files | wc -l | tr -d '[:space:]')"
echo "  files_referencing_crate_args: $(count_matching_files 'crate::args')"
echo "  files_referencing_tester_args: $(count_matching_files 'TesterArgs')"
echo "  top_cross_module_edges:"
print_top_module_edges

if [[ "$FAILED" -ne 0 ]]; then
  exit 1
fi
