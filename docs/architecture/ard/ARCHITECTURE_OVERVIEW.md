# Strest Architecture Overview

_Generated from `src/**/*.rs` on 2026-02-13 13:05:45 UTC_

_Phase 7 migration annotations updated on 2026-02-14._

## Scope
- Module inventory includes all source modules under `src/` (including test modules inside `src`).
- Dependency graph edges are derived from `crate::...` references in non-test source files only.
- Feature-gated modules: `wasm_plugins`, `wasm_runtime`, `fuzzing`, and legacy chart implementations.

## Runtime Flow
```mermaid
flowchart TD
  main["main.rs"] --> entry["entry::run"]
  entry --> planBuild["entry::plan::build_plan"]
  entry --> planExec["entry::plan::execute_plan"]

  planExec --> local["app::run_local"]
  planExec --> controller["distributed::run_controller"]
  planExec --> agent["distributed::run_agent"]
  planExec --> replay["app::run_replay"]
  planExec --> compare["app::run_compare"]
  planExec --> cleanup["app::run_cleanup"]
  planExec --> service["service::handle_service_action"]

  local --> proto["protocol::setup_request_sender"]
  local --> metrics["metrics::setup_metrics_collector"]
  local --> ui["ui::render::setup_render_ui"]
  local --> logs["app::logs::setup_log_sinks"]

  proto --> http["http::setup_request_sender"]
  proto --> transports["protocol::runtime::{tcp/udp/ws/grpc/mqtt}"]

  controller --> ctrlRunner["distributed::controller::runner"]
  ctrlRunner --> ctrlAuto["controller::auto::*"]
  ctrlRunner --> ctrlManual["controller::manual::*"]
  ctrlAuto --> wire["distributed::protocol::{read_message/send_message}"]
  ctrlManual --> wire

  agent --> agentSession["distributed::agent::session"]
  agentSession --> wire
```

## Hexagonal Migration Snapshot (Phase 7)

### Before (Legacy Coupling Path)
```mermaid
flowchart LR
  cli["CLI parse (`TesterArgs`)"]
  plan["entry::plan::build_plan"]
  commands["application::commands\n(owned `TesterArgs`)"]
  local["application::local_run\n(direct `TesterArgs`)"]
  dist["application::distributed_run\n(direct `TesterArgs`)"]
  runtime["Runtime adapters + infra"]

  cli --> plan --> commands
  commands --> local --> runtime
  commands --> dist --> runtime
```

### After (Phase 7 Boundary)
```mermaid
flowchart LR
  cli["CLI/config adapters (`TesterArgs`)"]
  mapper["adapters::cli::mapper"]
  plan["entry::plan\n(command + adapter payload)"]
  appcmd["application::commands\n(run metadata only)"]
  applocal["application::local_run\n(generic ports + `LocalRunSettings`)"]
  appdist["application::distributed_run\n(generic dispatch)"]
  runtime["Runtime adapters + infra (`TesterArgs`)"]

  cli --> mapper --> plan
  plan --> appcmd
  appcmd --> applocal
  appcmd --> appdist
  plan --> runtime
  applocal --> runtime
  appdist --> runtime
```

## Top-Level Dependency Graph
```mermaid
flowchart LR
  n1["app (30)"]
  n2["args (13)"]
  n3["charts (24)"]
  n4["config (16)"]
  n5["distributed (42)"]
  n6["entry (5)"]
  n7["error (11)"]
  n8["fuzzing (1)"]
  n9["http (15)"]
  n10["lib (1)"]
  n11["main (1)"]
  n12["metrics (14)"]
  n13["protocol (21)"]
  n14["script (2)"]
  n15["service (1)"]
  n16["shutdown (1)"]
  n17["sinks (4)"]
  n18["system (5)"]
  n19["ui (17)"]
  n20["wasm_plugins (5)"]
  n21["wasm_runtime (7)"]

  n1 -->|16| n2
  n1 -->|1| n3
  n1 -->|17| n7
  n1 -->|16| n12
  n1 -->|3| n16
  n1 -->|5| n18
  n1 -->|7| n19
  n1 -->|2| n20
  n2 -->|2| n7
  n2 -->|1| n12
  n2 -->|1| n17
  n3 -->|2| n2
  n3 -->|16| n7
  n3 -->|14| n12
  n4 -->|17| n2
  n4 -->|15| n7
  n4 -->|1| n12
  n4 -->|1| n17
  n5 -->|2| n1
  n5 -->|23| n2
  n5 -->|1| n3
  n5 -->|5| n4
  n5 -->|18| n7
  n5 -->|10| n12
  n5 -->|4| n16
  n5 -->|7| n17
  n5 -->|2| n18
  n5 -->|4| n19
  n6 -->|1| n1
  n6 -->|3| n2
  n6 -->|4| n4
  n6 -->|2| n5
  n6 -->|5| n7
  n6 -->|1| n13
  n6 -->|1| n14
  n6 -->|1| n15
  n6 -->|2| n18
  n8 -->|1| n2
  n8 -->|4| n4
  n8 -->|1| n7
  n8 -->|2| n9
  n8 -->|1| n12
  n9 -->|2| n2
  n9 -->|1| n7
  n12 -->|5| n7
  n12 -->|1| n16
  n12 -->|2| n19
  n13 -->|22| n2
  n13 -->|4| n7
  n13 -->|2| n9
  n13 -->|4| n12
  n13 -->|4| n16
  n14 -->|1| n2
  n14 -->|2| n7
  n14 -->|1| n21
  n15 -->|1| n2
  n15 -->|1| n7
  n17 -->|2| n7
  n18 -->|1| n7
  n18 -->|1| n16
  n19 -->|2| n7
  n19 -->|1| n16
  n20 -->|1| n2
  n20 -->|2| n7
  n20 -->|1| n12
  n21 -->|1| n2
  n21 -->|3| n4
  n21 -->|3| n7
```

## Complete Module Hierarchy
```mermaid
flowchart TB
  n22["crate"]
  n22 --> n1
  n22 --> n2
  n22 --> n3
  n22 --> n4
  n22 --> n5
  n22 --> n6
  n22 --> n7
  n22 --> n8
  n22 --> n9
  n22 --> n10
  n22 --> n11
  n22 --> n12
  n22 --> n13
  n22 --> n14
  n22 --> n15
  n22 --> n16
  n22 --> n17
  n22 --> n18
  n22 --> n19
  n22 --> n20
  n22 --> n21
  subgraph sg_app["app"]
    n1["app"]
    n23["app::cleanup"]
    n24["app::compare"]
    n25["app::compare::compare_output"]
    n26["app::export"]
    n27["app::logs"]
    n28["app::logs::merge"]
    n29["app::logs::parsing"]
    n30["app::logs::records"]
    n31["app::logs::setup"]
    n32["app::logs::streaming"]
    n33["app::progress"]
    n34["app::replay"]
    n35["app::replay::bounds"]
    n36["app::replay::records"]
    n37["app::replay::runner"]
    n38["app::replay::snapshots"]
    n39["app::replay::state"]
    n40["app::replay::summary"]
    n41["app::replay::tests"]
    n42["app::replay::ui"]
    n43["app::runner"]
    n44["app::runner::alloc"]
    n45["app::runner::core"]
    n46["app::runner::core::finalize"]
    n47["app::runner::rss"]
    n48["app::runtime_errors"]
    n49["app::summary"]
    n50["app::summary::lines"]
    n51["app::summary::percentiles"]
    n1 --> n23
    n1 --> n24
    n24 --> n25
    n1 --> n26
    n1 --> n27
    n27 --> n28
    n27 --> n29
    n27 --> n30
    n27 --> n31
    n27 --> n32
    n1 --> n33
    n1 --> n34
    n34 --> n35
    n34 --> n36
    n34 --> n37
    n34 --> n38
    n34 --> n39
    n34 --> n40
    n34 --> n41
    n34 --> n42
    n1 --> n43
    n43 --> n44
    n43 --> n45
    n45 --> n46
    n43 --> n47
    n1 --> n48
    n1 --> n49
    n49 --> n50
    n49 --> n51
  end
  subgraph sg_args["args"]
    n2["args"]
    n52["args::cli"]
    n53["args::cli::presets"]
    n54["args::cli::tester"]
    n55["args::defaults"]
    n56["args::parsers"]
    n57["args::tests"]
    n58["args::tests::defaults"]
    n59["args::tests::headers"]
    n60["args::tests::options_core"]
    n61["args::tests::options_extra"]
    n62["args::tests::subcommands"]
    n63["args::types"]
    n2 --> n52
    n52 --> n53
    n52 --> n54
    n2 --> n55
    n2 --> n56
    n2 --> n57
    n57 --> n58
    n57 --> n59
    n57 --> n60
    n57 --> n61
    n57 --> n62
    n2 --> n63
  end
  subgraph sg_charts["charts"]
    n3["charts"]
    n64["charts::aggregated"]
    n65["charts::aggregated::buckets"]
    n66["charts::aggregated::latency"]
    n67["charts::aggregated::rps"]
    n68["charts::aggregated::util"]
    n69["charts::average"]
    n70["charts::cumulative"]
    n71["charts::driver"]
    n72["charts::driver::naming"]
    n73["charts::driver::plotting"]
    n74["charts::errors"]
    n75["charts::inflight"]
    n76["charts::latency"]
    n77["charts::rps"]
    n78["charts::status"]
    n79["charts::streaming"]
    n80["charts::streaming::basic"]
    n81["charts::streaming::basic::buckets"]
    n82["charts::streaming::basic::counts"]
    n83["charts::streaming::breakdown"]
    n84["charts::streaming::latency"]
    n85["charts::tests"]
    n86["charts::timeouts"]
    n3 --> n64
    n64 --> n65
    n64 --> n66
    n64 --> n67
    n64 --> n68
    n3 --> n69
    n3 --> n70
    n3 --> n71
    n71 --> n72
    n71 --> n73
    n3 --> n74
    n3 --> n75
    n3 --> n76
    n3 --> n77
    n3 --> n78
    n3 --> n79
    n79 --> n80
    n80 --> n81
    n80 --> n82
    n79 --> n83
    n79 --> n84
    n3 --> n85
    n3 --> n86
  end
  subgraph sg_config["config"]
    n4["config"]
    n87["config::apply"]
    n88["config::apply::distributed"]
    n89["config::apply::load"]
    n90["config::apply::scenario"]
    n91["config::apply::section_basic"]
    n92["config::apply::section_runtime"]
    n93["config::apply::section_runtime::section_runtime_network"]
    n94["config::apply::section_runtime::section_runtime_output"]
    n95["config::apply::section_tail"]
    n96["config::apply::util"]
    n97["config::loader"]
    n98["config::parse"]
    n99["config::test_support"]
    n100["config::tests"]
    n101["config::types"]
    n4 --> n87
    n87 --> n88
    n87 --> n89
    n87 --> n90
    n87 --> n91
    n87 --> n92
    n92 --> n93
    n92 --> n94
    n87 --> n95
    n87 --> n96
    n4 --> n97
    n4 --> n98
    n4 --> n99
    n4 --> n100
    n4 --> n101
  end
  subgraph sg_distributed["distributed"]
    n5["distributed"]
    n102["distributed::agent"]
    n103["distributed::agent::command"]
    n104["distributed::agent::run_exec"]
    n105["distributed::agent::session"]
    n106["distributed::agent::wire"]
    n107["distributed::controller"]
    n108["distributed::controller::agent"]
    n109["distributed::controller::auto"]
    n110["distributed::controller::auto::events"]
    n111["distributed::controller::auto::finalize"]
    n112["distributed::controller::auto::setup"]
    n113["distributed::controller::control"]
    n114["distributed::controller::http"]
    n115["distributed::controller::load"]
    n116["distributed::controller::manual"]
    n117["distributed::controller::manual::connections"]
    n118["distributed::controller::manual::control_http"]
    n119["distributed::controller::manual::loop_handlers"]
    n120["distributed::controller::manual::loop_idle"]
    n121["distributed::controller::manual::orchestrator"]
    n122["distributed::controller::manual::run_finalize"]
    n123["distributed::controller::manual::run_lifecycle"]
    n124["distributed::controller::manual::state"]
    n125["distributed::controller::runner"]
    n126["distributed::controller::shared"]
    n127["distributed::controller::shared::aggregation"]
    n128["distributed::controller::shared::events"]
    n129["distributed::controller::shared::timing"]
    n130["distributed::controller::shared::ui"]
    n131["distributed::controller::tests"]
    n132["distributed::controller::tests::aggregation"]
    n133["distributed::controller::tests::ui"]
    n134["distributed::protocol"]
    n135["distributed::protocol::io"]
    n136["distributed::protocol::types"]
    n137["distributed::summary"]
    n138["distributed::tests"]
    n139["distributed::tests::sink_runs"]
    n140["distributed::tests::wire_args"]
    n141["distributed::utils"]
    n142["distributed::wire"]
    n5 --> n102
    n102 --> n103
    n102 --> n104
    n102 --> n105
    n102 --> n106
    n5 --> n107
    n107 --> n108
    n107 --> n109
    n109 --> n110
    n109 --> n111
    n109 --> n112
    n107 --> n113
    n107 --> n114
    n107 --> n115
    n107 --> n116
    n116 --> n117
    n116 --> n118
    n116 --> n119
    n116 --> n120
    n116 --> n121
    n116 --> n122
    n116 --> n123
    n116 --> n124
    n107 --> n125
    n107 --> n126
    n126 --> n127
    n126 --> n128
    n126 --> n129
    n126 --> n130
    n107 --> n131
    n131 --> n132
    n131 --> n133
    n5 --> n134
    n134 --> n135
    n134 --> n136
    n5 --> n137
    n5 --> n138
    n138 --> n139
    n138 --> n140
    n5 --> n141
    n5 --> n142
  end
  subgraph sg_entry["entry"]
    n6["entry"]
    n143["entry::plan"]
    n144["entry::plan::build"]
    n145["entry::plan::execute"]
    n146["entry::plan::types"]
    n6 --> n143
    n143 --> n144
    n143 --> n145
    n143 --> n146
  end
  subgraph sg_error["error"]
    n7["error"]
    n147["error::app"]
    n148["error::config"]
    n149["error::distributed"]
    n150["error::http"]
    n151["error::metrics"]
    n152["error::script"]
    n153["error::service"]
    n154["error::sink"]
    n155["error::test_support"]
    n156["error::validation"]
    n7 --> n147
    n7 --> n148
    n7 --> n149
    n7 --> n150
    n7 --> n151
    n7 --> n152
    n7 --> n153
    n7 --> n154
    n7 --> n155
    n7 --> n156
  end
  subgraph sg_fuzzing["fuzzing"]
    n8["fuzzing"]
  end
  subgraph sg_http["http"]
    n9["http"]
    n157["http::rate"]
    n158["http::sender"]
    n159["http::sender::config"]
    n160["http::sender::worker"]
    n161["http::tests"]
    n162["http::tls"]
    n163["http::workload"]
    n164["http::workload::builders"]
    n165["http::workload::builders_auth"]
    n166["http::workload::data"]
    n167["http::workload::execution"]
    n168["http::workload::runner"]
    n169["http::workload::runner_common"]
    n170["http::workload::template"]
    n9 --> n157
    n9 --> n158
    n158 --> n159
    n158 --> n160
    n9 --> n161
    n9 --> n162
    n9 --> n163
    n163 --> n164
    n163 --> n165
    n163 --> n166
    n163 --> n167
    n163 --> n168
    n163 --> n169
    n163 --> n170
  end
  subgraph sg_lib["lib"]
    n10["lib"]
  end
  subgraph sg_main["main"]
    n11["main"]
  end
  subgraph sg_metrics["metrics"]
    n12["metrics"]
    n171["metrics::collector"]
    n172["metrics::collector::helpers"]
    n173["metrics::collector::helpers::processing"]
    n174["metrics::collector::helpers::summary"]
    n175["metrics::collector::helpers::windows"]
    n176["metrics::collector::state"]
    n177["metrics::histogram"]
    n178["metrics::logging"]
    n179["metrics::logging::reader"]
    n180["metrics::logging::writer"]
    n181["metrics::logging::writer::db"]
    n182["metrics::tests"]
    n183["metrics::types"]
    n12 --> n171
    n171 --> n172
    n172 --> n173
    n172 --> n174
    n172 --> n175
    n171 --> n176
    n12 --> n177
    n12 --> n178
    n178 --> n179
    n178 --> n180
    n180 --> n181
    n12 --> n182
    n12 --> n183
  end
  subgraph sg_protocol["protocol"]
    n13["protocol"]
    n184["protocol::builtins"]
    n185["protocol::examples"]
    n186["protocol::examples::chat_websocket"]
    n187["protocol::examples::game_udp"]
    n188["protocol::examples::telemetry_mqtt"]
    n189["protocol::registry"]
    n190["protocol::runtime"]
    n191["protocol::runtime::datagram"]
    n192["protocol::runtime::grpc"]
    n193["protocol::runtime::mqtt"]
    n194["protocol::runtime::resolve"]
    n195["protocol::runtime::spawner"]
    n196["protocol::runtime::tests"]
    n197["protocol::runtime::tests::datagram_mqtt"]
    n198["protocol::runtime::tests::scheme_resolution"]
    n199["protocol::runtime::tests::transport_http_grpc"]
    n200["protocol::runtime::transports"]
    n201["protocol::runtime::types"]
    n202["protocol::tests"]
    n203["protocol::traits"]
    n13 --> n184
    n13 --> n185
    n185 --> n186
    n185 --> n187
    n185 --> n188
    n13 --> n189
    n13 --> n190
    n190 --> n191
    n190 --> n192
    n190 --> n193
    n190 --> n194
    n190 --> n195
    n190 --> n196
    n196 --> n197
    n196 --> n198
    n196 --> n199
    n190 --> n200
    n190 --> n201
    n13 --> n202
    n13 --> n203
  end
  subgraph sg_script["script"]
    n14["script"]
    n204["script::loader"]
    n14 --> n204
  end
  subgraph sg_service["service"]
    n15["service"]
  end
  subgraph sg_shutdown["shutdown"]
    n16["shutdown"]
  end
  subgraph sg_sinks["sinks"]
    n17["sinks"]
    n205["sinks::config"]
    n206["sinks::format"]
    n207["sinks::writers"]
    n17 --> n205
    n17 --> n206
    n17 --> n207
  end
  subgraph sg_system["system"]
    n18["system"]
    n208["system::banner"]
    n209["system::logger"]
    n210["system::probestack"]
    n211["system::shutdown_handlers"]
    n18 --> n208
    n18 --> n209
    n18 --> n210
    n18 --> n211
  end
  subgraph sg_ui["ui"]
    n19["ui"]
    n212["ui::model"]
    n213["ui::render"]
    n214["ui::render::charts"]
    n215["ui::render::charts_status_data"]
    n216["ui::render::charts_window"]
    n217["ui::render::dashboard"]
    n218["ui::render::formatting"]
    n219["ui::render::frame"]
    n220["ui::render::lifecycle"]
    n221["ui::render::progress"]
    n222["ui::render::summary"]
    n223["ui::render::summary_panels_metrics"]
    n224["ui::render::summary_panels_quality"]
    n225["ui::render::summary_run"]
    n226["ui::render::theme"]
    n227["ui::tests"]
    n19 --> n212
    n19 --> n213
    n213 --> n214
    n213 --> n215
    n213 --> n216
    n213 --> n217
    n213 --> n218
    n213 --> n219
    n213 --> n220
    n213 --> n221
    n213 --> n222
    n213 --> n223
    n213 --> n224
    n213 --> n225
    n213 --> n226
    n19 --> n227
  end
  subgraph sg_wasm_plugins["wasm_plugins"]
    n20["wasm_plugins"]
    n228["wasm_plugins::constants"]
    n229["wasm_plugins::host"]
    n230["wasm_plugins::tests"]
    n231["wasm_plugins::validate"]
    n20 --> n228
    n20 --> n229
    n20 --> n230
    n20 --> n231
  end
  subgraph sg_wasm_runtime["wasm_runtime"]
    n21["wasm_runtime"]
    n232["wasm_runtime::constants"]
    n233["wasm_runtime::loader"]
    n234["wasm_runtime::module"]
    n235["wasm_runtime::parse"]
    n236["wasm_runtime::tests"]
    n237["wasm_runtime::validate"]
    n21 --> n232
    n21 --> n233
    n21 --> n234
    n21 --> n235
    n21 --> n236
    n21 --> n237
  end
```

## Module Inventory
### `app` (30)
```text
app
app::cleanup
app::compare
app::compare::compare_output
app::export
app::logs
app::logs::merge
app::logs::parsing
app::logs::records
app::logs::setup
app::logs::streaming
app::progress
app::replay
app::replay::bounds
app::replay::records
app::replay::runner
app::replay::snapshots
app::replay::state
app::replay::summary
app::replay::tests
app::replay::ui
app::runner
app::runner::alloc
app::runner::core
app::runner::core::finalize
app::runner::rss
app::runtime_errors
app::summary
app::summary::lines
app::summary::percentiles
```
### `args` (13)
```text
args
args::cli
args::cli::presets
args::cli::tester
args::defaults
args::parsers
args::tests
args::tests::defaults
args::tests::headers
args::tests::options_core
args::tests::options_extra
args::tests::subcommands
args::types
```
### `charts` (24)
```text
charts
charts::aggregated
charts::aggregated::buckets
charts::aggregated::latency
charts::aggregated::rps
charts::aggregated::util
charts::average
charts::cumulative
charts::driver
charts::driver::naming
charts::driver::plotting
charts::errors
charts::inflight
charts::latency
charts::rps
charts::status
charts::streaming
charts::streaming::basic
charts::streaming::basic::buckets
charts::streaming::basic::counts
charts::streaming::breakdown
charts::streaming::latency
charts::tests
charts::timeouts
```
### `config` (16)
```text
config
config::apply
config::apply::distributed
config::apply::load
config::apply::scenario
config::apply::section_basic
config::apply::section_runtime
config::apply::section_runtime::section_runtime_network
config::apply::section_runtime::section_runtime_output
config::apply::section_tail
config::apply::util
config::loader
config::parse
config::test_support
config::tests
config::types
```
### `distributed` (42)
```text
distributed
distributed::agent
distributed::agent::command
distributed::agent::run_exec
distributed::agent::session
distributed::agent::wire
distributed::controller
distributed::controller::agent
distributed::controller::auto
distributed::controller::auto::events
distributed::controller::auto::finalize
distributed::controller::auto::setup
distributed::controller::control
distributed::controller::http
distributed::controller::load
distributed::controller::manual
distributed::controller::manual::connections
distributed::controller::manual::control_http
distributed::controller::manual::loop_handlers
distributed::controller::manual::loop_idle
distributed::controller::manual::orchestrator
distributed::controller::manual::run_finalize
distributed::controller::manual::run_lifecycle
distributed::controller::manual::state
distributed::controller::runner
distributed::controller::shared
distributed::controller::shared::aggregation
distributed::controller::shared::events
distributed::controller::shared::timing
distributed::controller::shared::ui
distributed::controller::tests
distributed::controller::tests::aggregation
distributed::controller::tests::ui
distributed::protocol
distributed::protocol::io
distributed::protocol::types
distributed::summary
distributed::tests
distributed::tests::sink_runs
distributed::tests::wire_args
distributed::utils
distributed::wire
```
### `entry` (5)
```text
entry
entry::plan
entry::plan::build
entry::plan::execute
entry::plan::types
```
### `error` (11)
```text
error
error::app
error::config
error::distributed
error::http
error::metrics
error::script
error::service
error::sink
error::test_support
error::validation
```
### `fuzzing` (1)
```text
fuzzing
```
### `http` (15)
```text
http
http::rate
http::sender
http::sender::config
http::sender::worker
http::tests
http::tls
http::workload
http::workload::builders
http::workload::builders_auth
http::workload::data
http::workload::execution
http::workload::runner
http::workload::runner_common
http::workload::template
```
### `lib` (1)
```text
lib
```
### `main` (1)
```text
main
```
### `metrics` (14)
```text
metrics
metrics::collector
metrics::collector::helpers
metrics::collector::helpers::processing
metrics::collector::helpers::summary
metrics::collector::helpers::windows
metrics::collector::state
metrics::histogram
metrics::logging
metrics::logging::reader
metrics::logging::writer
metrics::logging::writer::db
metrics::tests
metrics::types
```
### `protocol` (21)
```text
protocol
protocol::builtins
protocol::examples
protocol::examples::chat_websocket
protocol::examples::game_udp
protocol::examples::telemetry_mqtt
protocol::registry
protocol::runtime
protocol::runtime::datagram
protocol::runtime::grpc
protocol::runtime::mqtt
protocol::runtime::resolve
protocol::runtime::spawner
protocol::runtime::tests
protocol::runtime::tests::datagram_mqtt
protocol::runtime::tests::scheme_resolution
protocol::runtime::tests::transport_http_grpc
protocol::runtime::transports
protocol::runtime::types
protocol::tests
protocol::traits
```
### `script` (2)
```text
script
script::loader
```
### `service` (1)
```text
service
```
### `shutdown` (1)
```text
shutdown
```
### `sinks` (4)
```text
sinks
sinks::config
sinks::format
sinks::writers
```
### `system` (5)
```text
system
system::banner
system::logger
system::probestack
system::shutdown_handlers
```
### `ui` (17)
```text
ui
ui::model
ui::render
ui::render::charts
ui::render::charts_status_data
ui::render::charts_window
ui::render::dashboard
ui::render::formatting
ui::render::frame
ui::render::lifecycle
ui::render::progress
ui::render::summary
ui::render::summary_panels_metrics
ui::render::summary_panels_quality
ui::render::summary_run
ui::render::theme
ui::tests
```
### `wasm_plugins` (5)
```text
wasm_plugins
wasm_plugins::constants
wasm_plugins::host
wasm_plugins::tests
wasm_plugins::validate
```
### `wasm_runtime` (7)
```text
wasm_runtime
wasm_runtime::constants
wasm_runtime::loader
wasm_runtime::module
wasm_runtime::parse
wasm_runtime::tests
wasm_runtime::validate
```
