# ADR-0002: Type-Safe APIs, Static-First Dispatch, and Low-Lock Concurrency

- Status: Accepted
- Date: 2026-02-14
- Deciders: strest maintainers

## Context

After Phase 7 boundary migration, architecture seams are established (`entry -> application -> adapters -> slices`).

The next risk is implementation drift inside those seams:
- invariants moving back to ad-hoc runtime checks
- overuse of dynamic dispatch on hot paths
- lock contention in concurrent execution paths

## Decision

Adopt these technical architecture rules:

1. Type-safety first
- Encode invariants in types (newtypes/enums) at system boundaries.
- Prefer APIs that cannot represent invalid combinations.

2. Static dispatch by default
- Use static/generic dispatch for core orchestration and hot paths.
- Use dynamic dispatch only at explicit runtime-extension boundaries.

3. Low-lock concurrency
- Prefer `Arc`, atomics, and channels for high-frequency coordination.
- Use `ArcShift` for read-mostly shared snapshots in manual controller flows.
- Avoid lock scopes that cross `.await`.

4. Performance policy
- Optimize for cache behavior and allocation discipline first.
- Use explicit inlining annotations only with profiling evidence.

## Consequences

### Positive
- Stronger compile-time guarantees for config and execution state.
- Lower overhead on core execution paths.
- Better concurrency behavior under distributed/high-load scenarios.
- Clearer review criteria for architecture-sensitive changes.

### Tradeoffs
- More up-front type modeling work.
- Possible binary-size increase from monomorphization in generic paths.
- Requires discipline to keep dynamic dispatch limited to boundary seams.

## Follow-up

- Keep the implementation guidance in `docs/architecture/ard/ARCHITECTURE_TECHNICAL_GUIDE.md`.
- Keep reusable rule summaries in `docs/architecture/patterns/type-safety-performance-concurrency.md`.
- Enforce boundary constraints through existing architecture checks and code review.
