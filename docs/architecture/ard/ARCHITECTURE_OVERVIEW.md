# Strest Architecture Overview

_Generated from `src/**/*.rs` on 2026-02-14 16:15:09 UTC_

_Purpose: direct call-flow view of the current architecture with explicit slice boundaries._

## Boundary Contract
1. `entry` calls `application` only.
2. `application` orchestrates use-cases and calls ports only.
3. `adapters::runtime` implements those ports and calls concrete runtime slices.
4. Runtime slices (`app`, `distributed`, `service`) do not call each other directly.
5. Boundary checks are enforced by `scripts/check_architecture.sh`.

## No-Spiderweb View (Current)
```mermaid
flowchart LR
  main["main::main"] --> entry["entry::run"]
  entry --> plan["entry::plan::{build, execute}"]

  plan --> appSlice["application::slice_execution"]
  plan --> appDist["application::distributed_run"]

  appSlice --> rtLocal["RuntimeLocalPort"]
  appSlice --> rtReplay["RuntimeReplayPort"]
  appSlice --> rtCompare["RuntimeComparePort"]
  appSlice --> rtCleanup["RuntimeCleanupPort"]
  appSlice --> rtService["RuntimeServicePort"]
  appDist --> rtDistributed["RuntimeDistributedPort"]

  rtLocal --> local["app::run_local"]
  rtReplay --> replay["app::run_replay"]
  rtCompare --> compare["app::run_compare"]
  rtCleanup --> cleanup["app::run_cleanup"]
  rtService --> service["service::handle_service_action"]
  rtDistributed --> controller["distributed::run_controller"]
  rtDistributed --> agent["distributed::run_agent"]
```

## Flow By Mode

### Local Run
Call chain:
`main -> entry::run -> entry::plan::execute_plan -> application::slice_execution::execute_local -> adapters::runtime::RuntimeLocalPort::run_local -> app::run_local`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["slice_execution::execute_local"]
  a --> r["RuntimeLocalPort::run_local"]
  r --> l["app::run_local"]
  l --> pr["protocol::setup_request_sender"]
  l --> mc["metrics::setup_metrics_collector"]
  l --> ui["ui::render::setup_render_ui"]
```

### Replay
Call chain:
`main -> entry -> slice_execution::execute_replay -> RuntimeReplayPort::run_replay -> app::run_replay`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["slice_execution::execute_replay"]
  a --> r["RuntimeReplayPort::run_replay"]
  r --> l["app::run_replay"]
```

### Compare
Call chain:
`main -> entry -> slice_execution::execute_compare -> RuntimeComparePort::run_compare -> app::run_compare`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["slice_execution::execute_compare"]
  a --> r["RuntimeComparePort::run_compare"]
  r --> l["app::run_compare"]
```

### Cleanup
Call chain:
`main -> entry -> slice_execution::execute_cleanup -> RuntimeCleanupPort::run_cleanup -> app::run_cleanup`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["slice_execution::execute_cleanup"]
  a --> r["RuntimeCleanupPort::run_cleanup"]
  r --> l["app::run_cleanup"]
```

### Distributed Controller
Call chain:
`main -> entry -> distributed_run::execute(controller) -> RuntimeDistributedPort::run_controller -> distributed::run_controller`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["distributed_run::execute(mode=controller)"]
  a --> r["RuntimeDistributedPort::run_controller"]
  r --> d["distributed::run_controller"]
```

### Distributed Agent
Call chain:
`main -> entry -> distributed_run::execute(agent) -> RuntimeDistributedPort::run_agent -> distributed::run_agent -> AgentLocalRunPort -> app::run_local`

```mermaid
flowchart LR
  p["entry::plan::execute_plan"] --> a["distributed_run::execute(mode=agent)"]
  a --> r["RuntimeDistributedPort::run_agent"]
  r --> d["distributed::run_agent"]
  d --> port["AgentLocalRunPort"]
  port --> l["app::run_local"]
```

## Enforced Boundary Checks
Current `scripts/check_architecture.sh` guardrails include:
- `src/application` cannot import `crate::app` or `crate::distributed`.
- `src/distributed` cannot import `crate::app` or `crate::application`.
- `src/entry` cannot import `crate::app`, `crate::distributed`, or `crate::service`.

Latest local run on 2026-02-14 passed these checks.
