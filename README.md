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
Wasm execution, WASI P1 import payloads, and ChoreoFS driver facts are owned by
the sibling `hibana-wasip1-runtime` crate. `hibana-pico` only attaches those
facts to Hibana Endpoint/carrier progress. There is no Preview 2, WIT, or
Component Model public path.

## Public Surface

The crate exposes only:

```rust
pub mod appkit;
```

There is no root `choreography`, `kernel`, `machine`, `port`, `projects`,
`artifacts`, or `proof` module in the crate shape. Demo build packages live
under `examples/`. The WASI P1 engine implementation is not an appkit module;
it is `hibana-wasip1-runtime`.

`appkit` itself is also a curated facade. Its public path stays flat as
`hibana_pico::appkit::*`; implementation modules under `src/appkit/` remain
private and are re-exported only through the facade.

## Capsule API

Users define a Capsule from raw `hibana::g` choreography:

```rust
pub trait Capsule {
    type Placement;
    type Local;

    fn choreography() -> impl hibana::runtime::program::Projectable;
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
    type Carrier<'a>: hibana::runtime::transport::Transport + 'a
    where
        Self: 'a,
        C: 'a;

    const REQUESTED_ROLES: appkit::RoleSet;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        C: 'a;
}
```

`appkit::run` calls `safe_state` after the requested roles are projected and
attached; callers do not receive mutable access to the logical image.

If `appkit::run` receives an `appkit::WasiImage<'_>`, the image also implements
`appkit::WasiGuestImage<C>` to provide its site-local in-place guest storage.
If `appkit::run` receives `appkit::NoWasi`, the image does not lease guest
storage at all.
With `wasm-engine-core`, `Capsule::WASI_GUEST_DRIVE` defaults to
`appkit::WasiGuestDrive::Canonical`, meaning appkit owns the endpoint/carrier
WASI P1 import loop. A capsule that must step the guest from localside evidence
uses `appkit::WasiGuestDrive::Localside`; this is an explicit ownership choice,
not a hidden rescue path.

`REQUESTED_ROLES` is not authority. `appkit::run` validates it against the
Capsule placement, the linked typed role domain, and the concrete Hibana
`RoleProgram` witnesses materialized for that logical image. Static WASI import
tables are not admission authority and are never used as a pre-choreography
allowlist. `appkit::run` may read an artifact as implementation/load evidence,
but a WASI import becomes meaningful only when the guest actually calls it; that
runtime request must cross the projected Endpoint/carrier frontier, or the
session faults closed.
The endpoint session belongs to the `Capsule` choreography instance, not to the
individual logical image. Peer logical images that project different roles of
the same capsule therefore share the same session unless the capsule explicitly
chooses a different `SESSION_ID: core::num::NonZeroU32` for another choreography
instance.
An empty logical image is not a valid attachment target: `RoleSet::from_bits(0)`
fails at construction time, and `RoleSet::single(role)` is the normal one-role
case.
WASI guests are expected to be ordinary Rust `std` programs when that is the
ergonomic choice. Guest authors do not call Hibana-specific exit helpers. If the
WASI command returns normally, the VM surfaces the same explicit exit event as
status 0. If the guest calls `proc_exit`, the VM surfaces that exit code and
appkit sends the real projected `EngineReq::ProcExit(code)`. A static
`proc_exit` import is load evidence only, never proof that the guest dynamically
called it.
WASI guests do not emit out-of-band loop messages. If a repeated WASI import
stream is legal, the choreography must express that repetition with Hibana
`roll`/reentry over the actual WASI import messages. If the choreography is
straight-line, appkit does not synthesize loop authority; the next real import
either progresses through the frontier or faults closed.
When appkit attaches a logical image to Hibana, it passes only storage and clock
into `hibana::runtime::Config`. Lane domain and endpoint-slot capacity are
derived by Hibana from its typed domain and projected resident descriptors.
Operational deadline fuses belong to the logical image's concrete carrier/site
runtime, not to appkit config or endpoint methods. `hibana-pico` must not
reintroduce caller-chosen lane windows, endpoint-slot knobs, deadline knobs,
hidden pre-attach lowering, or local projection-capacity summaries.

A physical Cargo artifact may contain one or more logical images. This is needed
for targets such as RP2040 dual-core firmware, where one ELF/flash image can
host a Core0 logical image and a Core1 logical image.

Same artifact, firmware, process, or address space never implies direct calls,
authority merge, or syscall shortcuts. The boundary remains Endpoint/carrier.
Each `appkit::run::<LogicalImage, Capsule>()` attaches only that logical image's
validated `REQUESTED_ROLES`; peer core or peer process roles are not attached as
a hidden progress path. Bare-metal scheduler storage is owned by the logical image
storage lease; appkit must not map every nonzero role onto a hidden role1 arena.
On bare metal, one `appkit::run` owns one long-lived role task. Multiple roles in
one firmware are represented as multiple logical images selected by the
site-local entry code, not as a co-located hidden scheduler.
On RP2040, the Core0/Core1 split is connected by an example-defined SIO
transport as a real carrier. That carrier preserves the
logical lane carried by Hibana `Transport::open(PortOpen)`, stores it in SIO
frame metadata, demultiplexes before yielding payload bytes, and returns payload
bytes plus the staged `FrameHeader` inside the same `ReceivedFrame` from
`poll_recv`.
Its `poll_send` and `poll_recv` do not spin inside FIFO push/pop loops. Partial
frames are stored in carrier state across polls; FIFO readiness returns
`Poll::Pending`, not an unbounded in-carrier wait.
Carrier state is owned by the physical endpoint/core that consumes the stream,
not by each logical lane receiver. The rule is ownership first: if physical
ownership can express the state, that is the design. Do not replace ownership
with an atomic mailbox just because the target has atomics. RMW atomics are a
second-line primitive for state that is truly shared concurrently and cannot be
made single-owner without adding more complexity. In that case, if the target
provides read-modify-write atomics, use those atomics because they are the
simplest and fastest ownership primitive for that job. RP2040/thumbv6m SIO does
not provide pointer-width RMW atomics, and it does not need them: the Baker SIO
carrier is core-owned and structured without atomic slot ownership. Appkit's
embedded WASI guest arena uses a single-owner arena lease on every target;
atomics are never a hidden portability requirement for bare-metal images. The
arena is intentionally not `Sync`, and the physical artifact must provide a
separate owner arena for each logical image that can run a WASI guest. A
`NoWasi` logical image must not lease guest storage at all.

Route observation is lane-scoped. A frame label by itself is not route
authority, especially when different arms use the same wire label on different
lanes. Carrier hints may wake or keep an endpoint from parking on the wrong
lane, but they must not mint a continuation; only projected resolver / route /
payload evidence can do that. There is no separate receive-observation hook:
the staged frame header crosses the transport boundary only with the
`ReceivedFrame` that carries the payload bytes, and it must not commit
progress by itself.

## Logical Sites

`appkit` provides one generic logical-site marker, `appkit::Local<Image>`. Carrier
metadata and transport implementations live in examples or user crates,
including in-process carriers. A site may host linked engine capacity and typed boundary handles, but
it must not complete or authorize WASI P1 imports.

Every WASI P1 import emitted by every guest completes only through the projected
choreography to which that guest is attached:

```text
WASI P1 guest
  -> Hibana WASIP1 runtime engine side
  -> typed EngineReq
  -> Endpoint / carrier
  -> Driver side
  -> ledger / ChoreoFS / resolver / boundary facts
  -> typed EngineRet
  -> Endpoint / carrier
  -> Hibana WASIP1 runtime engine side
  -> import completion
```

There is no host filesystem authority, raw socket authority, raw MMIO from a
guest, route inference, timeout rescue, shape heuristic, lane mismatch recovery,
or co-located syscall shortcut.

## Dynamic Routes And Deadlines

Dynamic branch selection belongs to Hibana resolver policy at an explicit
`g::route` point. A passive logical image may be positioned at a route arm
before the controller's route decision or materializing payload has been
observed. That is not progress, and it must not repair missing route state.
`offer()` waits for projected route evidence before producing a continuation.
Offer progress has only evidence-driven outcomes: evidence arrived, still
pending, or terminal fault. There are no defer budgets, no force-poll rescue,
and no liveness heuristic that can mint progress without projected evidence.

Committed Hibana wait semantics are `Progress | Fault`. Rust public APIs expose
committed progress as `Ok(progress) | Err(domain evidence)` through
`EndpointError`, `ResolverError`, or `AttachError`. Committed Fault is terminal
evidence, not a route arm, and there is no wide `HibanaError` for localside.
Hibana also has non-consuming preview/probe points; a preview/probe mismatch is
not protocol progress and cannot select hidden progress.
A preview/probe `Err` is non-progress and cannot select hidden progress.

Operational deadline expiry is different from a protocol timeout. A deadline is
an internal fuse: it poisons the current session generation and returns domain
error evidence. It never selects a route arm. If time should choose a branch,
time must be present in the choreography, for example as a Timer / clock /
interrupt fact consumed by a resolver-selected route arm.
A protocol-visible timeout must be written as choreography: Timer / clock /
interrupt fact plus an explicit resolver-selected route arm.
Protocol-visible timeout uses resolver-selected explicit route arm evidence.

Timeout is not a public API. There is no public timeout API, no public cancel /
reconnect / same-generation recovery API, and no public wide `HibanaError`.
There is no public `EndpointErrorKind` / `ResolverErrorKind` /
`AttachErrorKind` decision surface. Retry after an operational fault is a new
choreography instance / new session generation. Failure never authorizes hidden
progress.

The Baker hardware examples keep both cases separate:

```text
deadline-fault:
  operational deadline -> terminal fault marker

timer-route:
  Timer/clock fact -> resolver-selected route arm over RP2040 SIO
  No shared atomic readiness flag; TimerFiredFact plus projected route evidence
  are observed by the resolver path.

epf-policy-timer:
  role0 delivers Target::Policy bytecode to role1 as a normal SIO choreography message
  timer IRQ fact resolver reads the RP2040 timer IRQ-ready fact and selects the timer-expired arm
  each resolver entry feeds that same timer IRQ-ready fact to EPF as a TapEvent
  loaded EPF policy VM reads the timer TapEvent input and selects the response-ready arm
  both core0 and core1 drain real Hibana TapEvents into EPF observe markers
```

The Baker hardware proof set also checks that failure evidence and preview
evidence stay distinct:

```text
endpoint-fault:
  endpoint error evidence records operation and caller location

endpoint-poison:
  poisoned generation cannot produce a new continuation

preview-probe:
  route-observation hint crosses SIO but remains preview evidence

panic-marker:
  firmware panic handler records file/line/column/message RAM evidence
```

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
cargo test --test host_capsule_api host_capsule_uses_current_hibana_surface
bash ./scripts/check_wasip1_guest_builds.sh
bash ./scripts/check_baker_section_budgets.sh
bash ./scripts/check_plan_pico_gates.sh
```

`check_baker_section_budgets.sh` builds every Baker RP2040 release artifact and
gates `.text`, `.rodata`, `.data`, `.bss`, and flash-size totals with explicit
numeric budgets. Size growth in the proof firmware is treated as a regression,
not as an incidental build artifact.

This workspace depends on the crates.io `hibana` release directly. During
Hibana core development, use a temporary local patch only for pre-release
validation and remove it before committing hibana-pico.

For Baker Link hardware, the runner flashes each physical firmware artifact and
checks RAM markers:

```sh
bash ./scripts/run_baker_link_hardware_pattern.sh timer-route
bash ./scripts/run_baker_link_hardware_pattern.sh epf-policy-timer
bash ./scripts/run_baker_link_hardware_pattern.sh deadline-fault
bash ./scripts/run_baker_link_hardware_pattern.sh preview-probe
bash ./scripts/run_baker_link_hardware_pattern.sh endpoint-fault
bash ./scripts/run_baker_link_hardware_pattern.sh endpoint-poison
bash ./scripts/run_baker_link_hardware_pattern.sh capacity-fault
```

Run every Baker proof after changing appkit attach, carrier, WASI VM driving, or
failure handling:

```sh
for pattern in \
  traffic choreofs-traffic choreofs-traffic-loop \
  fail-safe recovery many-reentry panic-marker \
  endpoint-fault endpoint-poison preview-probe \
  deadline-fault timer-route epf-policy-timer capacity-fault
do
  bash ./scripts/run_baker_link_hardware_pattern.sh "$pattern"
done
```

`capacity-fault` is the loaded EPF observe-bytecode proof for
`TransportFault/Capacity`: the firmware retains the actual Hibana `TapEvent`,
loads observe bytecode through the reserved EPF RAM image area, runs the VM, and
copies the compact `Out` into RAM markers.

`epf-policy-timer` is the loaded EPF policy-bytecode proof. The host commits a
`Target::Policy(57)` image into the BakerLink SWD mailbox. The first Hibana route
resolver reads that mailbox-ready fact and selects the image-load branch, where
role0 delivers the staged image to role1 as an ordinary SIO choreography payload.
The later timer IRQ fact resolver reads the RP2040 timer IRQ-ready fact and
would choose the timer-expired branch after the interrupt fires. The EPF-wrapped
resolver feeds that same timer fact to EPF as Hibana evidence at resolver entry
on each side; the loaded EPF policy VM requires `input[0] == 1` and chooses the
response-ready branch instead. The demo cannot reach its success marker unless
mailbox ingress, resolver-selected image-load choreography, timer-fact ingestion,
and Hibana's typed resolver path all work.
