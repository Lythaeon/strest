# Vertical Slices + Hexagonal Boundaries

## Intent

Group behavior by use case (`local_run`, `distributed_run`, `replay_compare`) and isolate infrastructure behind ports/adapters.

## Structure

1. `domain`: models, invariants, policies
2. `application`: use cases and orchestration against ports
3. `adapters`: CLI/config/transport/output/distributed/WASM infrastructure

## Rules

- Domain must not import infrastructure frameworks.
- Application should depend on ports, not concrete adapters.
- Adapters map external inputs/outputs to application commands/events.
- CLI args are adapter concerns and should be mapped early to typed commands.

## Migration Guidance

1. Add anti-corruption mapping at the boundary.
2. Keep old behavior stable while introducing ports.
3. Move orchestration into use-case services.
4. Remove legacy coupling after call sites migrate.
