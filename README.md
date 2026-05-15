# hibana-pico

`hibana-pico` is a Cargo-native projection, attach, and run integration layer for raw
Hibana choreography.

The shape is:

```text
projectable raw hibana in
projection-derived logical images
Cargo-built physical example packages
every WASI P1 import completion through Endpoint/carrier
```

It is not an OS, host runner, board framework, custom build tool, custom
choreography DSL, domain runtime, or general Wasm runtime.

The only runnable Wasm artifact target is `wasm32-wasip1` / WASI Preview 1.
Core Wasm execution is a private VM implementation detail, not a public module
runner. There is no Preview 2, WIT, or Component Model public path.

## Public Surface

The crate exposes only:

```rust
pub mod choreography;
pub mod appkit;
pub mod site;
```

`kernel` is a private implementation module for the WASI P1 VM and appkit
services. There are no root `machine`, `port`, `projects`, `artifacts`, or
`proof` modules/directories in the crate shape. Demo build packages live under
`examples/`.

## Capsule API

Users define a Capsule from raw `hibana::g` choreography:

```rust
pub trait Capsule {
    type Universe: hibana::integration::runtime::LabelUniverse;
    type Placement;
    type Local;
    type Report;

    fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe>;
}
```

The concrete raw Hibana term is inferred from `hibana::g`; users do not name
the internal step-list type. It is not an `appkit` wrapper or DSL node.

The public execution path is one call:

```rust
appkit::run::<LogicalImage, Capsule>(artifact)
```

The dynamic input is the artifact value, usually a `WasiImage`. Static selection
is carried by the `LogicalImage` and `Capsule` type parameters.

## Logical Images

A logical image is a requested projection slice:

```rust
pub trait LogicalImage<C: appkit::Capsule> {
    type Artifact;
    type Exit<R>;

    const IMAGE_ID: appkit::ImageId;
    const SITE_ID: appkit::SiteId;
    const REQUESTED_ROLES: appkit::RoleSet;
    const CARRIER: appkit::CarrierKind;

    fn init() -> Self;
    fn safe_state(&mut self);
}
```

`REQUESTED_ROLES` is not authority. `appkit::run` validates it against the
Capsule placement and Hibana projection metadata, then validates only the WASI
P1 imports required by that requested role slice.

A physical Cargo artifact may contain one or more logical images. This is needed
for targets such as RP2040 dual-core firmware, where one ELF/flash image can
host a Core0 logical image and a Core1 logical image.

Same artifact, firmware, process, or address space never implies direct calls,
authority merge, or syscall shortcuts. The boundary remains Endpoint/carrier.
Each `appkit::run::<LogicalImage, Capsule>()` attaches only that logical image's
validated `REQUESTED_ROLES`; peer core or peer process roles are not attached as
a hidden fallback. On RP2040, the Core0/Core1 split is connected by
`example-defined rp2040_sio::SIO` as a real carrier.

## Sites

`site` provides one generic logical-site marker, `site::Local<Image>`. Carrier
metadata and transport implementations live in examples or user crates,
including in-process carriers. A site may host linked engine capacity and typed boundary handles, but
it must not complete or authorize WASI P1 imports.

Every WASI P1 import emitted by every guest completes only through the projected
choreography to which that guest is attached:

```text
WASI P1 guest
  -> Engine side
  -> typed EngineReq
  -> Endpoint / carrier
  -> Driver side
  -> ledger / ChoreoFS / resolver / boundary facts
  -> typed EngineRet
  -> Endpoint / carrier
  -> Engine side
  -> import completion
```

There is no host filesystem fallback, raw socket authority, raw MMIO from a
guest, route inference, timeout rescue, shape heuristic, lane mismatch recovery,
or co-located syscall shortcut.

The private VM boundary is `Guest::new(bytes)` plus
`Guest::resume(BudgetRun)`. WASI P1 completion is typed affine
`Pending<K>::complete(...)`; there is no public `Guest::complete`, root
`complete_*`, VM profile selection, or handler-set constructor path.

## Build

Build demo physical packages with Cargo:

```sh
cargo build -p <example-package> --target <target-triple> --release
```

There is no appkit CLI, external projection generator, generated choreography
source, or macro choreography DSL.

Useful gates:

```sh
cargo check --all-targets
cargo test --test host_architecture_boundaries
cargo test --test host_capsule_api
cargo test -p hibana-pico --features wasm-engine-core,wasip1-sys-fd-write --lib drive_wasi_guest_completes_import_only_through_endpoint_carrier
bash ./scripts/check_plan_pico_gates.sh
```
