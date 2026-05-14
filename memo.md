# Cargo-Native One-Raw-Hibana Multi-Engine Capsule Memo

## 結論

この計画を採用する。

`hibana-pico` は OS / host runner / board framework / 独自 build tool / 独自 choreography DSL / domain runtime ではない。

最短定義:

```text
projectable raw hibana in
projection-derived logical images
Cargo-built physical artifacts
every WASI import completion through Endpoint/carrier
```

## 最重要不変式

Every WASI import emitted by every guest is completed only through the projected choreography to which that guest is attached.

すべての WASI import completion は次の経路だけで成立する。

```text
WASI guest
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

禁止:

- Engine side が syscall を直接完了する
- host FS fallback
- raw socket authority
- guest から raw MMIO
- driver-side route inference
- timeout rescue
- shape heuristic
- lane mismatch recovery
- same-firmware direct call
- same-process direct call
- co-located syscall shortcut

同じ firmware / process / address space でも boundary は消えない。

WASI guest memory は linked implementation capacity。
physical artifact は artifact が宣言する memory limit を超えて guest arena を予約しない。
Baker RP2040 proof は 64KiB / 1 page max の WASI P1 artifact を使い、
core1 logical image もその capacity だけを in-place materialize する。

## Public Root

最終 public root は三つだけ。

```rust
pub mod choreography;
pub mod appkit;
pub mod site;
mod kernel;
mod machine;
mod port;
mod projects;
```

削るもの:

- `pub mod proof`
- `appkit::Choreo`
- `appkit::Program`
- `appkit::support`
- `proof::baker_link::support`
- any intermediate escape hatch

## Module Responsibilities

`choreography`:

- protocol vocabulary
- optional raw `hibana::g` helper functions
- no DSL
- no wrapper

`appkit`:

- `Capsule`
- `LogicalImage`
- `Placement`
- `ArtifactBundle`
- `Localside`
- sealed contexts
- `run::<LogicalImage, Capsule>()`

`site`:

- host / linux / mcu / rp2040 / swarm site families
- carrier facts
- substrate facts
- may host engine implementation capacity
- must not complete or authorize WASI imports

`kernel` / `machine` / `port` / `projects`:

- private implementation

## Hibana Completeness

`appkit` must not reject a projectable raw hibana choreography for appkit-local reasons.

許される拒否理由:

- hibana projection failure
- declared site / logical-image capacity mismatch
- placement mismatch
- unsupported linked implementation capacity
- WASI artifact import mismatch against choreography-derived requirements
- target / site incompatibility

禁止:

- appkit-only choreography language
- fragment を使わないと capacity が導けない設計
- `g::par` を落とす
- policy resolver を落とす
- custom payload / control kind を落とす
- binding evidence / transport observation を落とす

## Capsule Shape

Capsule は associated Program を持たない。
ユーザーに `g::steps` / `Program<steps::...>` を書かせず、`choreography()` の戻り値で projection 可能性を要求する。

実際の trait 名は hibana 側 API に合わせる。

```rust
pub trait Capsule {
    type Universe: hibana::substrate::runtime::LabelUniverse;
    type Placement: appkit::Placement<Self>;
    type Local: appkit::Localside<Self>;
    type Report;

    fn choreography() -> impl hibana::substrate::program::Projectable<Self::Universe>;
}
```

注意:

- `Projectable` は appkit 側の逃げ trait にしない
- hibana/projection 側の正式 trait にする
- appkit が独自に blanket impl して wrapper 化しない
- user-facing API に `g::steps` や `Program<steps::...>` 型名を出さない
- `Artifacts` は `Capsule` の associated type にしない
- artifacts は実行時に渡す値であり、Capsule の意味にしない

## LogicalImage

`LogicalImage` は authority ではなく、requested projection slice。

`ROLES` ではなく `REQUESTED_ROLES` と呼ぶ。

```rust
pub trait LogicalImage<C: appkit::Capsule> {
    type Exit<R>;

    const IMAGE_ID: appkit::ImageId;
    const SITE_ID: appkit::SiteId;
    const REQUESTED_ROLES: appkit::RoleSet;
    const CARRIER: appkit::CarrierKind;

    fn init() -> Self;
    fn safe_state(&mut self);
}
```

`REQUESTED_ROLES` は必ず次で検証する。

- `Capsule::Placement`
- hibana projection metadata
- projected `RoleProgram` availability
- site capacity
- linked implementation capacity

## LogicalImage と Physical Cargo Artifact

分けて扱う。

```text
LogicalImage
  = requested projection slice
  = one projected role subset
  = one site-local execution subset
  = one appkit::run::<LogicalImage, Capsule>() target

Physical Cargo Artifact
  = one Cargo build output
  = one ELF / firmware / host binary
  = may contain one or more LogicalImages when the site requires it
```

RP2040 dual-core のように 1 firmware に複数 logical images が入ってよい。

ただし:

- same ELF != direct call
- same ELF != authority merge
- same ELF != syscall shortcut

core / process / address-space が違うなら、必ず別 logical image として扱う。
`appkit::run::<LogicalImage, Capsule>()` が attach するのは、その logical
image の `REQUESTED_ROLES` だけ。peer core / peer process の role を同じ
`run` で後追い attach しない。

RP2040 Baker proof の固定方針:

```text
core0 = kernel-driver logical image = role 0 slice
core1 = WASI P1 engine logical image = role 1 slice
carrier = site::rp2040::SIO
```

同じ physical firmware に両方の logical images が入っていても、core0 と
core1 はそれぞれ自分の projection slice だけを materialize する。進行は
`Endpoint` / `site::rp2040::SIO` を通る typed choreography frame だけで
成立する。

## ArtifactBundle

`ArtifactBundle` は通常の Rust 値。

```rust
pub trait ArtifactBundle<C: appkit::Capsule> {
    fn for_image<I>(&self) -> I::Artifact
    where
        I: appkit::LogicalImage<C>,
        Self: appkit::ArtifactForImage<C, I>;
}

pub trait ArtifactForImage<C: appkit::Capsule, I: appkit::LogicalImage<C>> {
    fn artifact_for_image(&self) -> I::Artifact;
}
```

artifact selection は logical image ごとに型で固定する。
driver image に `NoWasi`、engine image に `WasiImage` を渡すような split を
bundle 側の曖昧な associated type で誤魔化さない。

`run` の引数型は実装時に明確化する。
ここが曖昧だと mode enum が戻る。

## Execution API

public execution path は一本だけ。

```rust
appkit::run::<LogicalImage, Capsule>(
    artifacts.for_image::<LogicalImage>(),
)
```

作らないもの:

- `run_host`
- `run_qemu`
- `run_board`
- `run_embedded`
- `run_agent`
- `run_once`
- demo runner
- direct project runner

## Projection Metadata / Capacity Derivation

ここが blocking item。

capacity / imports / roles / routes は raw hibana choreography または projected `RoleProgram` の metadata から導く。

禁止:

- fragment trait で補う
- helper-name inference
- label 文字列 inference
- appkit DSL

必要な metadata:

- role_set
- lane_set
- label_set
- typed message specs
- control specs
- policy_set
- route shapes
- par ownership
- wasi_imports implied by typed messages
- route witness inputs
- descriptor fingerprints

hibana 側に metadata visitor が足りないなら、足す場所は appkit ではない。
hibana / projection 側に neutral projection metadata visitor を足す。

## ChoreoFS

ChoreoFS は route owner ではない。
path/object fact resolver。

```text
ChoreoFS:
  path -> object facts
ledger:
  fd -> rights + resource + generation + derived route witness
choreography:
  legal order of path_open / fd_read / fd_write / poll / boundary action
```

Only choreography can authorize protocol progress.
All other facts are consumed only at choreography-open phases.

## RouteKey

route coordinate family は一つ。

```rust
pub struct RouteKey<Target> {
    target: Target,
    lane: Lane,
    label: RouteLabel,
    generation: SessionGeneration,
    policy: PolicySlot,
}

pub struct RoleTarget {
    site: SiteId,
    role: RoleId,
}

pub struct NodeTarget {
    node: NodeId,
}
```

`RouteKey` は導出 witness。
app author に通常 path で手書きさせない。

削除:

- `NetworkRoute`
- `RemoteRoute`
- `PicoFdRoute`
- `AgentRoute`
- `WorkerRoute`
- `HostToolRoute`
- `SettlementRoute`
- `ComputerRoute`
- policy-less route shortcut
- `*_Route::new`
- `*_Route::with_policy`

## Proof

`pub mod proof` は置かない。

proof は public API ではなく、public API が十分であることの証明。

配置:

- `examples/`
- `tests/`
- private `projects/`

Baker は実機デモなので、physical firmware package も `examples/baker-firmware`
に置く。これは Cargo の単発 `examples/*.rs` target ではなく、no_std firmware
を build する workspace package。

example が import してよいもの:

```rust
use hibana_pico::{choreography, appkit, site};
```

## Implementation Order

初手はこの順序で固定する。

1. root freeze: `choreography` / `appkit` / `site` only
2. delete `appkit::Choreo` and `appkit::Program`
3. delete `pub mod proof`
4. introduce `Capsule` / `LogicalImage` / `Placement` / `ArtifactBundle` / `run`
5. move Baker to private projects + examples/tests proof
6. move metadata derivation to hibana/projection side
7. Cargo workspace physical artifact packages
8. attached engine invariant
9. same-artifact boundary preservation
10. sealed `Localside` contexts
11. raw hibana helper functions only
12. projection-derived logical images
13. `RouteKey` unification
14. ChoreoFS as fact resolver
15. site families
16. heterogeneous example
17. deletion / hygiene

No custom CLI phase.
No codegen phase.
No macro DSL phase.

## Final Invariants

- There is one projectable raw hibana choreography per Capsule.
- Capsule returns that choreography without user-facing concrete `steps` types.
- Every hibana feature remains usable.
- There may be many WASI engines per Capsule.
- Every WASI engine is attached to the choreography.
- Every import completion crosses endpoint/carrier.
- Every logical image is projection-derived.
- `LogicalImage::REQUESTED_ROLES` is a requested projection slice, not authority.
- `REQUESTED_ROLES` is validated against Placement and projection metadata.
- A physical Cargo artifact may contain one or more logical images.
- Same artifact never means direct call or authority merge.
- Core / process / address-space split is always represented as distinct logical images.
- Each `appkit::run::<LogicalImage, Capsule>()` attaches only that image's requested role slice.
- RP2040 core0/core1 split uses `site::rp2040::SIO` as a real carrier, not appkit in-process queues.
- Every cross-site link carries typed choreography frames only.
- Every external boundary is typed and endpoint-driven.
- Site may host engine capacity but may not complete or authorize WASI imports.
- `RouteKey<Target>` is a derived witness, not app-level authority.
- ChoreoFS resolves facts; it does not own progress authority.
- Ledger materializes fd/lease/session facts.
- Site provides substrate facts only.
- Placement decides location, not legality.
- Localside receives sealed contexts only.
- Capacity derives from hibana/projection metadata, not appkit DSL.
- Metadata derivation is a blocking item.
- Cargo features select implementation capacity, not Capsule meaning.
- Rust/Cargo build physical artifacts; no custom CLI exists.
- Domain semantics live in examples/Capsules, not core.
- No public kernel/machine/port/projects/proof path.
- No compatibility aliases.
- No heuristic recovery.
