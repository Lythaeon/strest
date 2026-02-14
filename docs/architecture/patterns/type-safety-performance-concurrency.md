# Type Safety, Performance, and Concurrency Patterns

## Intent

Define shared engineering rules for:
- making invalid states unrepresentable
- preserving cache-friendly hot paths
- choosing dispatch strategy
- minimizing lock contention in concurrent paths

These rules are expected across slices and adapters.

## 1. Newtypes and Invalid States

### Rule
Represent invariants in types, not in scattered runtime checks.

### Prefer
- Newtypes for constrained values (for example positive-only numeric types).
- Enums for mutually exclusive modes instead of boolean combinations.
- Small command/config structs with required fields over partially initialized structs.

### Avoid
- Passing raw primitive values with hidden constraints (`u64` that must be `> 0`).
- State machines encoded as unrelated booleans.
- Late validation deep in execution paths.

### Boundary Guidance
- Parse/validate at adapter boundaries (`args`, `config`, wire input).
- Keep application/domain APIs typed so invalid input cannot compile or be constructed.

## 2. Cache Locality and Inlining

### Rule
Optimize data layout and call structure for hot paths first; optimize instructions second.

### Prefer
- Contiguous data and predictable iteration in metrics/request loops.
- Reuse allocations where possible and pre-size collections when capacity is known.
- Passing by reference in hot paths to avoid clones.
- Small helper functions that are obvious candidates for inlining by the compiler.

### Inlining Policy
- Let the compiler decide by default.
- Use `#[inline]` or `#[inline(always)]` only when profiling proves measurable gain.
- Remove forced inlining when gains are not repeatable.

## 3. Dispatch Strategy (Static First)

### Rule
Prefer static dispatch for core execution paths; use dynamic dispatch only at extension seams.

### Prefer static dispatch when
- behavior is known at compile time
- code is in hot paths
- monomorphization overhead is acceptable

### Use dynamic dispatch when
- runtime extensibility is required (plugin/registry boundaries)
- implementation set is not known at compile time
- reduced compile-time or binary-size pressure is more important than max throughput

### Practical Guidance
- Keep dynamic trait objects at boundaries.
- Convert to concrete/static execution as early as possible after selection.

## 4. Lock-Free and Low-Lock Concurrency

### Rule
Prefer immutable sharing + atomic coordination over shared mutable locks in hot paths.

### Prefer
- `Arc<T>` for shared ownership.
- atomics for counters/flags (`AtomicU64`, `AtomicBool`, etc.).
- channels (`mpsc`, `watch`) for ownership transfer and signaling.
- `ArcShift<T>` for snapshot-style shared state updates where read-mostly maps need occasional replacement.

### Use locks only when
- mutation requires compound invariants that atomics cannot safely express
- the code path is not performance critical

### Avoid
- coarse `Mutex`/`RwLock` around high-frequency request/metrics paths
- holding locks across `.await`

## 5. PR Checklist (Technical)

Before merging performance/concurrency-sensitive changes:
1. Invariants are encoded in types at boundaries.
2. New invalid combinations are not representable by API shape.
3. Dispatch choice is explicit and justified (static by default).
4. Shared state avoids unnecessary lock contention.
5. Required checks pass:
   - `cargo make format`
   - `cargo make clippy`
   - `cargo make test`
   - `cargo make architecture-check`
