# Architecture Technical Guide

## Purpose

This ARD defines technical implementation guidance for type safety, performance, and concurrency in the current hexagonal architecture.

This document is implementation-focused.
For the formal decision, see `docs/architecture/adr/ADR-0002-type-safety-dispatch-concurrency.md`.

## 1. Type Invariants with Newtypes

Encode constraints in types instead of scattering runtime checks.

Guidance:
- Use constrained newtypes for validated inputs.
- Prefer enums over loosely related boolean flags.
- Validate at boundaries (CLI/config/wire), then pass typed values inward.

Current examples in code:
- `PositiveU64` and `PositiveUsize` in `src/args`.
- Typed run-plan commands in `src/application/commands.rs`.

## 2. Make Invalid States Unrepresentable

Shape APIs so impossible states cannot be expressed.

Guidance:
- Split mode-specific behavior into explicit command variants.
- Use dedicated structs for per-mode required data.
- Avoid optional fields when a field is logically required for a mode.

Current examples:
- Entry run plan variants in `src/entry/plan/types.rs`.
- Slice execution ports in `src/application/slice_execution.rs`.

## 3. Cache Locality and Inlining

Prioritize data movement and predictable access patterns in hot paths.

Guidance:
- Keep hot-loop data contiguous and iteration predictable.
- Reuse allocations and pre-size collections when bounds are known.
- Avoid unnecessary clones in request/metrics paths.
- Let the compiler inline by default.
- Use forced inlining only when profiling demonstrates repeatable gains.

## 4. Dispatch Strategy (Static Preferred)

Static dispatch is the default for core execution paths.

Guidance:
- Prefer generic/static dispatch in application and runtime orchestration.
- Use dynamic dispatch only at extension boundaries that require runtime selection.
- Keep trait-object usage near boundaries, then transition to concrete/static calls.

Current examples:
- Static/generic orchestration in `src/application/local_run.rs`.
- Dynamic extension boundary in protocol registry (`src/protocol/registry.rs`).

## 5. Low-Lock Concurrency

Prefer immutable sharing + atomics/channels over lock-heavy shared mutation.

Guidance:
- Use `Arc<T>` for shared ownership.
- Use atomics for counters/flags.
- Use channels (`mpsc`, `watch`) for signaling and data handoff.
- Use `ArcShift<T>` for read-mostly shared snapshots with occasional whole-value replacement.
- Avoid holding locks across `.await`.

Current examples:
- Agent/controller coordination in `src/distributed`.
- Manual controller shared state snapshots via `ArcShift` in `src/distributed/controller/manual`.

## 6. Review Checklist

For architecture-sensitive changes, verify:
1. Invariants are encoded in types at the boundary.
2. API shape prevents invalid state combinations.
3. Dispatch choice is explicit and static-first.
4. Shared state avoids unnecessary lock contention.
5. Required contribution checks pass.
