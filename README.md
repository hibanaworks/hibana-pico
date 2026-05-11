# hibana-pico

`hibana-pico` is a choreographic WASI Preview 1 microkernel swarm OS for
Raspberry Pi Pico-class boards.

The public proof surface is intentionally small:

- Baker link Dev Rev1 / RP2040: Rust-built `wasm32-wasip1` apps call real
  `wasi_snapshot_preview1.path_open`, `fd_write`, `poll_oneoff`, and
  `proc_exit`; those imports drive ChoreoFS, GuestLedger, memory leases, GPIO,
  timer resolver facts, and fail-safe abort/fence choreography on physical
  hardware.
- Pico 2 W / RP2350 QEMU: one composed swarm choreography connects coordinator
  and sensor nodes over a CYW43439 byte-transport model for remote samples,
  WASI P1 guest `fd_write`, network-object routes, telemetry, and management
  image activation.
- RP2040 SIO smoke: a low-level board diagnostic used only to keep the local
  two-core substrate honest.

The important point is not the payload itself. The important point is that the application logic stays in plain localside form:

```rust
let ping = endpoint.recv::<Msg<LABEL_PING, u8>>().await?;
endpoint.flow::<Msg<LABEL_PONG, u8>>()?.send(&PONG_VALUE).await?;
```

Board-specific code is confined to the downstream transport/backend and boot glue. `hibana` itself does not gain Pico-specific API or vocabulary.

## Current State

The current tree is a working downstream proof, not a finished board support
package. The important state is:

- `hibana` core is not modified for this work; the Pico and Pico 2 W pieces live
  in this crate as downstream transports, runtimes, demos, tests, and QEMU
  model patches.
- Application/session code is still plain hibana localside code:
  `flow::<T>().send(...).await`, `recv::<T>().await`, `offer().await`, and
  `decode::<T>().await`. The `run_current_task(...)` helper is only the outer
  no-std poll/park harness used to run an async task on the bare-metal proof.
- WASI guest apps are Rust-built `wasm32-wasip1` artifacts. Preview 2, WIT,
  component model imports, sockets/resources/streams as kernel concepts, and
  P2 paths are intentionally absent and guarded by tests.
- Guest allocator activity stays guest-internal, but any `memory.grow` that can
  invalidate host-visible `ptr/len` authority is represented as
  `MemFenceReason::MemoryGrow`: old leases and old memory generations are
  rejected, and syscall access must borrow again under the new epoch.
- The first physical RP2040 smoke target is Baker link Dev Rev1. The
  Baker traffic guest is executed by `kernel::engine::wasm::Guest`: core Wasm execution
  stays syscall-agnostic, and the WASI P1 import trampoline maps
  `wasi_snapshot_preview1.path_open`, `fd_write`, and `poll_oneoff` into typed
  Engine requests. `GuestLedger` is the app-local materialized view for this
  path: fds, memory leases, pending `poll_oneoff` tokens, quotas, and errno
  mapping live there, while choreography control messages still own authority
  and legal order. In the ChoreoFS Baker proof,
  `path_open("device/led/green")` admits the Baker object route, then the
  Baker project runtime materializes fd `3`; orange and red materialize fds
  `4` and `5`. Those fds route to GP22, GP21, and GP20. Each visible
  transition is a real WASI import trap that enters the Engine localside; each
  wait is admitted through the timer resolver before Kernel can return
  `PollReady`. The guest does not see the GPIO pins, route label, resolver, or
  hibana endpoint. See
  [baker.md](baker.md) for the physical-board flash, `probe-rs`, and GDB
  workflow.
- The Pico 2 W swarm proof has both a shared firmware image and
  split coordinator/sensor firmware images. The split images reuse one shared
  choreography definition from the library and are launched as ordinary QEMU
  processes over the CYW43439 UDP mesh. The default six-node swarm also has
  node-specific minimal projection images: one coordinator image and one sensor
  image for each of nodes 2 through 6.
- Swarm nodes do not share memory. Cross-node memory and WASI operations are
  represented as typed messages, bounded byte copies, materialized fd views, and
  lease/grant/release control messages in the choreography.
- Interrupt-like and device-ready events are modeled at the resolver boundary:
  timer, GPIO, and transport RX-ready signals become explicit typed progress
  only when the local role's projected program permits it.
- Blocking or readiness-backed WASI calls are represented by GuestLedger-owned
  `PendingSyscallToken`s. The current implementation covers `fd_read`,
  `fd_write`, `poll_oneoff`, clock sleep, network-object send/recv, ChoreoFS
  read/write, directory reads, and listener accept as one bounded token table;
  Baker uses the timer-only `poll_oneoff` case.
- ChoreoFS is now a bounded resource-store implementation rather than a host
  filesystem passthrough: preopen fds authorize manifest lookup, `path_open`
  mints object or directory fds, `StaticBlob` / `ConfigCell` / `AppendLog` /
  directory entries stay bounded, and the core WASI P1 trampoline has std-shaped
  completion helpers for `fd_prestat_*`, `fd_filestat_get`,
  `path_filestat_get`, `path_readlink`, `fd_readdir`, `fd_pread`, `fd_pwrite`,
  `fd_seek`, and `fd_tell`.
- Budget expiry follows the same shape: the resolver admits a budget-ready
  fact, and a bounded Core0 budget controller checks the current run id and
  generation before `BudgetExpired`, `Suspend`, or `Restart` can advance in
  choreography.
- Single-board RP2040 hardware is at public alpha quality. On Baker link Dev
  Rev1, the hardware runner has flashed and verified the original pre-minted
  LED patterns (`traffic`, `chaser`, `ordinary-std`), the ChoreoFS `path_open`
  LED pattern (`choreofs`), and typed-reject patterns for bad order, invalid fd,
  bad payload, forbidden ChoreoFS path, ChoreoFS bad payload, and wrong ChoreoFS
  object. Pico 2 W Wi-Fi swarm remains a QEMU and porting-track proof, not a
  physical Wi-Fi hardware claim.

## Repository Contents

This repository is intended to be publishable as source plus reproducible
proof tooling. Keep these in git:

- Rust source under `src/`, `apps/`, and `tests/`
- linker scripts under `linker/`
- QEMU overlay source and patch files under `qemu/overlay/` and
  `qemu/patches/`
- shell/Python helper scripts under `scripts/`
- documentation, including `README.md`, `baker.md`, `plan.md`, and
  `firmware/cyw43/README.md`
- `Cargo.toml` and `Cargo.lock`

Keep these local-only:

- `target/` and `target/wasip1-apps/`
- generated UF2/ELF/map files
- root-level QEMU configure scratch files: `config.log` and `config-temp/`
- CYW43439 firmware blobs, CLM blobs, generated manifests, disassembly
  excerpts, copied `LICENSE.RP`, and any local Pico SDK checkout under
  `firmware/cyw43/`

The CYW43439 firmware is not redistributed by hibana-pico. Users regenerate it
locally from Pico SDK / `cyw43-driver` by following
`firmware/cyw43/README.md`. The checked-in source tree may reference expected
firmware names and hashes for local verification, but the binary artifacts and
license copy stay ignored.

## What This Proves

- One frozen choreography can be projected into two roles and attached across two RP2040 cores.
- The visible application model remains `hibana::g` plus localside `flow().send()` / `recv()`.
- A board-local substrate can provide async wakeups through RP2040 SIO FIFO and still let the session logic read like normal localside code.
- `hibana` core does not need a Pico-only rescue path for this first proof.
- A narrow engine/supervisor request-reply slice can be expressed as ordinary localside code before introducing a Wasm guest.
- Rust-built WASI P1 guests trap into the same projected localside model without
  changing Kernel protocol authority.
- The backend can expose a real `g::route(...)` decision to the application localside, with Core 0 selecting an arm and Core 1 handling it through `offer().decode()`.
- A Rust-built WASI Preview 1 app can use ordinary `fd_write` plus
  `poll_oneoff` to drive real board-visible GPIO object controls: fds `3,4,5`
  are minted from ChoreoFS `GpioDevice` objects and map to active-high
  GP22/GP21/GP20 traffic-light LEDs. Every wait is admitted through the timer
  resolver and Kernel role, not through app-side GPIO authority.
- Rust-built WASI Preview 1 memory-grow smoke apps prove both sides of the
  lease rule: a post-grow syscall can proceed after a `MemoryGrow` fence and new
  borrow, while a stale pre-grow lease is rejected typed-reject.
- Remote sensor, remote actuator, datagram fd, telemetry, and app-policy payloads are represented as bounded `WirePayload` types and exercised through `hibana::g::Msg` in host parity tests.
- The swarm frame/replay/neighbor/provisioning layer remains a Pico-local transport boundary; semantic authority still comes from hibana labels, projected programs, fd generations, rights, and session generations.
- The Pico 2 W path keeps that same boundary on an RP2350/CYW43439 substrate: Wi-Fi is a transport carrier, while the sample and WASI guest request/reply authority is still wired through hibana.

This is a strong technical proof of `hibana`'s shape. It is not yet a finished OS or a full product demo. What it shows well is that session-typed choreography survives contact with a tiny dual-core MCU without collapsing into ad hoc transport code.

## Architecture

At a high level, every proof follows the same shape:

1. Write one `hibana::g` choreography.
2. Project it into `RoleProgram<N>` values.
3. Attach each projected role to an `Endpoint`.
4. Drive the endpoint directly with localside `send`, `recv`, `offer`, and
   `decode` calls.
5. Let board-local transport/resolver code provide wakeups and bounded byte
   movement, without changing the choreography surface.

The RP2040 demos use two cores in one QEMU machine. `SioTransport` carries
messages through the RP2040 SIO FIFO model, and the board glue wakes the peer
core with the patched `armv7m_set_event()` path. The host tests use
`HostQueueBackend` with the same session shapes so logic can be checked without
QEMU.

The Pico 2 W demos use an RP2350 plus a bounded SPI-facing CYW43439 QEMU model.
For a single-machine smoke, both roles live inside one RP2350 process. For the
swarm proof, each node is a separate QEMU process with its own kernel image,
node id, UDP port, runtime state, and local endpoint. The UDP mesh is only the
transport carrier; the coordinator/sensor sample exchange, WASI P1 `fd_write`,
memory lease, and aggregate broadcast are all ordinary hibana messages inside
one projected swarm choreography.

WASI P1 guest execution is deliberately narrow. The kernel validates generated
Wasm artifacts, extracts the small import/syscall surface used by the proof,
and represents guest actions as typed engine requests. Guest linear memory is
not handed out as shared memory; it is accessed through explicit leases and
remote read/write grants.

## Feature Profiles

Build configuration is Cargo features only. Features select linked
implementation capacity; they do not define choreography or protocol meaning.

Small Pico profiles use `wasm-engine-core` plus only the needed
`wasip1-sys-*` handlers. `wasm-engine-core` means core Wasm execution capacity;
it does not imply `fd_write`, `poll_oneoff`, `proc_exit`, or any other WASI
syscall. The minimum RP2040/Pico profile is:

```text
profile-rp2040-pico-min
  -> platform-rp2040
  -> machine-sio/timer/gpio/uart
  -> wasm-engine-core
  -> wasip1-sys-fd-write + wasip1-sys-poll-oneoff + wasip1-sys-proc-exit
  -> wasip1-ctrl-common
```

The minimum wireless RP2040/Pico W profile adds CYW43439 byte-transport
capacity without changing choreography meaning:

```text
profile-rp2040-picow-swarm-min
  -> platform-rp2040
  -> machine-sio/timer/gpio/uart/cyw43439
  -> machine-cyw43439-real-gspi
  -> wasm-engine-core
  -> selected WASI P1 handlers
  -> wasip1-ctrl-common
  -> swarm-frame + remote/datagram object capacity
```

Status note: Pico W is currently represented as a capacity/profile target.
Dedicated Pico W firmware and physical CYW43439 gates are still pending, so the
RP2040 Baker proof and Pico 2 W QEMU proof must not be read as physical Pico W
Wi-Fi success.

The higher-capacity RP2350/Pico 2 W profile uses the same semantic shape with
RP2350 board capacity:

```text
profile-rp2350-pico2w-swarm-min
  -> platform-rp2350
  -> machine-sio/timer/gpio/uart/cyw43439
  -> machine-cyw43439-real-gspi
  -> wasm-engine-core
  -> selected WASI P1 handlers
  -> wasip1-ctrl-common
  -> swarm-frame + remote/datagram object capacity
```

The host/full profile keeps the ordinary Rust std path explicit:

```text
profile-host-linux-wasip1-full
  -> platform-host-linux
  -> wasm-engine-wasip1-full
  -> wasip1-sys-full (46 WASI Preview 1 imports, including proc_raise)
  -> wasip1-ctrl-common
```

The choreography side is app-agnostic. It does not care whether the guest is a
Rust std app, a no-std app, or another `wasm32-wasip1` producer. It sees only the
typed syscall stream emitted by the engine/import trampoline. Wasm instruction
coverage and fuel/trap behavior are core engine responsibilities. WASI import
validation, `proc_exit`, `fd_write`, `poll_oneoff`, and memory-grow lease fencing
are feature-gated import-trampoline/control responsibilities. If a syscall
handler is not linked, the engine/import path must reject with the typed reject /
ENOSYS / trap path; it must not fake success or select another route.

The host/full gate includes `wasip1-std-core-coverage.wasm`, an ordinary Rust
`fn main()` app that uses `Vec`, `String`/`format!`, `if`/`loop`/`match`,
function pointers, `f32`/`f64`, and `memory.grow`. It is deliberately a larger
platform proof: the choreography is unchanged, while the engine profile decides
whether that artifact can be loaded and driven. The host/full proof uses
`kernel::wasi::host_runner::HostRunner`: it runs the public
`kernel::engine::wasm::Guest` facade,
engine, converts Preview 1 traps into `EngineReq`, completes them through the
bounded fd view / ChoreoFS resource store / network-object ingress, and records the
typed syscall stream. Preview 1 `sock_*` imports are treated only as import
ingress: `sock_send`, `sock_recv`, and `sock_shutdown` normalize into
`FdWrite`, `FdRead`, and `FdClose` over ChoreoFS network objects, while
`sock_accept` only CapMints an accepted network object when an explicit
NetworkListener accept route has been queued/projected. There is no socket authority or
test-side syscall completion path separate from the kernel shape.

## Trace Map

The fastest way to audit a proof is to read one row left to right:

| Proof | Choreography source | Localside driver | Resolver / device boundary |
| --- | --- | --- | --- |
| Timer sleep | `src/choreography/local.rs::timer_sleep_roles` | `tests/host_baker_led_fd.rs` and `src/projects/baker_link_led/runtime.rs` drive `flow::<TimerSleepUntil>().send`, resolver admission, then `flow::<TimerSleepDone>().send` through the typed poll path | `src/kernel/resolver.rs` converts `InterruptEvent::TimerTick` into `ResolvedInterrupt::TimerSleepDone`; `src/kernel/device/timer.rs` only completes due waits |
| WASI clock | `src/choreography/local.rs::wasip1_clock_now_roles` | `tests/host_wasip1_syscalls.rs` drives request/reply with `Wasip1ClockNow` | no interrupt resolver; this is a synchronous clock syscall returning `ClockNow` |
| Baker traffic light | `src/projects/baker_link_led/choreography.rs::traffic_light_roles` | `kernel::engine::wasm::Guest` executes the no-main `wasm32-wasip1` artifact and maps real `wasi_snapshot_preview1.fd_write` / `poll_oneoff` imports into typed `Event::Call` values; `tests/host_baker_led_fd.rs` and `src/projects/baker_link_led/runtime.rs` turn those pending calls into `fd_write("1" / "0") -> Kernel->GPIO -> poll_oneoff -> Kernel->Timer`; `src/kernel/guest_ledger.rs::GuestLedger` owns the app-local fd view, lease, pending, quota, and errno facts | `src/machine/rp2040/baker_link.rs` supplies only Baker Link board facts: the visible GP22/GP21/GP20 user LED pins and safe inactive levels. `src/projects/baker_link_led/manifest.rs` is the LED proof manifest: ChoreoFS `GpioDevice` objects plus fd/pin/active-high/route facts. `src/projects/baker_link_led/ledger.rs` applies those manifest facts to the Baker project `GuestLedger`; `src/kernel/fd_object.rs` checks the materialized fd view, explicit GPIO route, and payload; `src/machine/rp2040/timer.rs` owns TIMER0 top-half/raw readiness; `src/kernel/resolver.rs` admits each timeout as `TimerSleepDone`; UART is a separate local device proof and fail-diagnostic path, not the Baker success criterion |
| Baker ChoreoFS LED open | `src/projects/baker_link_led/choreography.rs::choreofs_traffic_light_roles` | `apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-led-choreofs-open.rs` is the small physical RP2040 proof artifact: a `#![no_main]` `wasm32-wasip1` app exporting `__main_void` that calls preopen-relative `path_open("device/led/green")`, `path_open("device/led/orange")`, `path_open("device/led/red")`, then `fd_write("1"/"0")` and `poll_oneoff`; `src/projects/baker_link_led/runtime.rs` drives every `path_open` as `MemBorrowRead -> PathOpen -> PathOpened -> MemRelease` before the fd_write loop | `src/projects/baker_link_led/manifest.rs` maps ChoreoFS `GpioDevice` objects into the explicit Baker GPIO route, not a generic object route; bad `not/allowed`, `fd_write("on")`, and `fd_write` to a non-GPIO ChoreoFS object are hardware-verified typed-reject patterns |
| Baker fail-safe terminal | `src/projects/baker_link_led/choreography.rs::abort_safe_terminal_roles` and `abort_safe_linear_roles` | `tests/host_baker_led_fd.rs::baker_link_abort_terminal_fences_ledger_and_uses_gpio_choreography_for_safe_state` verifies the full Engine-owned `Abort | Normal` terminal route; the physical `baker-abort-safe-demo` profile drives the selected terminal fragment as `EngineAbort -> AbortBegin -> Fence -> GPIO safe-state -> AbortAck` | `src/kernel/guest_ledger.rs::GuestLedger::apply_abort_fence` makes old fds, leases, and pending tokens stale by generation; Baker safe state uses the existing `Kernel -> GPIO -> Kernel` `GpioSet/GpioSetDone` choreography for GP22/GP21/GP20, while hard panic remains endpoint-free direct MMIO |
| WASI memory grow | `src/choreography/local.rs::memory_grow_stdout_roles` | `tests/host_wasip1_artifacts.rs` drives `MemFence(MemoryGrow)`, rejects the old lease, then borrows again under the new epoch | `kernel::wasi::MemoryLeaseTable` invalidates outstanding leases and rejects stale epochs |
| Ordinary std core coverage | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_ordinary_std_core_coverage_runs_on_host_full_profile` runs the Rust std artifact through `HostRunner` | core Wasm handles control/result values, bulk memory, table/ref, floats, and `memory.grow`; unsupported syscalls remain typed rejects |
| Ordinary std ChoreoFS read | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_std_choreofs_app_uses_resource_store_through_host_full_runner` runs `File::open` / `Read` from a Rust std app through `path_open` and `fd_read` | `src/kernel/choreofs.rs` is the bounded resource store; the app never reaches host filesystem authority |
| Ordinary std ChoreoFS append | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_std_choreofs_append_app_writes_and_reads_resource_store` runs `OpenOptions::append` then `File::open` through `path_open`, `fd_write`, and `fd_read` | append is object control state in ChoreoFS, not host filesystem authority |
| Bad ordinary std path | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_bad_std_path_app_rejects_before_hidden_host_fs` runs a Rust std app that opens a forbidden path | ChoreoFS rejects before fallback host FS access; failure is a typed reject |
| Bad ordinary std ChoreoFS write | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_bad_std_static_write_rejects_at_choreofs_control` runs a Rust std app that writes to a static blob | read-only object control rejects in ChoreoFS before hidden persistence semantics appear |
| WASI P1 network object ingress | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_std_sock_app_uses_network_object_without_p2` runs Preview 1 `sock_send` / `sock_recv` / `sock_shutdown` imports from an ordinary std app | imports normalize into typed `FdWrite` / `FdRead` / `FdClose` over ChoreoFS network objects; there is no P2 socket/resource/stream authority |
| WASI P1 NetworkListener accept | `NetworkAcceptRouteControl` only, no socket vocabulary | `tests/host_wasip1_artifacts.rs::rust_built_std_sock_accept_app_mints_network_object_without_socket_authority` runs Preview 1 `sock_accept`, then uses the returned fd with `sock_send` / `sock_recv` / `sock_shutdown` | NetworkListener route CapMints an accepted network object; follow-up imports still normalize into typed fd syscall stream |
| Bad network accept | no new choreography | `tests/host_wasip1_artifacts.rs::rust_built_bad_std_sock_accept_rejects_without_listener_route` runs Preview 1 `sock_accept` without an explicit accept route | NetworkListener accept is not present, so the network-object ingress rejects typed-reject |
| Pico 2 W swarm | `src/choreography/swarm.rs` | `src/projects/pico2w_swarm/runtime/mod.rs` drives projected node roles over the CYW43439 swarm carrier, including NetworkObjectTable-routed datagram/stream messages | `src/machine/rp2350/cyw43439.rs` and `src/kernel/swarm/mod.rs` keep Wi-Fi as transport/readiness, not semantic authority |

This split is intentional. `src/choreography/local.rs` and
`src/choreography/swarm.rs` should answer “what is the legal order?” The
project and host-test localside code should answer “is the endpoint advanced in
that order?” Resolver and machine modules should answer “how do raw hardware
events become admitted readiness without becoming protocol authority?”

## Scope

Current scope is still proof-oriented:

- `hibana-pico-rp2040-sio-smoke`: low-level RP2040 diagnostic; `Role 1` on
  Core 1 sends `PING`, `Role 0` on Core 0 returns `PONG`, and both cores print
  the handled value. This is a bring-up smoke, not a public application demo
- `hibana-pico-baker-led-demo`: Baker link Dev Rev1 / RP2040 physical smoke;
  Core 1 executes a minimal WASI P1 traffic-light guest with real
  `wasi_snapshot_preview1.fd_write` and `poll_oneoff` imports. Those import
  traps drive fds `3,4,5`: GP22 green, GP21 blinking orange, and GP20 red.
  Core 0 resolves only the explicit GPIO object controls and admits each timeout
  through the timer resolver
- `hibana-pico2w-swarm-demo`: one RP2350/QEMU Pico 2 W can run a dual-core smoke, and multiple QEMU Pico 2 W processes can form a CYW43439 UDP mesh where one global hibana choreography connects the coordinator and every sensor; the coordinator gathers typed remote samples, gates each WASI Preview 1 guest `fd_write`, computes an aggregate, and broadcasts that aggregate back to all sensors. In the default six-process QEMU runner, the same composed choreography then drives an explicit remote actuator route, actuator-to-gateway telemetry, NetworkObjectTable-routed datagram and stream messages over the swarm transport, and a remote management image install/activate sequence with a required fence before the typed image-update event. The sensor guest is the Rust-built `swarm-sensor.wasm` artifact embedded into the firmware image, and the runner checks the `hibana swarm sensor` artifact marker in the coordinator log for every sensor node.
- `hibana-pico2w-swarm-coordinator` and `hibana-pico2w-swarm-sensor`: role-fixed Pico 2 W swarm firmware images using the same shared choreography library and runtime module; these are the size-oriented path for QEMU swarm experiments
- `hibana-pico2w-swarm-coordinator-6` and `hibana-pico2w-swarm-sensor-2` through `hibana-pico2w-swarm-sensor-6`: default-six-node minimal projection images; each image directly references only the projected role program for its own node in the six-node choreography
- `apps/wasip1/swarm-node-apps`: Rust-built `wasm32-wasip1` guest apps for the coordinator, sensor, actuator, and gateway node roles; the gate verifies the generated `.wasm` artifact import sections use only Preview 1 modules, contain no P2/WIT/Component surface, exercise hibana localside syscall choreography from those artifact bytes, put all four app artifacts into one projected swarm choreography, and install a Rust-built artifact through the bounded image-transfer/hot-swap boundary with lease/control-message fencing
- `apps/wasip1/wasip1-smoke-apps`: Rust-built `wasm32-wasip1` smoke apps for stdout, stderr, stdin, clock, random, exit, timer, trap, infinite-loop, Baker LED `fd_write`, ordinary Rust std core coverage, ChoreoFS read/append/static-write rejection, bad std path rejection, network object `sock_*` WASI P1 import ingress, explicit NetworkListener accept, bad `sock_accept` rejection, and memory-grow lease/fence coverage; the same gate checks these artifacts stay Preview 1 and No-P2, including the timer `poll_oneoff`, trap marker, LED `fd_write`, std core coverage marker, ChoreoFS `path_open`/`fd_read`/`fd_write`, network object `sock_send`/`sock_recv`/`sock_shutdown`, NetworkListener `sock_accept`, bad-path / bad-accept typed-reject markers, and memory-grow artifacts
- host-only swarm plan proofs: bounded swarm frames, remote object route routing, datagram fd protocol, three logical node policy flow, and remote-management fencing over ordinary hibana messages

Still in progress for this crate revision:

- `wasm-engine-core` still intentionally omits post-P1 features such as threads,
  atomics, exceptions, GC, and Component Model. The WASI P1 import surface is
  feature-complete for Preview 1, and the bounded core engine covers the P1-era
  control, result, bulk-memory, float, table/ref, and memory-grow paths used by
  the Rust std coverage artifacts. SIMD and post-P1 proposals remain out of
  scope and syscall-agnostic:
  `proc_exit`, `fd_write`, `poll_oneoff`, path/resource store handling, and
  memory-grow lease fencing stay in `wasip1-sys-*` / `wasip1-ctrl-*`.
- physical Pico 2 W CYW43439 firmware-load and driver integration beyond the
  QEMU CYW43439 transport proof
- driver stacks beyond the minimal UART/SIO/Baker LED board glue
- RP2040 SIO smoke remains diagnostic-only; Baker is the RP2040 public proof

## Layout

- `src/choreography/`: common protocol source. `protocol/` is the local
  syscall/device/memory/control payload vocabulary, `swarm.rs` is the shared
  Pico 2 W swarm choreography and per-role projection accessors, and
  `local.rs` documents the local composition layer.
- `src/kernel/`: reusable WASI microkernel pieces that are not board specific.
  This includes app-scoped stream/lease helpers, budget, interrupt resolver,
  management/hot-swap, policy, remote object controls, network object protocols,
  metrics, local swarm transport state, the WASI P1 import trampoline module,
  the bounded Wasm engine facade in `engine::wasm`, and device roles under
  `device/`.
- `src/port/`: board/host byte-port and executor glue. `transport.rs`
  implements the board-local FIFO `hibana::substrate::Transport`,
  `host_queue.rs` contains host queue plus RP2040 SIO backend primitives, and
  `exec.rs` is the tiny no-std poll/park/signal layer.
- `src/machine/`: machine-specific bindings. `rp2040/sio.rs` exposes the SIO
  backend, `rp2040/baker_link.rs` is the Baker Link Dev Rev1 board fact module
  for physical pins and safe levels
  for ChoreoFS LED object facts, active-high GP22/GP21/GP20 one-hot selection,
  and board safe state. It does not own `GuestLedger` fd materialization;
  the Baker project runtime applies those facts after the projected route
  admits progress. `rp2040/timer.rs` owns the TIMER0 IRQ
  top-half and raw readiness slot, `rp2040/uart.rs` owns the UART0 MMIO/debug
  sink used by the local UART device role, and `rp2350/cyw43439.rs` is the
  RP2350 SPI-facing CYW43439 driver for the QEMU swarm carrier.
- `src/projects/`: concrete firmware entrypoints. Public proof binaries live by
  purpose: `rp2040_sio_smoke` is a low-level bring-up diagnostic,
  `baker_link_led` is the RP2040 physical WASI P1 + ChoreoFS + fail-safe proof,
  and `pico2w_swarm` is the RP2350/CYW43439 swarm proof.
- `src/projects/pico2w_swarm/runtime/mod.rs`: shared RP2350/CYW43439 boot,
  runtime, localside session, and WASI P1 `fd_write` harness used by all Pico
  2 W swarm firmware entrypoints.
- `apps/wasip1/swarm-node-apps/`: Rust `wasm32-wasip1` node apps for coordinator, sensor, actuator, and gateway artifacts
- `apps/wasip1/wasip1-smoke-apps/`: Rust `wasm32-wasip1` smoke artifacts for syscall/import coverage
- `scripts/check_wasip1_guest_builds.sh`: guest build and artifact-import gate
- `scripts/check_plan_pico_gates.sh`: focused plan gate covering host proofs, guest builds, firmware builds, the six-process Pico 2 W QEMU swarm runner, No-P2 source checks, and No-bridge/relay source checks
- `scripts/run_choreofs_demo.sh`: supplemental host/full ChoreoFS demo; it
  builds ordinary Rust std `wasm32-wasip1` apps that use `File::open`,
  `OpenOptions::append`, and intentionally bad paths, then runs them through
  `HostRunner` so `path_open`, `fd_read`, and `fd_write` become
  typed WASI P1 boundary events over the bounded ChoreoFS resource store
- `scripts/run_pico2w_swarm_qemu.sh`: multi-process Pico 2 W QEMU swarm runner
- `tests/host_sio_ping_pong.rs`: host parity proof of the same localside ping-pong sequence
- `tests/host_baker_led_fd.rs`: host parity proof of Baker Engine/Kernel
  lifecycle, abort/fence/safe-state, fd object routing, and timer-poll progress
  over projected choreography
- `tests/host_wasip1_syscalls.rs`: host parity proof for the local WASI P1 syscall subset over typed hibana messages, including direct localside `fd_read` with write lease/commit/release, `fd_fdstat_get`, `fd_close`, and `poll_oneoff`
- `tests/host_feature_profiles.rs`: Cargo feature profile proof that Pico
  small and host full-ish WASI P1 capacities are separate implementation
  profiles and that choreography source does not gate protocol shape on
  features
- `tests/host_baker_led_fd.rs`: host parity proof that Baker fds `3,4,5`
  select GP22/GP21/GP20 through memory lease, fd_write choreography,
  `src/kernel/fd_object.rs`, GPIO device role, and resolver-admitted timer
  waits
- `tests/host_swarm_plan.rs`: host parity proof for the Phase 7-13 swarm/fd/network/policy/management model over typed hibana messages where semantic routing is involved
- `qemu/overlay/`: RP2040, RP2350, Raspberry Pi Pico/Pico 2 W, and CYW43439 `.c` / `.h` source files copied into an upstream QEMU checkout
- `qemu/patches/`: patches for existing upstream QEMU files only

## Build And Test

From this directory:

```bash
rustup target add thumbv6m-none-eabi
rustup target add thumbv8m.main-none-eabi
rustup target add wasm32-wasip1
cargo test
cargo build --target thumbv6m-none-eabi --release --bin hibana-pico-rp2040-sio-smoke
bash ./scripts/check_wasip1_guest_builds.sh
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts"
cargo build --target thumbv8m.main-none-eabi --release \
  --bin hibana-pico2w-swarm-demo \
  --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
cargo build --target thumbv8m.main-none-eabi --release \
  --bin hibana-pico2w-swarm-coordinator \
  --bin hibana-pico2w-swarm-sensor \
  --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
cargo build --target thumbv8m.main-none-eabi --release \
  --bin hibana-pico2w-swarm-coordinator-6 \
  --bin hibana-pico2w-swarm-sensor-2 \
  --bin hibana-pico2w-swarm-sensor-3 \
  --bin hibana-pico2w-swarm-sensor-4 \
  --bin hibana-pico2w-swarm-sensor-5 \
  --bin hibana-pico2w-swarm-sensor-6 \
  --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"
```

The host tests prove the positive route branches and the negative early-yield guard with `HostQueueBackend`. The RP2040 builds prove the same session shapes still compile for `thumbv6m-none-eabi`. The Pico 2 W build proves the RP2350/CYW43439 swarm demo compiles for `thumbv8m.main-none-eabi`.

For the focused `plan.md` gate, run:

```bash
bash ./scripts/check_plan_pico_gates.sh
```

This runs the measurement gate tests, including fixed Pico 2 W swarm proof
counts for two-node ping/pong, modeled packet-loss redelivery, join/revoke messages,
and the default six-process sample, WASI, and aggregate phases. It also runs the
resolver-backed timer and rejection-telemetry smoke, the bounded swarm fragmentation smoke, the auth/replay drop-telemetry smoke, the
one-shot interrupt ready-fact consumption smoke, the
GPIO wait fence/revoke smoke, the network object revoke/quiesce, authenticated grant, and rejection-telemetry smokes, the route-control arm id smoke, the
bounded local fd view smoke for invalid, closed, stale-generation, rights, interrupt-subscription fds, gateway route metadata, authorized policy-slot rejection, and local fd/lease rejection telemetry, the
budget-expiry-as-choreography smoke, the
Wasm fuel-exhaustion-to-budget-event smoke, the
artifact-free host fuel-exhaustion-to-budget choreography smoke, the
GPIO wait-through-resolver admitted fact smoke, the
management activation-boundary smoke covering memory leases, interrupt subscriptions, remote object quiescence, network object quiescence, and stale fence-epoch rejection, the
authenticated management install smoke covering forged credentials, stale session generations, wrong-slot grants, bounded rejection telemetry, and management status-code roundtrips for those typed-reject reasons, the
authenticated remote object/resource grant smokes covering forged credentials, stale session generations, wrong-node grants, tampered route metadata, bounded rejection telemetry, and management/telemetry resource route arms, the
routed fd metadata typed-reject smoke, the
two-node Wi-Fi ping/pong-over-swarm smoke, the phone-local and BLE-local
provisioning to swarm-join smokes, the swarm leave/revoke choreography smoke with stale session-generation revoke rejection, the budget-telemetry app
policy route smoke, the single global swarm choreography smoke, the
six-process coordinator-plus-five-sensors swarm choreography smoke, the
production QEMU swarm NetworkObject-to-transport route smoke, the
coordinator/sensor/actuator/gateway telemetry smoke, the wrong-payload and
wrong-localside-label typed-reject smoke, the explicit policy-slot resolver smoke, the remote object route metadata and policy-slot typed-reject smoke, the remote management-fd and telemetry-fd explicit route-arm smokes, the remote packet-loss-no-fallback
smoke, the datagram and bounded stream-segment network-fd-without-P2 smokes with route metadata and authorized policy-slot rejection, the remote image-install-over-swarm smoke, the node-image-update
observer smoke, the remote bad-image rejection smoke, the Rust `wasm32-wasip1` swarm guest build gate for all node
roles, the Rust `wasm32-wasip1` stdout/stderr/stdin/clock/random/exit/timer/trap/infinite-loop/LED-fd-write/LED-blink
smoke app build gate, the generated guest artifact strict import-section Preview 1 / No-P2
checks, the generic WASI `fd_read`/`fd_fdstat_get`/`fd_close`/`poll_oneoff`
localside syscall smokes, the Pico 2 W release build, an artifact-backed localside syscall choreography smoke, the
artifact-backed one-global-swarm-choreography smoke for the coordinator,
sensor, actuator, and gateway apps, the artifact-backed image-slot hot-swap
fence smoke, the Pico 2 W firmware build with the
Rust-built sensor WASI P1 artifact embedded, the split and default-six-node
minimal Pico 2 W swarm firmware builds, the default six-process Pico 2 W QEMU
swarm runner with the minimal node-specific firmware images, the No-P2 source
guard, and the No-bridge/relay source guard.

The plan gate now treats the QEMU swarm runner as part of the achievement
criteria. It looks for a patched `qemu-system-arm` through
`HIBANA_PICO_QEMU_BIN`, `QEMU_BIN`, `../qemu-rp2040/build/qemu-system-arm`, or
`../qemu-upstream/build/qemu-system-arm`, and fails if the selected QEMU does
not expose the `raspberrypi-pico2w` machine. Use
`HIBANA_PICO_SKIP_QEMU_SWARM=1` only for local environments where patched QEMU
is intentionally unavailable.

To view the same artifacts through the Pico budget vocabulary used on the core side, run:

```bash
bash ./scripts/check_pico_demo_budget.sh
```

This reports `flash bytes`, `static sram bytes`, `kernel stack reserve bytes`, and `peak sram upper-bound bytes` for the RP2040 demos. The plan gate runs the same script with `HIBANA_PICO_ENFORCE_PRACTICAL=1`, so the practical `static SRAM <= 48 KiB` / `peak SRAM <= 96 KiB` targets are now enforced for the RP2040 proof binaries.

Current normalized view from the latest run:

| demo | flash bytes | static sram bytes | kernel stack reserve bytes per core | peak sram upper-bound bytes |
| --- | ---: | ---: | ---: | ---: |
| `hibana-pico-rp2040-sio-smoke` | 686588 | 47600 | 24576 | 96752 |
| `hibana-pico-baker-led-demo` | 1097532 | 211952 | 24576 | 261104 |

What this means today:

- flash is inside the practical `768 KiB` budget for the RP2040 demos
- per-core kernel stack reserve is exactly `24 KiB`
- static SRAM and peak SRAM upper-bound are inside the practical proof target after reducing the board-local session slab to `45 KiB`

The Pico 2 W swarm images are measured by release ELF file size in the local
toolchain. These are not stripped flash-image numbers, but they make the current
kernel split visible:

| Pico 2 W swarm image | release ELF bytes | purpose |
| --- | ---: | --- |
| `hibana-pico2w-swarm-demo` | 2583856 | shared image; QEMU passes role/node id at runtime |
| `hibana-pico2w-swarm-coordinator` | 1718208 | role-fixed coordinator, selectable node count |
| `hibana-pico2w-swarm-sensor` | 2351560 | role-fixed sensor, selectable sensor role/node count |
| `hibana-pico2w-swarm-coordinator-6` | 961304 | default-six-node coordinator with only `Role<0>` projection for the six-node choreography |
| `hibana-pico2w-swarm-sensor-2` | 887944 | node 2 image with only `Role<1>` projection for the six-node choreography |
| `hibana-pico2w-swarm-sensor-3` | 888256 | node 3 image with only `Role<2>` projection for the six-node choreography |
| `hibana-pico2w-swarm-sensor-4` | 888288 | node 4 image with only `Role<3>` projection for the six-node choreography |
| `hibana-pico2w-swarm-sensor-5` | 888256 | node 5 image with only `Role<4>` projection for the six-node choreography |
| `hibana-pico2w-swarm-sensor-6` | 888256 | node 6 image with only `Role<5>` projection for the six-node choreography |

## QEMU Patch Base

The QEMU support under `qemu/overlay/` and `qemu/patches/` was prepared against upstream QEMU commit:

```text
da6c4fe60fee30dd77267764d55b38af9cb89d4b
```

If you choose a different upstream base, patch adjustment may be required.

## Build Patched QEMU

The patch set is meant to be applied to a QEMU **source checkout**, not to an
already-installed `qemu-system-arm` binary. You can use a fresh clone or an
existing local checkout. The flow below mirrors the local experiment: configure
only `arm-softmmu`, disable unrelated UI/network/tooling features, then ask
QEMU's Makefile wrapper to build only `qemu-system-arm`.

Install the native dependencies first.

macOS/Homebrew:

```bash
xcode-select --install  # if command line tools are not installed yet
brew install git pkg-config glib pixman
```

Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y \
  git build-essential pkg-config python3 python3-venv \
  libglib2.0-dev libpixman-1-dev zlib1g-dev
```

QEMU still uses Ninja internally through Meson, but Ninja does not need to be
installed globally. If `ninja` is not in `PATH`, create a local/temporary Python
venv and pass it to `configure` with `--ninja`:

```bash
python3 -m venv /tmp/ninja-venv
/tmp/ninja-venv/bin/python -m pip install ninja
NINJA=/tmp/ninja-venv/bin/ninja
```

If your machine already has `ninja`, you can instead use:

```bash
NINJA="$(command -v ninja)"
```

Patch a fresh QEMU clone:

```bash
cd ..
git clone https://github.com/qemu/qemu.git qemu-rp2040
QEMU_SRC="$PWD/qemu-rp2040"
git -C "$QEMU_SRC" checkout da6c4fe60fee30dd77267764d55b38af9cb89d4b
./hibana-pico/qemu/apply-patches.sh "$QEMU_SRC"
```

Or, from `hibana-pico/`, patch an existing local QEMU checkout:

```bash
QEMU_SRC=/path/to/qemu
git -C "$QEMU_SRC" fetch origin
git -C "$QEMU_SRC" checkout da6c4fe60fee30dd77267764d55b38af9cb89d4b
git -C "$QEMU_SRC" switch -c hibana-rp2040
./qemu/apply-patches.sh "$QEMU_SRC"
```

Configure a small ARM-only build from `hibana-pico/`:

```bash
cd /path/to/hibana-pico
QEMU_SRC=../qemu-rp2040
QEMU_BUILD="$QEMU_SRC/build"
mkdir -p "$QEMU_BUILD"
cd "$QEMU_BUILD"

../configure \
  --target-list=arm-softmmu \
  --disable-docs \
  --disable-tools \
  --disable-slirp \
  --disable-vnc \
  --disable-sdl \
  --disable-gtk \
  --disable-cocoa \
  --disable-virglrenderer \
  --disable-opengl \
  --disable-libnfs \
  --disable-smartcard \
  --disable-libusb \
  --disable-nettle \
  --disable-gcrypt \
  --ninja="$NINJA"
```

Build only the ARM system emulator:

```bash
make qemu-system-arm
```

For parallelism, use `make -j"$(sysctl -n hw.ncpu)" qemu-system-arm` on macOS
or `make -j"$(nproc)" qemu-system-arm` on Linux. This is the partial build path;
it does not build all QEMU targets/tools.

The resulting binary is:

```text
../qemu-rp2040/build/qemu-system-arm
```

Confirm that the patched machines are present:

```bash
../qemu-rp2040/build/qemu-system-arm -machine help | grep raspberrypi-pico
```

Expected matches include both `raspberrypi-pico` and `raspberrypi-pico2w`.

The apply script does two things:

- copies the RP2040/RP2350/CYW43439 source/header overlay from `qemu/overlay/`
- applies the remaining diffs from `qemu/patches/`

The resulting tree adds:

- RP2040 SoC modeling
- `raspberrypi-pico` machine support
- RP2040 SIO FIFO support
- an `armv7m_set_event()` hook so one core can wake the other out of `wfe`
- RP2350 dual Cortex-M33 machine plumbing for `raspberrypi-pico2w`
- a bounded SPI-facing CYW43439 Wi-Fi device model for swarm transport tests

## Run The Demo

From `hibana-pico/`:

The supplemental ChoreoFS demo is host/full on purpose. It shows the wider
resource-store side of the same design: ordinary Rust std `wasm32-wasip1` apps
issue Preview 1 `path_open`, `fd_read`, and `fd_write`; the host/full engine
turns those imports into typed boundary events; ChoreoFS grants only manifest
objects such as `config.txt` and `log.txt`; forbidden paths and read-only
objects reject typed-reject before any ambient host filesystem authority appears.
The Baker Link hardware ChoreoFS proof below is the small RP2040 physical
`path_open -> GpioDevice -> fd_write -> GPIO` version of that idea.

```bash
scripts/run_choreofs_demo.sh
```

Use the Baker link Dev Rev1 / RP2040 LED smoke for the first physical-board
validation. The guest-visible interface is ordinary WASI P1: write ASCII `1`
or `0` to fds `3,4,5` and call `poll_oneoff` after each write. The Kernel role
checks fds materialized by the Baker project runtime from ChoreoFS `GpioDevice`
object facts, then through the explicit GPIO route arm, to active-high
GP22, GP21, and GP20. The visible
sequence matches the Baker traffic-light tutorial route order: GP22 green,
GP21 orange blink phases, and GP20 red. The current bring-up timing constants
are tuned in `src/projects/baker_link_led/manifest.rs` for visible real-board
feedback. Do not use fd numbers `0` or `1` for the LEDs; those retain their
standard stdin/stdout meaning. The payload byte selects on/off, while the fd
selects which LED route arm is legal.

```bash
CARGO_TARGET_DIR=$PWD/target/wasip1-apps \
RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
  cargo build --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
  --target wasm32-wasip1 --release --bin wasip1-led-blink

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts"
```

`scripts/check_wasip1_guest_builds.sh` applies the same linker flags to the
Baker min LED artifacts. Without those flags, Rust's default WASI layout uses a
larger initial linear memory that belongs to host/full profiles, not the RP2040
firmware profile.

To flash through a UF2 flow:

```bash
elf2uf2-rs \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo.uf2
```

Copy the UF2 to the board's BOOTSEL mass-storage volume, or flash the ELF with
your Baker link debugger workflow. The detailed physical-board path, including
`probe-rs`, `arm-none-eabi-gdb`, RAM stage reads, and known bring-up pitfalls, is
documented in [baker.md](baker.md). The success criterion is
`HIBANA_DEMO_RESULT == 0x48494f4b`; UART text is not required on the USB debug
connection. A conceptual trace is:

```text
[core0] hibana baker link traffic light
[core1] WASI fd_write(fd=3, "1") -> GP22 green on
[core1] WASI poll_oneoff 250 ticks
[core0] timer IRQ top-half recorded readiness
[core0] resolver admitted timer readiness
[core1] WASI fd_write(fd=4, "1") -> GP21 on
[core1] WASI poll_oneoff 50 ticks
[core1] WASI fd_write(fd=4, "0") -> GP21 off
[core1] WASI poll_oneoff 50 ticks
[core1] WASI fd_write(fd=5, "1") -> GP20 red on
[core1] WASI poll_oneoff 250 ticks
[core0] timer IRQ top-half recorded readiness
[core0] resolver admitted timer readiness
[core1] WASI app returned -> LoopBreak + proc_exit(0)
[core0] break arm observed; no implicit app restart
[core0] baker traffic-light choreography ok
```

The host parity test for this physical smoke is:

```bash
cargo test --test host_baker_led_fd
```

The negative Baker proof uses another Rust-built WASI P1 app:
`wasip1-led-bad-order.wasm`. It deliberately calls `poll_oneoff` before the
choreography has reached the timer phase. The expected result is rejection:
Engine localside cannot open `LABEL_WASI_POLL_ONEOFF` while the projected
program is still at the fd-write memory-borrow phase, and Kernel does not invent
hidden progress. On hardware this is reported as
`HIBANA_DEMO_RESULT == 0x4849524a` and
`HIBANA_DEMO_FAILURE_STAGE == 0x48490043`.

```bash
CARGO_TARGET_DIR=$PWD/target/wasip1-apps \
RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
  cargo build --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
  --target wasm32-wasip1 --release --bin wasip1-led-bad-order

cargo test --test host_baker_led_fd \
  baker_link_bad_order_wasip1_poll_oneoff_is_rejected_before_fd_write_phase

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-bad-order-demo"
```

Two more negative patterns keep the syscall order legal and prove the resolver
boundary. `wasip1-led-invalid-fd.wasm` calls `fd_write(fd=6, "1")`; the fd is
not in the explicit Baker GPIO route arm. `wasip1-led-bad-payload.wasm` calls
`fd_write(fd=3, "2")`; the fd is legal, but the payload is not an allowed LED
control byte. Both stop before any GPIO role progress is created.

```bash
cargo test --test host_baker_led_fd \
  baker_link_invalid_fd_wasip1_app_is_rejected_by_fd_object_without_gpio_progress

cargo test --test host_baker_led_fd \
  baker_link_bad_payload_wasip1_app_is_rejected_by_fd_object_without_gpio_progress

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-invalid-fd-demo"

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-bad-payload-demo"
```

For hardware runs, the helper script builds, flashes, resets, and reads the RAM
markers for each pattern:

```bash
scripts/run_baker_link_hardware_pattern.sh traffic
scripts/run_baker_link_hardware_pattern.sh chaser
scripts/run_baker_link_hardware_pattern.sh ordinary-std
scripts/run_baker_link_hardware_pattern.sh choreofs
scripts/run_baker_link_hardware_pattern.sh bad-order
scripts/run_baker_link_hardware_pattern.sh invalid-fd
scripts/run_baker_link_hardware_pattern.sh bad-payload
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-path
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-payload
scripts/run_baker_link_hardware_pattern.sh choreofs-wrong-object
scripts/run_baker_link_hardware_pattern.sh fail-safe
```

The Baker ChoreoFS hardware proof embeds
`wasip1-led-choreofs-open.wasm`. It is the small physical RP2040 artifact:
a `#![no_main]` `wasm32-wasip1` app exporting `__main_void`. Its first actions
are:

```text
path_open(9, "device/led/green")  -> ChoreoFS GpioDevice -> fd 3
path_open(9, "device/led/orange") -> ChoreoFS GpioDevice -> fd 4
path_open(9, "device/led/red")    -> ChoreoFS GpioDevice -> fd 5
fd_write(fd=3, "1")                -> Kernel -> GPIO -> GP22
poll_oneoff(...)                   -> Timer IRQ -> resolver -> PollReady
```

The matching choreography is
`src/projects/baker_link_led/choreography.rs::choreofs_traffic_light_roles`: three
`MemBorrowRead -> PathOpen -> PathOpened -> MemRelease` cycles followed by the
same Engine-owned `LoopContinue` / `LoopBreak` route used by the normal Baker
traffic light. The forbidden-path variant uses
`src/projects/baker_link_led/choreography.rs::choreofs_bad_path_roles`, a
terminal one-`path_open` proof, so rejection is not hidden by a later traffic
loop. The bad ChoreoFS hardware patterns are:

| Pattern | Expected result |
| --- | --- |
| `choreofs-bad-path` | `not/allowed` rejects at ChoreoFS lookup, `stage=0x4849004b` |
| `choreofs-bad-payload` | `fd_write("on")` reaches the LED object route and rejects at payload policy, `stage=0x4849004c` |
| `choreofs-wrong-object` | `device/not-gpio` can mint an fd, but `fd_write("1")` rejects because the object is not GPIO, `stage=0x4849004d` |

The fail-safe hardware pattern proves the recoverable terminal path without
using a hard-stop escape hatch for a soft abort:

```text
EngineAbort
  -> AbortBegin
  -> Fence
  -> GpioSet(GP22 inactive) -> GpioSetDone
  -> GpioSet(GP21 inactive) -> GpioSetDone
  -> GpioSet(GP20 inactive) -> GpioSetDone
  -> AbortAck
```

The host test keeps the full `Abort | Normal` route proof. The physical Baker
profile runs the selected terminal fragment so the same RP2040 two-core mapping
can keep the safe-state proof small enough for the board. Expected hardware
result is `HIBANA_DEMO_RESULT = 0x48494653`.

The app-owned behavior proof swaps only the WASI guest artifact. The same
`traffic_light_roles()` choreography, same fd view, same memory lease path,
same GPIO role, and same timer interrupt resolver run a chaser pattern:

```text
fd=3 on -> fd=4 on -> fd=5 on -> fd=4 on -> fd=3 on -> fd=4 on -> fd=5 on
```

There is no `if step == 0` route decision in the app or driver. The Baker loop
is the hibana route itself:

```text
BudgetRun;
route(
  LoopContinue + fd_write/poll_oneoff body,
  LoopBreak + proc_exit(0)
)
```

The Engine sends `LoopContinue` once for each syscall step produced by the WASI
guest, so the guest owns the loop count. When the WASI app reaches `Done`, the
Engine sends `LoopBreak + proc_exit(0)`; Kernel only observes the selected arm
through `offer().decode()` and never restarts a non-looping guest behind its
back.

```bash
CARGO_TARGET_DIR=$PWD/target/wasip1-apps \
RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
  cargo build --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
  --target wasm32-wasip1 --release --bin wasip1-led-chaser

cargo test --test host_baker_led_fd \
  baker_link_chaser_wasip1_app_changes_fd_order_without_choreography_changes

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-chaser-demo"
```

`wasip1-led-ordinary-std-chaser.wasm` is the ordinary Rust std proof. It is a
normal `fn main()` `wasm32-wasip1` artifact with Rust std `_start`; the source
has no hand-written `__main_void` trampoline. To fit RP2040 no-alloc firmware it
keeps the app small and calls only the WASI P1 `fd_write` and `poll_oneoff`
imports needed by the traffic light. The default Baker firmware still uses the
smallest no-main artifact, but `baker-ordinary-std-demo` embeds this ordinary std
artifact when it is built with a Pico-sized 64 KiB initial memory. That profile
is not full WASI: it links only the std startup and traffic-light surface
(`args/env`, `fd_write`, `poll_oneoff`, `proc_exit`). The std startup imports are
not answered by a side-channel adapter. This particular app does not emit
`environ_*` at runtime; if it did, Baker would typed reject because those labels
are not part of the Baker traffic choreography. Wider ordinary std coverage
remains the `profile-host-linux-wasip1-full` / larger-platform track.

```bash
CARGO_TARGET_DIR=$PWD/target/wasip1-apps \
RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
  cargo build --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
  --target wasm32-wasip1 --release --bin wasip1-led-ordinary-std-chaser

cargo test --test host_baker_led_fd \
  baker_link_ordinary_std_wasip1_app_fits_embedded_std_start_profile_when_sized

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-ordinary-std-demo"
```

Use the Pico 2 W swarm demo if you want the RP2350 + Wi-Fi substrate proof:

```bash
cargo build --target thumbv8m.main-none-eabi --release \
  --bin hibana-pico2w-swarm-demo \
  --features "profile-rp2350-pico2w-swarm-min embed-wasip1-artifacts"

timeout 8s ../qemu-rp2040/build/qemu-system-arm \
  -M raspberrypi-pico2w \
  -kernel target/thumbv8m.main-none-eabi/release/hibana-pico2w-swarm-demo \
  -nographic \
  -serial mon:stdio
```

Typical Pico 2 W output looks like this:

```text
[core0] hibana pico2w cyw43439 swarm
[core0] init rp2350 + cyw43439 runtime
[core0] cyw43439 firmware ready
[core0] cyw43439 remote sample req
[core1] cyw43439 wait remote sample
[core1] sensor id 0x00000002
[core0] sample value 0x0000a5a5
[core1] sent sample 0x0000a5a5
[core1] wasip1 guest fd_write node 0x00000002
[core0] wasip1 fd_write node 0x00000002: hibana swarm sensor
[core1] wasip1 guest fd_write done
[core0] hibana pico2w cyw43439 swarm ok
```

QEMU keeps running after the success line because the demo parks both cores, so
`timeout` exiting with status `124` is expected when the success line is present.

The command above is the single-QEMU smoke: both roles live inside one RP2350
machine and the CYW43439 uses its built-in bounded queue. The CYW43439 QEMU
model links RX readiness to RP2350 CPU events, so the demo can park with WFE and
wake on UDP RX instead of relying on a spin executor. For the swarm check, run
ordinary QEMU processes and connect their CYW43439 devices over a localhost UDP
mesh. The runner defaults to six processes: one coordinator plus five sensors.

```bash
./scripts/run_pico2w_swarm_qemu.sh ../qemu-rp2040/build/qemu-system-arm
```

To run a smaller swarm, set `HIBANA_PICO_SWARM_NODES=4` or
`HIBANA_PICO_SWARM_NODES=5`. The script starts all sensor nodes first, then the
coordinator. It uses QEMU device properties to select role, node identity, and
the shared UDP port base:

```text
-global cyw43439-wifi.node-role=0
-global cyw43439-wifi.node-id=1
-global cyw43439-wifi.node-count=6
-global cyw43439-wifi.radio-port-base=39000
```

Each node binds `radio-port-base + node-id`, so node 1 uses UDP port `39001`,
node 2 uses `39002`, and so on. The success condition is the coordinator
printing `[core0] hibana pico2w cyw43439 swarm ok` after it has completed all
sensors. In the default six-process run, the coordinator receives distinct
sample values `0x0000a5a5` through `0x0000a5a9` from nodes 2 through 6,
completes one WASI Preview 1 guest `fd_write` exchange with each sensor, and
broadcasts aggregate `0x00033c43`; each sensor must print that the same
aggregate was accepted. The runner also checks the non-hardware-only swarm
phases after the aggregate: node 3 accepts an actuator fd route and emits
gateway telemetry, node 4 consumes that telemetry and accepts datagram/stream
network object traffic, and node 5 accepts the management image transfer, rejects
the first activation until the fence is present, activates the image, and emits
the typed node-image-update event.

The default runner still uses one shared swarm firmware image for every
node and lets QEMU pass `node-role`/`node-id`. For split-size experiments, pass
role-specific images with `HIBANA_PICO_COORD_KERNEL=/path/to/coordinator` and
`HIBANA_PICO_SENSOR_KERNEL=/path/to/sensor`, or set
`HIBANA_PICO_SPLIT_KERNELS=1` to build and run the built-in coordinator/sensor
images; the processes still communicate only through the CYW43439 UDP mesh.

For the default six-node run, the smallest current proof is the node-specific
projection mode:

```bash
HIBANA_PICO_MINIMAL_KERNELS=1 HIBANA_PICO_SWARM_NODES=6 \
  ./scripts/run_pico2w_swarm_qemu.sh ../qemu-rp2040/build/qemu-system-arm
```

That launches one `hibana-pico2w-swarm-coordinator-6` process plus
`hibana-pico2w-swarm-sensor-2` through `hibana-pico2w-swarm-sensor-6`. Each
kernel directly references only its own projected role program from the shared
six-node choreography. You can override individual paths with
`HIBANA_PICO_SENSOR_2_KERNEL` through `HIBANA_PICO_SENSOR_6_KERNEL` when testing
custom node images.

## QEMU Assets

Patch order is recorded in `qemu/patches/series`.

- `0001-armv7m-set-event-and-rp2040-sio.patch`
- `0002-rp2040-soc-and-raspberrypi-pico-machine.patch`
- `0003-rp2350-soc-pico2w-and-cyw43439-wifi.patch`

The RP2040/RP2350/CYW43439 implementation files themselves live under `qemu/overlay/` as normal `.c` and `.h` sources.
Only existing upstream files are kept as `git apply` patches.

The CYW43439 model is deliberately a bounded SPI-facing datagram carrier for
Hibana/Pico swarm experiments. It can run in built-in queue mode for a
single-QEMU smoke test or UDP mesh mode for separate QEMU processes. The
firmware talks to it through the `src/machine/rp2350/cyw43439.rs` driver: power on, reset
assert/release, probe, official Pico SDK firmware/CLM load chunks, NVRAM apply
marker, boot/ready state, node-role discovery, status/readiness, bounded TX/RX
frames, label hints, and overflow status are all explicit SPI commands. QEMU
keeps firmware load closed until power/reset/probe have happened, and keeps
TX/RX closed until the firmware-load prefix reaches ready. RX readiness is only
a wake/demux signal; semantic authority remains in the typed hibana choreography
and the Pico resolver.

The CYW43439 firmware artifact is intentionally local-only under
`firmware/cyw43/`. It is extracted from the Pico SDK `cyw43-driver` firmware
header by `scripts/extract_cyw43_firmware.py`; the generated blobs, manifest,
disassembly excerpt, and copied `LICENSE.RP` are ignored by git. This keeps
Hibana/Pico source distribution separate from the upstream Raspberry Pi firmware
license. The manifest records the source commits, lengths, offsets, SHA-256
hashes, and FNV-1a values used by the QEMU load-prefix verifier.
`scripts/disassemble_cyw43_firmware.sh` regenerates the raw Thumb disassembly in
`target/cyw43/` and updates the local disassembly excerpt.

The model does not emulate the proprietary fullmac firmware, RF, WPA,
Bluetooth controller, or real silicon timing. For real Pico 2 W hardware, the
synthetic QEMU load commands must be replaced by the Pico SDK/libcyw43
firmware-load path or an equivalent bounded gSPI loader for the same
precompiled CYW43439 firmware blob, then bound to the same Hibana/Pico
transport interface. With QEMU swarm and the QEMU-facing firmware-load gate in
place, real CYW43439 gSPI bring-up is the remaining physical porting step.
