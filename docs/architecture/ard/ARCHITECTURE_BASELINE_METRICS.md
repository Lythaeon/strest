# Architecture Baseline Metrics

_Snapshot date: 2026-02-13_

This baseline is used to track migration progress toward vertical slices with hexagonal boundaries.

## Counting Method

- Scope: `src/**/*.rs`.
- Excludes tests: `**/tests/**`, `**/tests.rs`, `**/test_*.rs`, `**/*_test.rs`.
- Source of truth: `scripts/check_architecture.sh`.

## Baseline

- `non_test_rust_files`: `209`
- `files_referencing_crate_args`: `71`
- `files_referencing_tester_args`: `62`

## Top Cross-Module Edges (Top 10)

- `distributed -> args` (`22`)
- `distributed -> error` (`18`)
- `app -> error` (`17`)
- `charts -> error` (`16`)
- `app -> metrics` (`16`)
- `config -> error` (`13`)
- `charts -> metrics` (`13`)
- `app -> args` (`12`)
- `config -> args` (`11`)
- `protocol -> args` (`10`)
