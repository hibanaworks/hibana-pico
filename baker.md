# Baker Link Hardware Proof Notes

This note records the current Baker Link / RP2040 proof path after the
AppKit Capsule refactor.

## Current Shape

The Baker example package has a common library plus one bin target per hardware
validation pattern. Each selected bin is one physical Cargo artifact, and that
artifact contains two logical images:

```text
examples/baker-firmware
  src/lib.rs: Baker SIO carrier, markers, reset support, logical image helpers
  src/bin/traffic.rs: Capsule + choreography + Localside
  src/bin/choreofs_traffic.rs: Capsule + choreography + Localside + ObjectSpec
  src/bin/choreofs_traffic_loop.rs: Capsule + choreography + Localside + ObjectSpec
  src/bin/fail_safe.rs: Capsule + choreography + Localside
  src/bin/recovery.rs: Capsule + choreography + Localside
  src/bin/many_reentry.rs: Capsule + choreography + Localside
  Core0 logical image: DriverImage
  Core1 logical image: EngineImage
```

Both images are projections of the same raw Hibana choreography. Each
`appkit::run::<LogicalImage, Capsule>()` attaches only that image's requested
role slice:

```text
Core0 DriverImage REQUESTED_ROLES = role 0
Core1 EngineImage REQUESTED_ROLES = role 1
```

The two logical images are connected by the real RP2040 SIO carrier defined by the Baker example as `rp2040_sio::SIO`. Same firmware, same ELF, and same address space do not
mean direct call, authority merge, or syscall shortcut.

The current source map is:

| Layer | File |
| --- | --- |
| AppKit capsule/logical-image substrate | `src/appkit.rs` |
| Generic site marker | `src/site.rs` |
| Baker-local RP2040 SIO carrier | `examples/baker-firmware/src/lib.rs` |
| Baker logical-image/reset support | `examples/baker-firmware/src/lib.rs` |
| Baker validation Capsules, choreography, Localside, ObjectSpec | `examples/baker-firmware/src/bin/*.rs` |
| WASI P1 ChoreoFS traffic guest | `apps/wasip1/wasip1-smoke-apps/src/bin/wasip1-led-choreofs-traffic-cycle.rs` |
| WASI P1 guest build gate | `scripts/check_wasip1_guest_builds.sh` |
| Hardware proof runner | `scripts/run_baker_link_hardware_pattern.sh` |

## What Is Proved

The hardware runner flashes the firmware and reads RAM markers by symbol. A
successful flash alone is not a proof; the RAM markers are the evidence.

The currently supported patterns are:

```sh
bash scripts/run_baker_link_hardware_pattern.sh traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic
bash scripts/run_baker_link_hardware_pattern.sh choreofs-traffic-loop
bash scripts/run_baker_link_hardware_pattern.sh fail-safe
bash scripts/run_baker_link_hardware_pattern.sh recovery
bash scripts/run_baker_link_hardware_pattern.sh many-reentry
```

`traffic`, `fail-safe`, `recovery`, and `many-reentry` are two-role endpoint /
carrier control proofs. They no longer use a role-2 boundary shortcut or a
composite `0b101` attach slice.

`choreofs-traffic` and `choreofs-traffic-loop` are the WASI P1 proofs.
Core1 runs the WASI P1 engine and Core0 runs the driver/kernel side. The guest
itself is an infinite WASI P1 loop:

```text
path_open("device/traffic")
loop {
  fd_write("1"); poll_oneoff(...)
  fd_write("2"); poll_oneoff(...)
  fd_write("4"); poll_oneoff(...)
}
```

The guest imports complete through:

```text
WASI P1 guest
  -> Engine side
  -> typed EngineReq
  -> Endpoint / RP2040 SIO carrier
  -> Driver side
  -> ledger / ChoreoFS / resolver / boundary facts
  -> typed EngineRet
  -> Endpoint / RP2040 SIO carrier
  -> Engine side
  -> import completion
```

There is no host filesystem fallback, route inference, timeout rescue,
lane-recovery loop, or co-located syscall completion.

## ChoreoFS Scope

ChoreoFS is a bounded path/object fact resolver. It is not a host filesystem,
route owner, protocol authority, public Manifest API, POSIX compatibility
layer, or hidden fallback.

For the Baker proof:

```text
ChoreoFS:
  path string -> selector -> object facts

Ledger:
  object facts -> fd materialized view

Choreography:
  RouteDecision / legal order / phase authority
```

The `choreofs-traffic` pattern opens the configured LED object path, mints the
fd through the driver-side materialization path, then performs three
green/orange/red cycles through the projected choreography. That is nine
`fd_write` completions and nine `poll_oneoff` completions. The hardware proof
checks:

```text
choreofs_engine_status = 0x57414f4b
choreofs_path_open_count = 1
choreofs_fd_write_count = 9
choreofs_poll_count = 9
choreofs_last_object = 1
choreofs_led_mask = 4
choreofs_seen_led_mask = 7
```

The `choreofs-traffic-loop` pattern uses the same WASI artifact and
choreography, but leaves the guest and driver in the visual loop. The runner
does not require a fixed final LED mask for that mode; it checks that at least
one full green/orange/red cycle was observed:

```text
choreofs_path_open_count = 1
choreofs_fd_write_count >= 3
choreofs_poll_count >= 3
choreofs_last_object = 1
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

## Known Hardware Evidence

The current proof run passed these patterns on Baker Link hardware:

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
  choreofs_path_open_count=1
  choreofs_fd_write_count=9
  choreofs_poll_count=9
  choreofs_last_object=1
  choreofs_led_mask=4
  choreofs_seen_led_mask=7

choreofs-traffic-loop:
  result=0x48494f4b
  core0_stage=0x4849000a
  core1_stage=0x4849000a
  hardfault pc/lr=0
  choreofs_engine_error_code=0
  choreofs_path_open_count=1
  choreofs_fd_write_count>=3
  choreofs_poll_count>=3
  choreofs_last_object=1
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
```

## Build And Run

Build the WASI P1 guests and firmware:

```sh
bash scripts/check_wasip1_guest_builds.sh

cargo build -p baker-firmware \
  --bin baker-choreofs-traffic-loop \
  --target thumbv6m-none-eabi \
  --release \
  --features "wasm-engine-core wasip1-sys-args-env wasip1-sys-fd-write wasip1-sys-path-open wasip1-sys-poll-oneoff wasip1-sys-proc-exit embed-wasip1-artifacts"
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
```
