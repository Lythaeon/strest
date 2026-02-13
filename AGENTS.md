# AGENTS.md

## Purpose
This file defines how contributors and coding agents should work in this repository.
It combines contribution rules, engineering best practices, and the target architecture direction.

## Scope and Priority
When making changes, follow this order:
1. Correctness and user safety.
2. Architectural direction (vertical slices + hexagonal boundaries).
3. Existing contribution and lint rules.
4. Minimal, reviewable diffs.

## Project Context
- `strest` is a high-performance load testing CLI.
- Use is only valid for infrastructure you own or are explicitly authorized to test.
- Main docs:
  - `README.md`
  - `CONTRIBUTING.md`
  - `docs/guides/USAGE.md`
  - `docs/guides/ADVANCED.md`
  - `docs/architecture/README.md`
  - `docs/architecture/ard/ARCHITECTURE_OVERVIEW.md`
  - `docs/architecture/ard/ARCHITECTURE_RISKS_HEXAGONAL_PLAN.md`

## Required Contribution Workflow
1. Create a scoped branch.
2. Keep changes tight and atomic.
3. Update docs and `CHANGELOG.md` for user-visible behavior changes.
4. Run required checks.
5. Submit a concise PR with rationale and tradeoffs.

### Required Checks
```bash
cargo make format
cargo make clippy
cargo make test
```

If WASM is touched:
```bash
cargo make test-wasm
```

If dependencies change:
```bash
cargo make audit
cargo make deny
```

## Engineering Best Practices

### Error Handling and Safety
- Do not use `unwrap`, `expect`, `todo!`, or panic-driven control flow in production code.
- Return typed errors (`AppError` and specific error variants) with actionable context.
- Prefer explicit validation at boundaries.

### Code Quality
- No `#[allow(...)]` unless explicitly approved.
- Keep functions focused; split orchestration from transformation logic.
- Add tests for behavior changes, not just happy paths.
- Preserve determinism where possible (especially metrics, replay, and distributed orchestration).

### Performance and Concurrency
- Avoid unnecessary allocations and cloning on hot paths.
- Be explicit about async cancellation and shutdown behavior.
- Avoid hidden blocking in async flows.

### Documentation
- Keep CLI/config docs aligned with behavior.
- If a flag or output contract changes, update docs in the same PR.

## Architecture Goal
Target architecture is **vertical slices with hexagonal ports/adapters**.

### Target Layers
- `domain`: business models, policies, invariants.
- `application`: use cases and orchestration against ports.
- `adapters` (infrastructure): CLI, config parsing, HTTP/protocol transport, distributed wire IO, UI/charts/sinks, WASM/plugin runtime.

### Dependency Rules (Desired)
- `domain` must not depend on infrastructure frameworks (`clap`, `reqwest`, `tokio`, `ratatui`, `crossterm`).
- `application` depends on `domain` + port traits, not concrete infra implementations.
- `adapters` may depend on infra crates and implement application ports.
- Entry points compose concrete adapters into use cases.

## Transitional Rules for Current Codebase
The repository is migrating. During migration:

1. Do not introduce new deep coupling to `TesterArgs`.
- New core/business logic should accept typed command/config structs, not raw CLI structs.

2. Treat `src/args` as an adapter boundary.
- CLI parsing and clap concerns stay there.
- Avoid placing new domain policy there.

3. Prefer anti-corruption mapping at boundaries.
- Map CLI/config inputs into domain/application commands early.

4. Keep vertical behavior grouped.
- New features should fit a slice (`local_run`, `distributed_run`, `replay_compare`) instead of scattering across horizontal modules.

5. Use branch-by-abstraction.
- Add ports + adapters first, then migrate call sites incrementally.

## Suggested Slice Ownership
- `local_run`: run execution, protocol traffic, local metrics lifecycle.
- `distributed_run`: controller/agent coordination, aggregation, distributed execution.
- `replay_compare`: replay windows, snapshots, comparison flows.
- `shared_kernel`: minimal shared value objects only.

## PR Acceptance Checklist
A PR is ready when:
- Behavior is correct and tests pass.
- Diff is scoped and understandable.
- Lint/format/test checks pass.
- Docs/changelog are updated when needed.
- New code does not increase infra-domain coupling.
- Any architectural compromise is called out with follow-up plan.

## Anti-Patterns to Avoid
- Passing `TesterArgs` through new domain/application APIs.
- Mixing UI/sink/charts wiring directly inside core policy logic.
- Embedding config precedence as ad-hoc mutation order across modules.
- Adding new cross-cutting flags without explicit boundary mapping.

## Preferred Change Pattern
For new behavior, aim for this sequence:
1. Define domain model/policy.
2. Define application use-case input/output and ports.
3. Implement or adapt infrastructure adapter.
4. Wire in entry layer.
5. Add tests at unit and integration level.

## Notes for Agents
- Optimize for small, reversible, evidence-backed changes.
- If architecture and delivery conflict, preserve behavior first, then add a migration seam.
- Mention architectural impact explicitly in PR summaries.
