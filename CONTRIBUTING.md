# Contributing

Thanks for your interest in contributing to strest. This guide mirrors the README style and lists the exact steps I expect for a successful contribution.

## Requirements

- Rust (stable) and Cargo.
- `cargo make` installed.
- Linux chart deps: `fontconfig`, `freetype`, `pkg-config` (e.g. `libfontconfig1-dev libfreetype6-dev pkg-config` on Debian/Ubuntu).

## Workflow

1. Create a branch with a descriptive name.
1. Make changes with a tight scope.
1. Update docs and the changelog if behavior changes.
1. Run the required checks.
1. Commit with a clear, scoped message.
1. Push and open a PR with a concise summary.

## Branch Naming

Examples:

- `feat/timeout-charts`
- `fix/ui-latency`
- `docs/readme-updates`
- `chore/ci-hardening`

## Rules I Expect

- No `#[allow(...)]` macros unless explicitly agreed.
- Keep user-visible changes documented in `CHANGELOG.md`.
- Keep the README accurate for new flags, charts, or UI additions.

## Required Checks

```bash
cargo make format
cargo make clippy
cargo make test
cargo make architecture-check
```

If you touched WASM:

```bash
cargo make test-wasm
```

If you changed dependencies:

```bash
cargo make audit
cargo make deny
```

## Exact Command Sequence (typical)

```bash
git checkout -b feat/my-change
# edit files
cargo make format
cargo make clippy
cargo make test
# cargo make architecture-check
# cargo make test-wasm   # if WASM touched
# cargo make audit       # if deps changed
# cargo make deny        # if deps changed
git status
git commit -am "feat: my change"
git push -u origin feat/my-change
```

## PR Message

Use this structure:

```markdown
## Summary
- What changed and why

## Changes
- Key bullets

## Checks
- cargo make format
- cargo make clippy
- cargo make test
- cargo make architecture-check
- cargo make test-wasm (if applicable)
- cargo make audit (if deps changed)
- cargo make deny (if deps changed)
```

## Notes

- Some distributed tests can fail in environments without socket permissions.
- If you need a new dependency, explain the reason and impact in the PR.
