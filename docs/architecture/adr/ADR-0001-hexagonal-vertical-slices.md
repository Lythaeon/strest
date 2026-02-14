# ADR-0001: Vertical Slices with Hexagonal Boundaries

- Status: Accepted
- Date: 2026-02-13
- Deciders: strest maintainers

## Context

`strest` currently works as a modular monolith, but core behavior is coupled to adapter concerns, especially CLI (`TesterArgs`) and runtime IO wiring. The migration target is vertical slices with explicit ports/adapters boundaries, without a big-bang rewrite.

## Decision

Adopt a phased migration architecture with these boundaries:

1. Layer model
- `domain`: business models, invariants, and policies.
- `application`: use cases orchestrating domain through ports.
- `adapters`: infrastructure implementations (CLI, config, transport, distributed IO, output, WASM).

2. Dependency rules
- `domain` must not depend on infrastructure frameworks (`clap`, `reqwest`, `tokio`, `ratatui`, `crossterm`).
- `application` must not depend directly on `clap`.
- `adapters` may depend on infra crates and implement ports for application use cases.

3. Transitional rules
- Do not introduce new deep coupling to `TesterArgs` in core logic.
- Treat `src/args` as a CLI adapter boundary.
- Prefer anti-corruption mapping from CLI/config into typed commands at entry boundaries.
- Keep behavior grouped by vertical slices (`local_run`, `distributed_run`, `replay_compare`).

4. Guardrail enforcement
- Add repository guardrail script: `scripts/check_architecture.sh`.
- Run guardrails in CI for pull requests and release validation.
- Track coupling baseline metrics (`crate::args` references, `TesterArgs` references).

## Consequences

### Positive
- Architectural drift is blocked early in CI.
- Migration progress is measurable by coupling metrics.
- New use-case code can be tested with lower adapter coupling.

### Tradeoffs
- Temporary dual pathways (legacy + new boundaries) increase short-term complexity.
- Additional CI checks add maintenance overhead.

## Follow-up

- Phase 1 introduces typed commands and mapping from CLI args.
- Phase 2 moves config precedence to explicit override policies.
- Later phases extract local/distributed/replay slices behind ports.
- Detailed technical execution guidance is captured in `ADR-0002-type-safety-dispatch-concurrency.md`.
