# Baker Link Hardware Proof Notes

This note records the current Baker Link / RP2040 proof path after the
AppKit Capsule refactor.

## Current Shape

The Baker example package has a common library plus one bin target per hardware
validation pattern. Each selected bin is one physical Cargo artifact, and that
artifact contains two logical images:

```text
examples/baker-firmware
  src/lib.rs: Baker boot2, clock init, SIO carrier, markers, reset support, logical image helpers
  src/bin/traffic.rs: Capsule + choreography + Localside
  src/bin/choreofs_traffic.rs: Capsule + choreography + Localside + ChoreoFS object facts
  src/bin/choreofs_traffic_loop.rs: Capsule + choreography + Localside + ChoreoFS object facts
  src/bin/fail_safe.rs: Capsule + choreography + Localside
  src/bin/recovery.rs: Capsule + choreography + Localside
  src/bin/many_reentry.rs: Capsule + choreography + Localside
  src/bin/panic_marker.rs: firmware panic marker proof
  src/bin/endpoint_fault.rs: endpoint error evidence proof
  src/bin/endpoint_poison.rs: poisoned generation proof
  src/bin/preview_probe.rs: route-observation preview proof
  src/bin/deadline_fault.rs: operational deadline fault proof
  src/bin/timer_route.rs: protocol timer route proof
  src/bin/epf_policy_timer.rs: loaded EPF policy VM route proof
  wasip1/guest/src/lib.rs: Baker WASI P1 guest helpers
  wasip1/guest/src/bin/*.rs: Baker WASI P1 guest programs
  Core0 logical image: private driver marker
  Core1 logical image: private engine marker
```

Both images are projections of the same raw Hibana choreography. Each
`appkit::run::<LogicalImage, Capsule>()` attaches only that image's requested
role slice:

```text
Core0 driver requested roles = role 0
Core1 engine requested roles = role 1
```

The two logical images are connected by the real RP2040 SIO carrier owned by
the Baker example. Same firmware, same ELF, and same address space do not mean
direct call, authority merge, or syscall shortcut.

The current source map is:

| Layer | File |
| --- | --- |
| AppKit capsule/logical-image integration | `src/appkit/mod.rs`, `src/appkit/internal.rs` |
| Generic logical-site marker | `src/appkit/internal.rs` via `hibana_pico::appkit::Local` |
| Baker-local RP2040 SIO carrier | `examples/baker-firmware/src/lib.rs` |
| Baker logical-image/reset support | `examples/baker-firmware/src/lib.rs` |
| Baker-local boot2 and clock setup | `examples/baker-firmware/src/lib.rs` |
| Baker validation Capsules, choreography, Localside, ChoreoFS object facts | `examples/baker-firmware/src/bin/*.rs` |
| Baker WASI P1 guest helpers | `examples/baker-firmware/wasip1/guest/src/lib.rs` |
| WASI P1 finite ChoreoFS traffic guest | `examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-once.rs` |
| WASI P1 loop ChoreoFS traffic guest | `examples/baker-firmware/wasip1/guest/src/bin/wasip1-led-choreofs-traffic-cycle.rs` |
| WASI P1 guest build gate | `scripts/check_wasip1_guest_builds.sh` |
| Hardware proof runner | `scripts/run_baker_link_hardware_pattern.sh` |

## What Is Proved

The hardware runner flashes the firmware and reads RAM markers by symbol. A
successful flash alone is not a proof; the RAM markers are the evidence.

Baker does not depend on an external boot/runtime crate. Its second-stage
W25Q080 boot block and RP2040 clock setup live in
`examples/baker-firmware/src/lib.rs`. Reset establishes the Pico SDK-equivalent
clock shape used by these proofs: XOSC stable at 12MHz, `clk_ref` sourced from
XOSC, PLL_SYS configured as 1500MHz VCO with post dividers 6 and 2, `clk_sys`
sourced from PLL_SYS at 125MHz, `clk_peri` sourced from `clk_sys`, and watchdog
tick generation set to 1MHz from the 12MHz XOSC. The hardware runner reads the
MMIO registers for those facts after flashing.

The currently supported patterns are:

```sh
bash scripts/run_baker_link_hardware_pattern.sh traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic-loop
bash scripts/run_baker_link_hardware_pattern.sh fail-safe
bash scripts/run_baker_link_hardware_pattern.sh recovery
bash scripts/run_baker_link_hardware_pattern.sh many-reentry
bash scripts/run_baker_link_hardware_pattern.sh panic-marker
bash scripts/run_baker_link_hardware_pattern.sh endpoint-fault
bash scripts/run_baker_link_hardware_pattern.sh endpoint-poison
bash scripts/run_baker_link_hardware_pattern.sh preview-probe
bash scripts/run_baker_link_hardware_pattern.sh deadline-fault
bash scripts/run_baker_link_hardware_pattern.sh timer-route
bash scripts/run_baker_link_hardware_pattern.sh epf-policy-timer
bash scripts/run_baker_link_hardware_pattern.sh capacity-fault
```

`traffic`, `fail-safe`, `recovery`, and `many-reentry` are two-role endpoint /
carrier control proofs. They no longer use a role-2 boundary shortcut or a
composite `0b101` attach slice.

`choreofs-traffic` and `choreofs-traffic-loop` are the WASI P1 proofs. Core1
runs the Hibana WASIP1 runtime engine and Core0 runs the driver side. The
guests are ordinary `std` `wasm32-wasip1` programs. They do not call board APIs;
they reach the hardware only by making WASI P1 imports that cross the projected
Endpoint/carrier frontier.

Both proofs start by opening the same ChoreoFS LED object paths:

```text
path_open("device/led/green")
path_open("device/led/yellow")
path_open("device/led/red")
```

`choreofs-traffic` embeds the finite `wasip1-led-choreofs-traffic-once.wasm`
guest. It performs one green/yellow/red cycle and exits through the real WASI
`proc_exit` boundary.

`choreofs-traffic-loop` embeds the looping
`wasip1-led-choreofs-traffic-cycle.wasm` guest. Its projected choreography admits
reentry over the actual `fd_write` / `poll_oneoff` imports:

```text
loop {
  fd_write(green, "1"); poll_oneoff(...)
  fd_write(yellow, "0"); poll_oneoff(...)
  fd_write(red, "0"); poll_oneoff(...)
  ...
}
```

The guest imports complete through:

```text
WASI P1 guest
  -> Hibana WASIP1 runtime engine side
  -> typed EngineReq
  -> Endpoint / RP2040 SIO carrier
  -> Driver side
  -> ledger / ChoreoFS / resolver / boundary facts
  -> typed EngineRet
  -> Endpoint / RP2040 SIO carrier
  -> Hibana WASIP1 runtime engine side
  -> import completion
```

There is no host filesystem authority, route inference, timeout rescue,
lane-recovery loop, or co-located syscall completion.

`panic-marker`, `endpoint-fault`, `endpoint-poison`, and `deadline-fault` are
negative evidence proofs. They intentionally terminate through the firmware
panic/fault marker path and verify that the recorded evidence carries the
operation, location, or deadline cause instead of silently continuing the same
session generation.

`preview-probe` proves that route-observation hints can cross SIO as preview
evidence without becoming route authority. `timer-route` is the protocol-time
counterpart to `deadline-fault`: a timer/clock fact is present in the
choreography and resolver-selected route arm, so the expired branch is typed
progress rather than an operational timeout rescue path.

`epf-policy-timer` proves that loaded EPF policy bytecode can affect a Hibana
dynamic policy point on RP2040/SIO without becoming a hook or side channel. The
BakerLink runner commits the framed `Target::Policy(57)` bytes into the SWD
mailbox and sets only an image-ready fact. A Hibana route resolver reads that
fact and selects the image-load branch; role0 then delivers the staged image to
role1 as a normal SIO choreography payload. The later timer IRQ fact resolver
reads the RP2040 timer IRQ-ready fact and selects the timer-expired arm after
the interrupt fires; each EPF-wrapped resolver entry feeds the timer IRQ fact to
EPF as Hibana evidence, and the loaded policy VM selects the response-ready arm
instead. Both core0 and core1 also drain real Hibana `TapEvent`s through EPF
observe markers.

## ChoreoFS Scope

ChoreoFS is a bounded path/object fact resolver. It is not a host filesystem,
route owner, protocol authority, public Manifest API, POSIX emulation layer, or
hidden progress path.

For the Baker proof:

```text
ChoreoFS:
  path string -> selector -> object facts

Ledger:
  object facts -> fd materialized view

Choreography:
  RouteDecision / legal order / phase authority
```

The `choreofs-traffic` pattern opens the three configured LED object paths,
mints their fds through the driver-side materialization path, then performs one
green/yellow/red traffic cycle through the projected choreography. The finite
guest then exits through the real WASI `proc_exit` boundary. The hardware proof
checks:

```text
choreofs_engine_status = 0x57414f4b
choreofs_path_open_count = 3
choreofs_fd_write_count = 13
choreofs_poll_count = 13
choreofs_last_object = 3
choreofs_led_mask = 4
choreofs_seen_led_mask = 7
```

The `choreofs-traffic-loop` pattern leaves the guest and driver in the visual
loop. The runner does not require a fixed final LED mask or final object for
that mode; it checks that at least one full green/yellow/red cycle was observed:

```text
choreofs_path_open_count = 3
choreofs_fd_write_count >= 13
choreofs_poll_count >= 13
choreofs_seen_led_mask = 7
```

## Result Markers

The runner resolves marker addresses with `llvm-nm` instead of hard-coding RAM
addresses.

Important result values:

| Value | Meaning |
| --- | --- |
| `0x48494f4b` | traffic / ChoreoFS traffic success |
| `0x48494653` | fail-safe proof success |
| `0x48495243` | recovery proof success |
| `0x4849524d` | many-reentry proof success |
| `0x48495050` | preview-probe proof success |
| `0x48495452` | timer-route proof success |
| `0x48494550` | EPF policy timer proof success |
| `0x48494641` | hard failure |

Important stage values:

| Value | Meaning |
| --- | --- |
| `0x4849000a` | runtime ready |
| `0x48490004` | engine runtime begin/ready stage accepted for non-WASI control patterns |
| `0x48490f10` | WASI engine error |
| `0x48490f11` | ChoreoFS driver error |
| `0x48490f12` | control-flow proof error |

The runner also verifies:

```text
failure stage == 0
hardfault pc/lr == 0
core0 and core1 stack high-water marks are non-zero and <= 8 KiB
```

## Hardware Evidence Shape

The hardware runner verifies these marker shapes on Baker Link hardware:

```text
traffic:
  result=0x48494f4b
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

choreofs-traffic:
  result=0x48494f4b
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0
  choreofs_engine_status=0x57414f4b
  choreofs_engine_error_code=0
  choreofs_path_open_count=3
  choreofs_fd_write_count=13
  choreofs_poll_count=13
  choreofs_last_object=3
  choreofs_led_mask=4
  choreofs_seen_led_mask=7

choreofs-traffic-loop:
  result=0x48494f4b
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0
  choreofs_engine_error_code=0
  choreofs_path_open_count=3
  choreofs_fd_write_count>=13
  choreofs_poll_count>=13
  choreofs_seen_led_mask=7

fail-safe:
  result=0x48494653
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

recovery:
  result=0x48495243
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

many-reentry:
  result=0x4849524d
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

panic-marker:
  result=0x48494641
  panic marker contains file/line/column/message evidence

endpoint-fault:
  result=0x48494641
  panic marker contains EndpointError evidence

endpoint-poison:
  result=0x48494641
  panic marker contains EndpointError evidence
  endpoint error marker prefix=0x57451

preview-probe:
  result=0x48495050
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

deadline-fault:
  result=0x48494641
  panic marker contains DeadlineExceeded evidence

timer-route:
  result=0x48495452
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0

epf-policy-timer:
  result=0x48494550
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  active_epf_target=Policy(57)
  image_ingress=CHOR
  timer_irq_ready=1
  timer_fact_kind=0x57
  timer_fact_arg0=1
  timer_fact_fuel>0
  core0_epf_epoch>0
  core1_epf_epoch>0
  hardfault pc/lr=0

capacity-fault:
  result=0x00000000
  core0_stage=0x48490004
  core1_stage=0x48490004
  epf_load_epoch=1
  epf_kind=TransportFault
  epf_reason=Capacity
  epf_arg0=capacity site
  epf_arg1=lane
  epf_arg2=0x02070003
  epf_fuel_used=7
```

## Build And Run

Build the WASI P1 guests and firmware:

```sh
bash scripts/check_wasip1_guest_builds.sh

cargo build -p baker-firmware \
  --bin baker-choreofs-traffic-loop \
  --target thumbv6m-none-eabi \
  --release \
  --features "wasm-engine-core embed-wasip1-artifacts"
```

`embed-wasip1-artifacts` is local to the `baker-firmware` example package. It
embeds the already-built WASI P1 guest into that physical firmware artifact; it
is not a `hibana-pico` core feature.

Run a hardware pattern:

```sh
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic-loop
```

The script builds, flashes through `probe-rs`, resets the board, polls the
marker symbols, and fails if the physical evidence does not match the selected
pattern.

## GDB Attach

Use GDB after flashing when a marker does not explain the failure:

```sh
probe-rs gdb \
  --chip RP2040 \
  --non-interactive \
  --gdb arm-none-eabi-gdb \
  target/thumbv6m-none-eabi/release/baker-choreofs-traffic-loop \
  -- \
  --batch \
  -ex "symbol-file target/thumbv6m-none-eabi/release/baker-choreofs-traffic-loop" \
  -ex "info registers pc sp xpsr" \
  -ex "bt" \
  -ex "detach" \
  -ex "quit"
```

Avoid reset-halt when preserving marker evidence matters.

## Gates

Run these after changing Baker firmware or the AppKit attach path:

```sh
cargo check --tests
cargo test --test host_capsule_api --features wasm-engine-core -- --nocapture
cargo test --test host_architecture_boundaries -- --nocapture
bash scripts/check_wasip1_guest_builds.sh
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic-loop
bash scripts/run_baker_link_hardware_pattern.sh traffic
bash scripts/run_baker_link_hardware_pattern.sh fail-safe
bash scripts/run_baker_link_hardware_pattern.sh recovery
bash scripts/run_baker_link_hardware_pattern.sh many-reentry
bash scripts/run_baker_link_hardware_pattern.sh panic-marker
bash scripts/run_baker_link_hardware_pattern.sh endpoint-fault
bash scripts/run_baker_link_hardware_pattern.sh endpoint-poison
bash scripts/run_baker_link_hardware_pattern.sh preview-probe
bash scripts/run_baker_link_hardware_pattern.sh deadline-fault
bash scripts/run_baker_link_hardware_pattern.sh timer-route
bash scripts/run_baker_link_hardware_pattern.sh epf-policy-timer
bash scripts/run_baker_link_hardware_pattern.sh capacity-fault
```
