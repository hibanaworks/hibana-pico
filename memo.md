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
  -> protocol::*ReqMsg
  -> Endpoint / carrier
  -> Driver side
  -> ledger / ChoreoFS / resolver / boundary facts
  -> protocol::*RetMsg
  -> Endpoint / carrier
  -> Engine side
  -> import completion
```

禁止:

- Engine side が syscall を直接完了する
- host FS authority
- raw socket authority
- guest から raw MMIO
- driver-side route inference
- timeout bypass
- shape heuristic
- lane mismatch repair
- same-firmware direct call
- same-process direct call
- co-located syscall shortcut

同じ firmware / process / address space でも boundary は消えない。

WASI guest memory は linked implementation capacity。
physical artifact は artifact が宣言する memory limit を超えて guest arena を予約しない。
Baker RP2040 proof は 64KiB / 1 page max の WASI P1 artifact を使い、
core1 logical image もその capacity だけを in-place materialize する。

## Public Root

最終 public root は一つだけ。

```rust
pub mod appkit;
```

root は appkit 以外の vocabulary を持たない。raw Hibana
programming、WASI P1 runtime、board firmware support はそれぞれの所有層に置く。

## Module Responsibilities

raw Hibana choreography:

- owned by `hibana`
- written directly with `hibana::g`
- no `hibana-pico` adapter vocabulary
- no `hibana-pico` re-exported protocol vocabulary

`appkit`:

- `Capsule`
- `LogicalImage`
- `Placement`
- `Localside`
- sealed contexts
- `run::<LogicalImage>(artifact)`

Concrete `LogicalImage` type:

- logical image selection only
- may host engine implementation capacity
- must not complete or authorize WASI imports

`choreography` / `kernel` / `machine` / `port` / `projects`:

- deleted root paths; do not reintroduce them as public or private side languages

## Hibana Completeness

`appkit` must not reject a projectable raw hibana choreography for appkit-local reasons.

許される拒否理由:

- hibana projection failure
- declared site / logical-image capacity mismatch
- placement mismatch
- unsupported linked implementation capacity
- WASI artifact import mismatch against choreography-derived requirements
- target / site mismatch

禁止:

- appkit-only choreography language
- fragment を使わないと capacity が導けない設計
- `g::par` を落とす
- policy resolver を落とす
- custom payload / resolver evidence を落とす
- binding evidence / transport observation を落とす

## Capsule Shape

Capsule は associated Program を持たない。
ユーザーに `g::steps` / `Program<steps::...>` を書かせず、`choreography()` の戻り値で projection 可能性を要求する。

実際の trait 名は hibana 側 API に合わせる。

```rust
pub trait Capsule: Sized {
    type Placement: appkit::Placement<Self>;
    type Localside: appkit::Localside<Self>;

    fn choreography() -> impl hibana::runtime::program::Projectable;
}

pub trait Placement<C: appkit::Capsule> {
    fn role_kind<const ROLE: u8>() -> appkit::RoleKind;
}
```

注意:

- `Projectable` は appkit 側の逃げ trait にしない
- hibana runtime 側の正式 trait にする
- appkit が独自に blanket impl して汎用 marker 化しない
- user-facing API に `g::steps` や `Program<steps::...>` 型名を出さない
- placement は caller が渡す `u8` ではなく projected const role で決まる
- `Artifacts` は `Capsule` の associated type にしない
- artifacts は実行時に渡す値であり、Capsule の意味にしない

## LogicalImage

`LogicalImage` は authority ではなく、requested projection slice。

`ROLES` ではなく `REQUESTED_ROLES` と呼ぶ。

```rust
pub trait LogicalImage {
    type Capsule: appkit::Capsule;

    type Carrier<'a>: hibana::runtime::transport::Transport + 'a
    where
        Self: 'a,
        Self::Capsule: 'a;

    const REQUESTED_ROLES: appkit::RoleSet;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>
    where
        Self::Capsule: 'a;
}
```

`run` は requested roles の projection / attach が成立した後に `safe_state`
を呼ぶ。caller に logical image の mutable access は渡さない。

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
  = one appkit::run::<LogicalImage>(artifact) target

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
`appkit::run::<LogicalImage>(artifact)` が attach するのは、その logical image
の `REQUESTED_ROLES` だけ。peer core / peer process の role を同じ `run` で
後追い attach しない。

RP2040 Baker proof の固定方針:

```text
core0 = driver logical image = role 0 slice
core1 = WASI P1 engine logical image = role 1 slice
carrier = example-defined RP2040 SIO transport
```

同じ physical firmware に両方の logical images が入っていても、core0 と
core1 はそれぞれ自分の projection slice だけを materialize する。進行は
`Endpoint` / example-defined SIO transport を通る typed choreography frame だけで
成立する。

## Artifact Binding

artifact は logical image の実行入力であり、authority ではない。
Baker/RP2W の固定 2-image firmware では driver image は常に `NoWasi`、
engine image は capsule の `run_engine_image()` が `NoWasi` または `WasiImage` を
`appkit::run` に直接渡す。

artifact selection は `run` の入力値で固定する。
driver image に `NoWasi`、engine image に `WasiImage` を渡すような split を
空 marker や別 bundle trait に逃がさない。

複数の site image を持つ example でも、artifact は image marker の inherent
constructor か `appkit::NoWasi` で直接渡す。別 bundle trait は正規 vocabulary にしない。

`run` は artifact 値を直接受け取る。driver は `NoWasi`、WASI engine は
`WasiImage` を渡し、mode enum や bundle trait は作らない。

## Execution API

public execution path は一本だけ。

```rust
appkit::run::<LogicalImage>(artifact);
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

ここは appkit が消費する検証事実であり、appkit 独自 DSL の入口ではない。

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
- policy_set
- route shapes
- par ownership
- roll / reentry shape
- wasi_imports implied by typed messages
- route witness inputs
- descriptor fingerprints
- tap / evidence specs

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

## Route / Topology Witnesses

Route / topology coordinates are derived witnesses from Hibana projection,
resolver evidence, transport observation, or example-local facts. They are not
an appkit public routing vocabulary.

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

Baker は実機デモなので、physical firmware package も `examples/baker-firmware`
に置く。これは Cargo の単発 `examples/*.rs` target ではなく、no_std firmware
を build する workspace package。

example が import してよい root:

```rust
use hibana_pico::appkit;
```

## Current Settled Shape

- root freeze: `appkit` only
- no `appkit::Choreo`, `appkit::Program`, `pub mod proof`, or appkit side DSL
- `Capsule` / `LogicalImage` / `Placement` / `Localside` / `run`
- metadata derivation stays in hibana/projection
- Cargo workspace physical artifact packages
- attached engine invariant
- same-artifact boundary preservation
- sealed `Localside` contexts
- raw `hibana::g` choreography only
- projection-derived logical images
- ChoreoFS as fact resolver
- logical image type is the site selection marker
- heterogeneous examples remain proofs, not core API

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
- Each `appkit::run::<LogicalImage>(artifact)` attaches only that image's requested role slice.
- RP2040 core0/core1 split uses an example-defined SIO transport as a real carrier, not appkit in-process queues.
- Every cross-site link carries typed choreography frames only.
- Every external boundary is typed and endpoint-driven.
- Site may host engine capacity but may not complete or authorize WASI imports.
- Route / topology evidence is a derived witness, not app-level authority.
- ChoreoFS resolves facts; it does not own progress authority.
- Ledger materializes fd/lease/session facts.
- Concrete logical images provide site facts only.
- Placement decides location, not legality.
- Localside receives sealed contexts only.
- Capacity derives from hibana/projection metadata, not appkit DSL.
- Appkit validates requested roles against projected Hibana role witnesses.
- Cargo features select implementation capacity, not Capsule meaning.
- Rust/Cargo build physical artifacts; no custom CLI exists.
- Domain semantics live in examples/Capsules, not core.
- No public kernel/machine/port/projects/proof path.
- No aliases for deleted public paths.
- No heuristic recovery.
