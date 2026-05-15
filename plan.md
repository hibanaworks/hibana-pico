最終計画完全版

# Cargo-Native One-Raw-Hibana Multi-Engine Capsule

hibana-pico の完成形はこれです。

```text
hibana-pico
  = projectable な raw hibana choreography を、複数 WASI P1 engine と複数 logical site image に attach し、
    Rust/Cargo の通常 build artifact として出力する projection / attach / run integration layer
```

最短定義はこれです。

```text
projectable raw hibana in
projection-derived logical images
Cargo-built physical artifacts
every WASI P1 import completion through Endpoint/carrier
```

hibana-pico は OS ではない。
host runner ではない。
board framework ではない。
独自 build tool ではない。
独自 choreography DSL ではない。
domain runtime でもない。
general Wasm runtime でもない。

hibana-pico が実行する Wasm artifact は `wasm32-wasip1` / WASI Preview 1 だけです。
core Wasm の parser / interpreter は private VM implementation detail であり、public runnable target ではありません。
Preview 2 / WIT / Component Model / raw core-wasm module runner は public path にしません。

hibana の完全経路はこれです。

```text
hibana::g choreography
  -> project(&program)
  -> SessionKit::enter(...)
  -> Endpoint
  -> flow().send() / recv() / offer() / decode()
```

hibana-pico はこの経路を狭めず、WASI P1 engine / site / carrier / logical image split に接続するだけです。

---

## 1. 最上位不変式

Every WASI P1 import emitted by every guest is completed only through
the projected choreography to which that guest is attached.

すべての WASI P1 import completion はこの形だけで成立します。

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

禁止するもの。

- Engine side が syscall を直接完了する
- host FS fallback
- raw socket authority
- guest から raw MMIO
- driver-side route inference
- Engine side が demo / device / ChoreoFS semantics で route や loop を選ぶ
- Engine side が path_open / fd_write / poll / reentry の example cycle を数える
- Engine side が ChoreoFsObject / ChoreoFS / ledger / device facts を読む
- timeout rescue
- shape heuristic
- lane mismatch recovery
- same-firmware direct call
- same-process direct call
- co-located syscall shortcut

同じ firmware / 同じ process / 同じ address space でも境界は消えません。

- same firmware != direct call
- same firmware != authority merge
- same firmware != syscall shortcut
- same process != direct call
- same process != authority merge
- same process != syscall shortcut

Engine side は WASI P1 VM driver だけです。

Engine side がしてよいこと。

- WASI P1 VM を resume する
- WASI P1 import を typed `EngineReq` に正規化する
- `EngineRet` を Endpoint / carrier 経由で受け取って import を complete する
- 次の WASI import phase に入るための protocol-neutral loop admission を appkit bridge 内で進める

Engine side がしてはいけないこと。

- traffic light / fail-safe / ChoreoFS demo の loop state を持つ
- `path_open` / `fd_write` / `poll_oneoff` / reentry cycle を数える
- `ChoreoFsObject` / ChoreoFS / ledger / device facts を読む
- Baker / example 固有の `LoopContinue` / `LoopBreak` を送る
- device / boundary / driver role を直接呼ぶ
- syscall を fake-success する

Choreography が WASI import loop を持つ場合でも、engine side の continue は
「guest が次の import を発行した」ことを protocol-neutral に admission するだけです。
example 固有の cycle counter や device state で continue / break を決めてはいけません。

---

## 2. Public Root

最終 public root は三つだけです。

```rust
pub mod choreography;
pub mod appkit;
pub mod site;
mod kernel;
```

`proof` は public root に置きません。
proof は tests / examples に置きます。
`machine` / `port` / `projects` の空 placeholder module は置きません。
実装が必要になった時だけ、private implementation として追加します。

各 module の意味は固定します。

```text
choreography
  protocol vocabulary
  optional raw hibana::g helper functions
  no DSL
  no wrapper

appkit
  Capsule
  LogicalImage
  Placement
  ArtifactBundle
  Localside
  sealed contexts
  run::<LogicalImage, Capsule>()
  WASI P1 import normalization

site
  generic site contract only
  one built-in logical image marker: site::Local<Image>
  carrier facts
  site facts
  may host engine implementation capacity
  must not complete or authorize WASI P1 imports

kernel / machine / port / projects
  no public path
  no empty placeholder module
  private implementation only when actually needed
```

削るもの。

- `pub mod proof`
- `appkit::Choreo`
- `appkit::Program`
- `appkit::support`
- `proof::baker_link::support`
- any intermediate escape hatch
- root `artifacts/` directory
- empty private placeholder module

---

## 3. Hibana Completeness Invariant

最重要不変式です。

```text
appkit must not reject a projectable raw hibana choreography
for appkit-local reasons.
```

許される拒否理由はこれだけです。

- hibana projection failure
- declared site / logical-image capacity mismatch
- placement mismatch
- unsupported linked implementation capacity
- WASI P1 artifact import mismatch against choreography-derived requirements
- target / site incompatibility

appkit 独自 DSL を使っていないから拒否する、ということは絶対にしません。

hibana-pico は hibana の能力を通します。

- `g::send`
- `g::seq`
- `g::route`
- `g::par`
- `Program::policy`
- custom `Msg` labels
- custom payload `WireEncode` / `WirePayload`
- control messages
- `GenericCapToken`
- custom `ResourceKind` / `ControlResourceKind`
- resolver policy
- binding evidence
- transport observation
- projection failure checks
- lane ownership checks
- route label checks
- affine `Endpoint` progress

禁止。

- appkit-only choreography language
- fragment を使わないと capacity が導けない設計
- `g::par` を落とす
- policy resolver を落とす
- custom payload を落とす
- custom control kind を落とす
- binding evidence を落とす
- transport observation を落とす

---

## 4. Choreography は Raw Hibana そのもの

choreography = hibana::g

作らないもの。

- `appkit::Choreo`
- `appkit::Program`
- type-level Seq DSL
- `choreo!` macro
- `placement!` macro
- proc_macro choreography
- custom choreography generator

ユーザーは普通に `hibana::g` を書きます。
ユーザーに `g::steps` や `Program<steps::...>` の具体型を書かせません。

```rust
fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe> {
    use hibana::g::{self, Role};
    g::seq(
        g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
        g::seq(
            g::send::<Role<1>, Role<0>, PathOpenMsg, 1>(),
            g::seq(
                g::send::<Role<1>, Role<0>, FdWriteMsg, 2>(),
                g::send::<Role<1>, Role<0>, PollOneoffMsg, 3>(),
            ),
        ),
    )
}
```

`choreography::fragment` は public concept にしません。
helper 名や fragment trait から capacity を推測しません。
必要なら各 Capsule/example が普通の Rust 関数として raw `hibana::g` term を組み立てます。

---

## 5. Capsule

Capsule は associated Program を持ちません。
ユーザーに raw hibana の内部 `steps` 型を書かせると、hibana::g の良さを壊すためです。
projection 可能性は `choreography()` の戻り値境界で要求します。

```rust
pub trait Capsule {
    type Universe: hibana::integration::runtime::LabelUniverse;
    type Placement: appkit::Placement<Self>;
    type Local: appkit::Localside<Self>;
    type Report;

    fn choreography() -> impl hibana::integration::program::Projectable<Self::Universe>;
}
```

`Projectable<Self::Universe>` は、raw hibana global choreography として projection / metadata extraction の対象になれることを表す境界です。

Capsule が持つものはこれだけです。

```text
choreography()
  projectable raw hibana global choreography
Placement
  roles / logical images / sites の割り当て
Local
  endpoint drivers
Report
  terminal result
```

authority は `choreography()` が返す projectable raw hibana choreography だけ。
Placement は location。legality ではありません。

Artifacts は Capsule の associated type にしません。
artifact は実行時に渡す値です。
Capsule の意味にしません。

---

## 6. Hibana 変更ポリシー

hibana を変えてよいのは、hibana 自体に真の価値がある時だけです。
hibana-pico の都合を満たすための逃げ API は足しません。

hibana に足してよい可能性があるもの。

- raw hibana program から projection / neutral metadata extraction ができる official trait
- neutral projection metadata visitor
- role / lane / label / policy / route shape / descriptor fingerprint の中立 metadata

hibana に足してはいけないもの。

- `project_role`
- role-erased projection shortcut
- appkit 専用 trait
- appkit 専用 wrapper
- WASI / board / Capsule / site の知識
- public `g::steps`
- user-facing `Program<steps::...>`
- macro codegen path

`Projectable` は appkit 側の逃げ trait にしません。
hibana/projection 側の正式な中立 trait にします。
appkit が独自に blanket impl して wrapper 化しません。

### Current Typed Role Domain

現行 hibana の typed projection domain は `Role<0>` から `Role<15>` までの 16 roles です。

これは「appkit が勝手に 16 に削った」という意味ではありません。
現行の raw hibana typed API / projection / rendezvous storage がこの domain を前提にしています。

```text
current typed hibana role domain:
  Role<0> ... Role<15>
  16 roles total
```

`RoleSet` などの metadata storage が 16 より広い bitset を持てることと、
`project::<N>()` できる typed role domain は別です。

16 roles を超える必要が出た場合にしてはいけないこと。

- appkit 側だけで role mask を 128 / 256 に増やす
- per-role carrier queue を stack 上で広げる
- hibana の public API に `project_role` などの逃げ API を足す
- app users に public `g::steps` / concrete step type を書かせる

16 roles を超える必要がある場合にやるべきこと。

- hibana 自体の role-domain 設計を見直す
- projection metadata / rendezvous table / role masks / waiter storage を一体で設計する
- large role domain 用の heap/static/materialized storage 方針を決める
- stack 使用量を target ごとに測る

したがって現時点では、`LogicalImage::REQUESTED_ROLES` は current typed hibana role domain 内にあることを gate します。
これは最終理想の任意 role 数対応ではなく、現行 hibana integration surface の正直な implementation capacity です。

---

## 7. ユーザーが定義するもの

通常のユーザーが定義するものはこれです。

```text
Capsule
  - projectable raw hibana choreography
  - associated Placement
  - Localside
  - Report

WASI P1 artifacts
  - one or many wasm32-wasip1 images
```

必要な場合だけ custom site family を定義します。

中心は常に三つです。

- raw hibana choreography
- localside
- WASI P1 artifacts

---

## 8. WASI Engine Count And Swarm Shape

1 Capsule に接続される WASI P1 engine 数は固定しません。

```text
one Capsule
  = one projectable raw hibana choreography

many LogicalImages
  = projection-derived role slices of that one choreography

many WASI P1 engines
  = roles whose Placement role kind is Engine
```

したがって WASI P1 engine 数は `0..N` です。
`appkit` が「1 個」「2 個」と決めません。
`Capsule::Placement::role_kind(role) == RoleKind::Engine` の role が
WASI P1 engine role です。

Swarm も複数 choreography ではありません。
原則は one choreography projected into many logical images です。

```text
same Capsule / same raw hibana choreography

LogicalImage A:
  requested role slice for Linux process

LogicalImage B:
  requested role slice for Cortex-M33 image

LogicalImage C:
  requested role slice for RP2040 core image

LogicalImage D:
  requested role slice for another process/node/core
```

各 logical image は別 physical Cargo artifact でもよく、
同じ physical Cargo artifact 内の別 entrypoint でもよい。
ただし core / process / address-space / node が違えば、実行 image は別 logical image です。

heterogeneous split は通常の Cargo build だけで表します。

```text
cargo build -p linux-control-artifact \
  --target x86_64-unknown-linux-gnu \
  --release

cargo build -p m33-realtime-artifact \
  --target thumbv8m.main-none-eabihf \
  --release

cargo build -p rp2040-io-artifact \
  --target thumbv6m-none-eabi \
  --release
```

それぞれが同じ Capsule を使い、自分の logical image だけを run します。

```rust
appkit::run::<site::Local<image::LinuxControl>, my_capsule::Control>(
    ARTIFACTS.for_image::<image::LinuxControl>(),
);

appkit::run::<site::Local<image::M33Realtime>, my_capsule::Control>(
    ARTIFACTS.for_image::<image::M33Realtime>(),
);

appkit::run::<site::Local<image::Rp2040Io>, my_capsule::Control>(
    ARTIFACTS.for_image::<image::Rp2040Io>(),
);
```

Linux process / Cortex-M33 / RP2040 core の違いは `appkit` core の API 分岐ではありません。
違うのは target triple、linked capacity、site-local storage facts、carrier implementation だけです。

すべての WASI P1 import completion は同じ invariant に従います。

```text
WASI P1 engine role
  -> typed EngineReq
  -> Endpoint/carrier
  -> driver/boundary role
  -> typed EngineRet
  -> Endpoint/carrier
  -> import completion
```

禁止。

- swarm を複数 choreography の寄せ集めにする
- Linux process だけ direct host runner にする
- Cortex-M33 / RP2040 だけ別 appkit API にする
- same process / same firmware だから direct call にする
- peer logical image の role slice を同じ `run` で attach する

---

## 9. Execution API

public execution path は一本です。

```rust
appkit::run::<LogicalImage, Capsule>(
    artifacts.for_image::<LogicalImage>(),
)
```

artifacts は `ArtifactBundle` を実装する通常の Rust 値です。

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

`ArtifactBundle` は Capsule 全体の意味ではなく、実行時に渡す artifact value です。
artifact selection は logical image ごとに型で決まり、driver image に `NoWasi`、
engine image に `WasiImage` を渡す形を型で分けます。
全 image に同じ artifact 型を返す escape hatch は作りません。

ここでいう artifact は WASI P1 byte artifact / runtime input です。
root `artifacts/` directory や core-owned demo package taxonomy ではありません。
demo / smoke / hardware proof packages は `examples/` に置きます。

作らないもの。

- `run_host`
- `run_qemu`
- `run_board`
- `run_embedded`
- `run_agent`
- `run_once`
- demo runner
- direct project runner

例。

```rust
appkit::run::<
    site::Local<image::Composite>,
    host_smoke::Wasip1Smoke,
>(
    artifacts.for_image::<image::Composite>(),
);

appkit::run::<
    site::Local<image::Engine>,
    baker_link_traffic::Traffic,
>(
    artifacts.for_image::<image::Engine>(),
);
```

同じ Capsule。
違う LogicalImage。

---

## 10. LogicalImage と Physical Cargo Artifact

ここは必ず分けます。

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

`LogicalImage::REQUESTED_ROLES` は authority ではありません。
名前も `ROLES` ではなく `REQUESTED_ROLES` にします。

```text
LogicalImage::REQUESTED_ROLES is a requested projection slice.
It must be validated against Capsule::Placement and hibana projection metadata.
It is not protocol authority.
```

WASI P1 engine を持つ logical image は、WASI guest storage を自分の実行 site の storage fact として渡します。
appkit が host / embedded / RP2040 などを feature で見て初期化方式を選びません。

```rust
pub trait LogicalImage<C: appkit::Capsule> {
    const REQUESTED_ROLES: appkit::RoleSet;

    #[cfg(feature = "wasm-engine-core")]
    fn wasi_guest_storage<'guest, const ROLE: u8>() -> appkit::WasiGuestStorage<'guest>;
}
```

`WasiGuestStorage` は in-place arena lease です。
WASI guest はこの lease からだけ `Guest::init_in_place(...)` されます。

WASI guest memory は linked implementation capacity です。
physical artifact は artifact が宣言する memory limit を超えて guest arena を予約しません。
RP2040 Baker proof は 64KiB / 1 page max の WASI P1 artifact を使い、
core1 logical image の in-place arena もその capacity だけを materialize します。

許さないもの。

- stack 上に巨大 `Guest` を置く
- `Box<Guest>` / heap guest allocation
- host だけ `Guest::new(bytes)` に戻す
- site / feature ごとに別の guest construction path を持つ
- appkit が `platform-*` feature で guest 初期化方式を分岐する
- physical artifact が artifact capacity を超える guest memory を隠れて予約する

site / logical image が違ってよいのは storage の所在だけです。
初期化方式は常に in-place です。

Localside future / scheduler storage も同じです。

- host/linux だけ `Box<dyn Future>` に逃がさない
- host/linux だけ `Vec` scheduler に逃がさない
- attach slab を heap allocation にしない
- appkit scheduler storage は bounded in-place slot だけを使う
- std/no_std の差分で protocol path / storage policy を分岐しない

host / linux site は richer integration layer を持ってよい。
ただし appkit の projection / attach / scheduler / guest initialization path は
RP2040 と同じ bounded in-place 方針から外れない。

RP2040 dual-core のように、物理的には 1 firmware / 1 reset vector / 1 flash image になりがちな環境でも自然に扱えます。

同じ physical artifact に複数 logical images が入っても境界は消えません。

- same ELF != direct call
- same ELF != authority merge
- same ELF != syscall shortcut

RP2040 dual-core なら、1 firmware の中に Core0 logical image と Core1 logical image を入れてよい。

```rust
fn reset_entry() -> ! {
    match rp2040_sio::core_id() {
        0 => appkit::run::<
            site::Local<image::Driver>,
            baker_link_traffic::Traffic,
        >(ARTIFACTS.for_image::<image::Driver>()),
        _ => appkit::run::<
            site::Local<image::Engine>,
            baker_link_traffic::Traffic,
        >(ARTIFACTS.for_image::<image::Engine>()),
    }
}
```

これは 1 physical Cargo artifact ですが、2 logical images です。
境界は endpoint/carrier に残ります。

core / process / address-space が違うなら、必ず別 logical image です。
`appkit::run::<LogicalImage, Capsule>()` はその logical image が宣言した
`REQUESTED_ROLES` だけを attach します。
別 core / 別 process の peer role を同じ `run` で後追い attach しません。

```text
core1 = WASI P1 engine logical image
core0 = kernel-driver logical image
```

この split は、本物の endpoint/carrier 境界で接続されます。
RP2040 では `example-defined rp2040_sio::SIO` が materialized carrier です。
同じ physical firmware に core0/core1 の logical images が入っていても、
`core0` が role 0 slice を attach し、`core1` が role 1 slice を attach します。
片方の `run` が相手 core の role を attach してはなりません。

---

## 11. Cargo-Native Build

独自 CLI はありません。

削除。

- appkit build
- xtask requirement
- custom generator
- external manifest generator
- proc_macro choreography
- out-of-band projection tool

build は Cargo だけです。

```text
cargo build -p <physical-example-package> --target <target-triple> --release
```

各 physical artifact は普通の Cargo binary package です。

```text
workspace/
  capsules/baker-link-traffic/       # one raw hibana choreography
  examples/baker-firmware/           # Baker demo physical RP2040 firmware, multiple logical images
  examples/host-smoke-example/       # host smoke example package
  examples/swarm-smoke-example/      # swarm smoke example package
```

physical artifact package は、必要なら検証や配置ごとに複数の bin target
を持ってよいです。package-local `lib.rs` は reset / carrier / markers /
small execution helpers だけを共有します。各 bin は、その検証で使う
Capsule / raw hibana choreography / Localside / ChoreoFsObject を直接持ち、
source を開けば検証内容が一目で読める形にします。共通 Capsule を
`lib.rs` に隠しません。

```rust
fn main() -> ! {
    appkit::run::<LogicalImage, Capsule>(
        artifacts.for_image::<LogicalImage>(),
    )
}
```

複数 architecture は Cargo target triple で分けます。

```text
cargo build -p baker-firmware \
  --bin baker-traffic \
  --target thumbv6m-none-eabi \
  --release
cargo build -p linux-main \
  --target x86_64-unknown-linux-gnu \
  --release
cargo build -p mcu-main \
  --target thumbv7em-none-eabihf \
  --release
```

同じ choreography。
別 logical image。
必要なら同じ physical artifact。
別 target triple。
Cargo だけ。

最小化は Rust/Cargo の能力で行います。

- separate physical artifact packages when useful
- single package with multiple logical images when site requires it
- target-specific dependencies
- capacity-only features
- generic type selection
- linker dead-code elimination
- no generated source
- no custom CLI

---

## 12. LogicalImage

```rust
pub trait LogicalImage<C: appkit::Capsule> {
    type Exit<R>;
    type Carrier<'a>: hibana::integration::Transport + 'a
    where
        Self: 'a;

    const IMAGE_ID: appkit::ImageId;
    const SITE_ID: appkit::SiteId;
    const REQUESTED_ROLES: appkit::RoleSet;
    const CARRIER: appkit::CarrierKind;

    fn init() -> Self;
    fn safe_state(&mut self);
    fn carrier<'a>() -> Self::Carrier<'a>;
}
```

`appkit::CarrierKind` は opaque な attach metadata です。
`appkit` 本体に `Rp2040Sio` / `Tcp` / `Udp` / `Uart` のような site 固有 enum variant を置きません。
site / board 固有の carrier 名は user crate / example / optional site crate 側の定数として出します。

`LogicalImage::Carrier` は actual carrier implementation です。
`CarrierKind` は manifest / attach metadata であり、transport implementation ではありません。
carrier implementation は user crate / example / optional site crate が提供します。
appkit core は in-process queue / `RefCell` / carrier frame buffer を持ちません。
in-process かどうかも core `site` の名前ではなく、その logical image が選ぶ carrier implementation と metadata で表します。

例:

- test/example-local carrier kind
- example-local `rp2040_sio::SIO`
- user-defined `TcpCarrierKind`
- user-defined `UdpCarrierKind`
- user-defined `UartCarrierKind`

core `site` は分類 taxonomy を持ちません。
提供する logical image helper は一つだけです。

- `site::Local<Image>`

process / bare / no_std / std / board / node / core id の区別を core `site` の型階層にしません。
それらの意味は `site::Local` の型名ではなく、
`LogicalImage` 実装の `SITE_ID` / `REQUESTED_ROLES` / `Carrier` / artifact-local site facts が持ちます。
別の表現が必要なら user crate が自分の marker type を定義して `LogicalImage` を実装します。

LogicalImage は authority を持ちません。
projection と placement を materialize するだけです。

LogicalImage は実行元 site の role slice だけを materialize します。
core / process / address-space が違えば、実行イメージも違います。
同じ physical Cargo artifact に複数 logical images が含まれていても、
各 `appkit::run::<LogicalImage, Capsule>()` が attach するのは
その logical image の `REQUESTED_ROLES` だけです。

`REQUESTED_ROLES` は必ず次で検証されます。

- `Capsule::Placement`
- hibana projection metadata
- projected `RoleProgram` availability
- site capacity
- linked implementation capacity

---

## 13. Sealed Localside Contexts

Localside に raw site / raw device / raw host authority を渡しません。

```rust
pub trait Localside<C: appkit::Capsule> {
    type Error;

    fn engine<const ROLE: u8>(
        ctx: appkit::EngineCtx<'_, C, ROLE>,
    ) -> impl Future<Output = Result<Infallible, Self::Error>>;

    fn driver<const ROLE: u8>(
        ctx: appkit::DriverCtx<'_, C, ROLE>,
    ) -> impl Future<Output = Result<Infallible, Self::Error>>;

    fn boundary<const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'_, C, ROLE>,
    ) -> impl Future<Output = Result<Infallible, Self::Error>>;

    fn link<const ROLE: u8>(
        ctx: appkit::LinkCtx<'_, C, ROLE>,
    ) -> impl Future<Output = Result<Infallible, Self::Error>>;

    fn supervisor<const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'_, C, ROLE>,
    ) -> impl Future<Output = Result<Infallible, Self::Error>>;
}
```

domain taxonomy を増やしません。
boundary は GPIO でも timer でも host service でも browser でも model でも sensor でも actuator でもよい。
core はその意味を知りません。

contexts は role-typed です。
`EngineCtx<'_, C>` のような role-erased context では、hibana の `Endpoint<'_, ROLE>` が持つ `flow()` / `recv()` / `offer()` / `decode()` の型安全性を残せません。
user に `g::steps` を書かせず、かつ raw hibana endpoint progress を保つには、context 側だけが `const ROLE` を持つ必要があります。

各 context の authority は最小です。

```text
EngineCtx:
  guest
  engine endpoint
  WASI P1 import dispatch / normalization
  protocol-neutral WASI import loop admission inside appkit bridge only
  no ChoreoFS
  no ChoreoFsObject / ledger / device facts
  no raw boundary
  no host FS
  no direct syscall completion
  no example-specific loop / cycle / route control

DriverCtx:
  driver endpoint
  ledger
  ChoreoFS facts
  resolver
  no raw boundary handle

BoundaryCtx:
  boundary endpoint
  typed site-local boundary handle
  no EngineReq business matching

LinkCtx:
  carrier only
  no app semantics

SupervisorCtx:
  lifecycle
  safe-state
  image attestation
```

Localside failure is propagated with `?`.
The top-level logical image runner is the only place allowed to unwrap / panic.
Project code must be able to write the normal hibana shape directly:

```rust
let flow = endpoint.flow::<Msg>()?;
flow.send(&msg).await?;

let reply = endpoint.recv::<ReplyMsg>().await?;
```

That `recv` shape is only for the next deterministic message in the projected
role program. If the choreography point is a route choice, the hibana shape is
`offer` followed by branch-local `decode`:

```rust
let branch = endpoint.offer().await?;
let decoded = branch.decode::<Msg>().await?;
```

Do not require Baker-specific wrappers, appkit wrappers, `map_err` at every call
site, or stage-code helper functions around `flow` / `send` / `recv` / `offer`
/ `decode`.

To preserve origin evidence while still using plain `?`, hibana must expose a
domain-specific error family with caller location captured by the public
fallible API itself.

```rust
pub struct EndpointError {
    pub fn operation(&self) -> &'static str;
    pub fn file(&self) -> &'static str;
    pub fn line(&self) -> u32;
    pub fn column(&self) -> u32;
}

pub struct ResolverError { /* same public shape, resolver domain only */ }
pub struct AttachError { /* same public shape, attach domain only */ }
```

This is intentionally not one wide `HibanaError`.
Endpoint progress, resolver policy, and attach/runtime assembly are different
domains. A single enum would only reintroduce a domain switch inside the error.
The shared abstraction is only the caller-location rule. `ErrorLocation`,
operation enums, and error-kind enums do not need to become public concepts;
`EndpointError` / `ResolverError` / `AttachError` may expose only small getters
and `Debug`. Endpoint error-kind accessors must not become a public decision
surface. Appkit may attempt an optional loop-control `flow`; if the endpoint
does not admit that control, appkit must not infer why from error internals and
must let the immediately following real WASI import be validated by the
projected endpoint frontier.

For async endpoint operations, the callsite must be captured when constructing
the future, not after the future is polled. If necessary, hibana should expose
`fn send(...) -> impl Future<Output = Result<_, EndpointError>>`-style entry
points internally rather than losing caller location inside `async fn`.

App / firmware crates may define their own aggregate error type with `From`
impls:

```rust
enum AppError {
    Endpoint(hibana::EndpointError),
    Resolver(hibana::ResolverError),
    Attach(hibana::AttachError),
}
```

but that aggregation belongs to the app. hibana's public error surface remains
domain-specific.

appkit must not add a public failure-hook trait just to support one firmware
marker strategy. `Localside::Error` only needs `Debug`; firmware-specific panic
marker recording belongs in the firmware panic handler or in the firmware's
own error representation, not in appkit's public contract.

### Failure / Deadline / Cancellation Constitution

Committed Hibana wait semantics are `Progress | Fault`.
Rust public APIs expose committed progress as `Ok(progress) | Err(domain evidence)`.
Committed Fault is terminal evidence, not a route arm.
Timeout is not a public API.
Deadline is an operational fuse.
A protocol-visible timeout must be written as choreography: Timer / clock /
interrupt fact at an explicit route point, with a resolver choosing the route
arm. It is not the same proof as operational deadline failure.

Hibana wait sites have only two semantic outcomes:

```text
Progress
Fault
```

The Rust surface is:

```text
Ok(progress)
Err(domain evidence)
```

`Err` is not a branch. For committed progress, `Err` is terminal evidence for
the current session generation. Failure never authorizes alternate progress.

Hibana also has non-consuming preview/probe points. `flow` previews a send,
`offer` previews a branch, and `decode` may validate the selected branch before
committing endpoint progress. A preview/probe mismatch is not protocol progress
and must not become route authority. It may leave the endpoint on the same
frontier so the same projected branch can be decoded correctly, but it must not
select a fallback arm, infer a retry policy, or hide transport failure.

```text
Failure never authorizes alternate progress.
Failure never authorizes hidden progress.
Timeout is not a public API.
Deadline is an operational fuse.
Committed Fault poisons the current session generation.
Retry requires a new session generation.
`?` is the normal failure propagation path.
```

Time may choose a typed branch only when time appears in the choreography. A
protocol-visible timeout is therefore an ordinary projected route, for example
a Timer / clock / interrupt fact consumed by a resolver at an explicit route
point. The resolver may choose the `Expired` arm only at that route point.
Operational deadline expiry is different: it is a committed wait-site fault,
poisons the current generation, and wakes the affected waiters with domain
evidence. It must not select a route arm.

Public endpoint progress remains only:

```text
flow
send
recv
offer
decode
```

Do not add:

```text
recv_timeout
send_timeout
offer_timeout
decode_timeout
cancel
try_recover
reconnect
ignore_fault
same-generation retry
```

Public Hibana exposes only these error evidence envelopes:

```text
EndpointError
ResolverError
AttachError
```

There is no wide `HibanaError` for localside.

`EndpointError` / `ResolverError` / `AttachError` are evidence envelopes, not
recovery handles. Error kind may exist internally for `Debug`, diagnostics,
fault sinks, or tests, but public `EndpointErrorKind` / `ResolverErrorKind` /
`AttachErrorKind` must not become user decision surfaces. User code must not be
expected to match `DeadlineExceeded` or `TransportClosed` and continue the same
generation.

The public API boundary determines the error domain:

```text
endpoint.flow/send/recv/offer/decode -> EndpointError
resolver decision / policy registration -> ResolverError
rendezvous / role enter -> AttachError
```

Lower-layer causes are mapped into the boundary's domain evidence. For example,
if `offer()` fails because a resolver or transport invariant failed inside the
endpoint, the public result is still `EndpointError`.

User computation errors are not `EndpointError`. A role future may return its
own app error. If that error escapes while the session is still live, the
runner / role guard must terminally fault the current generation; the user error
itself remains a user error.

Retry after an operational fault is a new choreography instance / new session generation.

Drop is a backstop, not the authority. It may poison a live generation when an
endpoint is abandoned, but hard reset, panic-abort, hard fault, disabled
interrupts, or process kill require the runner / supervisor / integration owner /
watchdog to provide the terminal fault boundary.

---

## 14. Site の責務

site image が engine capacity を含むことはあります。
それ自体は禁止しません。
ただしその engine capacity は WASI P1 artifact を動かすための implementation capacity であり、site authority ではありません。

禁止するのはこれです。

```text
site must not complete or authorize WASI P1 imports.
```

WASI P1 import dispatch は import を typed request に正規化するだけです。

```text
WASI P1 import dispatch
  -> typed EngineReq
  -> Endpoint/carrier
```

site がしてよいこと。

- provide carrier facts
- provide CPU/core/process facts
- provide physical/electrical facts
- provide typed boundary handles
- provide memory/link site facts
- host one or more logical images
- host linked engine implementation capacity
These facts live in the `LogicalImage` implementation, user/example site-local
types, or private appkit attachment state. They are not root core site-family
modules.

site がしてはいけないこと。

- complete WASI P1 imports
- authorize WASI P1 imports
- perform EngineReq business matching
- own GuestLedger internals
- own protocol route authority
- infer protocol
- repair mismatch

---

## 15. Placement

Placement は Capsule の associated type。
外部引数にしません。

```rust
type Placement: appkit::Placement<Self>;
```

別 placement が欲しいなら別 Capsule type です。

```text
placement decides location, not legality.
```

macro は使わない。
const table / type で書きます。

Placement は `LogicalImage::REQUESTED_ROLES` を検証するための静的事実です。
Placement 自体も protocol authority ではありません。

---

## 16. WASI Semantics

WASI P1 guest は authority ではありません。
import source でしかありません。

hibana-pico が実行する Wasm は WASI Preview 1 のみです。

- accepted artifact target は `wasm32-wasip1`
- WASI Preview 2 は public path にしない
- WIT / Component Model は public path にしない
- raw core-wasm module runner は public path にしない
- unsupported import は validation / attach / typed reject で止める
- unsupported syscall fake-success はしない

WASI P1 import support は「std / POSIX 互換にあるから残す」では決めません。
hibana-pico core に高い価値があるものだけを active capacity として残します。
価値が薄いもの、POSIX 互換の名残、protocol authority を濁すもの、
network / signal / scheduler などを host OS 風に見せるものは削除または unsupported にします。

判断基準。

- choreography 境界をより明確にするか
- WASI P1 guest の現実的な実行に必要か
- typed `EngineReq` / `EngineRet` として Endpoint / carrier を通せるか
- ChoreoFS / ledger / resolver / boundary facts の責務を濁さないか
- fake-success / fallback / POSIX compatibility layer を要求しないか

この基準を満たさない WASI P1 import は、import 名を診断用に認識しても
active capacity では validation reject します。

Active WASI P1 support は閉じた allowlist です。
core に残す import はこれだけです。

- `fd_write`
- `fd_read`
- `fd_close`
- `fd_fdstat_get`
- `fd_readdir`
- `path_open`
- `poll_oneoff`
- `clock_time_get`
- `clock_res_get`
- `random_get`
- `args_get` / `args_sizes_get`
- `environ_get` / `environ_sizes_get`
- `proc_exit`

この list にない WASI P1 import は、明示的に計画へ昇格しない限り unsupported です。
`fd_readdir` は host filesystem enumeration ではありません。
ChoreoFS の bounded directory object facts を choreography-open phase で読むためだけに残します。
LLM / agent / management guest が利用可能な object を調べてから `path_open` する用途はあります。
その場合でも `fd_readdir` が返すのは Capsule が明示した bounded ChoreoFS catalog facts だけです。
path / object kind / rights / generation などの facts は返してよいが、route decision / protocol authority /
host filesystem access は返しません。列挙で見えた object も、`path_open` / `fd_read` / `fd_write` /
`poll_oneoff` が raw hibana choreography で開いていなければ消費できません。
network / swarm / remote object を列挙する場合も、`fd_readdir` が返すのは catalog facts だけです。
node が接続断になっても ChoreoFS entry が即削除されるとは限りません。
reachable か、どの peer/session generation か、stale fd かどうかは link/carrier facts と ledger
generation validation が決めます。ChoreoFS catalog entry の存在は liveness authority ではありません。

WASI は engine route ではありません。
WASI は import namespace です。

```text
Wasm instruction execution
  -> WASI P1 import boundary
  -> typed EngineReq
  -> Endpoint / carrier
```

`TinyWasm` / `CoreWasm` / `CoreWasip1` のような engine route を public concept にしません。
capacity 差分は Cargo feature / linked implementation capacity / choreography-derived requirements で決まり、ユーザーが VM profile を選ぶ設計にはしません。

```text
path_open      -> typed PathOpen request
fd_read        -> typed object read
fd_write       -> typed object write / message emission
fd_readdir     -> bounded directory object read
poll_oneoff    -> resolver wait
proc_exit      -> terminal event
clock_time     -> clock fact
random_get     -> bounded entropy fact
fd_close       -> fd generation update
memory.grow    -> MemFence(MemoryGrow)
path mutation  -> typed reject
fd tuning      -> typed reject
fd table mutate -> typed reject
unsupported    -> typed reject
```

禁止。

- Preview 2 public path
- WIT / Component Model public path
- host filesystem fallback
- raw socket authority
- raw host boundary access
- unsupported syscall fake-success

WASI P1 artifact build も Cargo でよいです。

```text
cargo build -p my-wasi-app --target wasm32-wasip1 --release
```

third-party artifact は `include_bytes!` / file embedding / site-specific artifact provider で渡します。
ただし import completion は必ず choreography を通ります。

### WASI P1 VM Boundary

旧 WASI VM 計画の思想はこの境界として残します。
VM は public runtime ではなく、private kernel implementation capacity です。

外側が覚える engine 境界はこれだけです。

```rust
pub(crate) struct Guest<'a> {
    /* private */
}

impl<'a> Guest<'a> {
    pub(crate) unsafe fn init_in_place(dst: *mut Self, bytes: &'a [u8]) -> Result<(), Error>;
    pub(crate) fn resume(&mut self, budget: BudgetRun) -> Result<Event<'_, 'a>, Error>;
}
```

`Guest::resume(BudgetRun)` だけが次を管理します。

- instruction 実行
- fuel / budget 消費
- host import 到達
- pending host call
- done / exit / trap

`Guest::new(bytes)` は最終 runtime construction path にしません。
WASI guest は `LogicalImage` / site-local storage facts が渡す in-place arena lease にだけ構築します。
これは RP2040 の stack 制約だけの特殊対応ではなく、全 site で同じ制約です。

completion は `Guest` の method ではありません。
`resume` が返す affine pending token の method に閉じます。

```rust
impl Pending<'_, '_, FdWrite> {
    pub(crate) fn fd(&self) -> Fd;
    pub(crate) fn bytes(&self) -> &[u8];
    pub(crate) fn complete(self, written: u32, errno: Errno) -> Result<(), Error>;
}
```

この形なら、`FdRead` 待ちに `FdWrite` completion を返すことは型で表現できません。

appkit-internal canonical WASI engine driver はこの private VM と appkit の唯一の bridge です。
この bridge は WASI P1 VM execution / import normalization / typed completion だけを扱います。
ChoreoFS, ChoreoFsObject, ledger, device, demo loop, traffic pattern, fail-safe scenario selection は持ちません。

```text
Guest::resume(BudgetRun)
  -> Call::FdWrite / Call::FdRead / ...
  -> typed EngineReq
  -> Endpoint / carrier
  -> typed EngineRet
  -> Pending<K>::complete(...)
  -> Guest::resume(BudgetRun)
```

禁止。

- `Guest::complete`
- `Guest::new(bytes)` as runtime construction path
- root public `complete_*`
- `resume_with_fuel`
- `resume_with_budget`
- `run_until_*`
- trap API が `EngineReq` を直接返す設計
- public VM profile argument
- public handler set constructor argument
- legacy poll / socket / route shape fallback
- example-specific loop / route / cycle control in engine side
- ChoreoFS / ChoreoFsObject / ledger / device facts in engine side
- unsupported import absorption
- runtime completion kind matching fallback

保持する guarantee。

- import signature validation
- memory bounds check
- memory grow fence
- fd / iovec / path / env decode safety
- one pending at a time
- completion kind correctness
- no stale guest memory view after `memory.grow`
- WASI errno lowering in one place

### WASI VM Machine Shape

`kernel::engine::wasm` は private façade、実装は private machine です。
外から見える engine route を増やしません。

```text
src/kernel/engine/
  mod.rs
  wasm/
    mod.rs       // private crate façade
    machine.rs // private VM implementation
```

`tiny`, `core`, `wasip1` は public-facing route 名として残しません。
内部分割が必要な場合でも、外側から見える概念は増やしません。

`Call` は WASI P1 import lowering で host completion が必要なものだけです。
timer / GPIO / device / service semantics は VM call ではありません。
それらは raw hibana choreography と driver / boundary localside の意味です。

Allowed VM calls:

- `FdWrite`
- `FdRead`
- `FdReaddir`
- `FdFdstatGet`
- `FdClose`
- `ClockResGet`
- `ClockTimeGet`
- `PollOneoff`
- `RandomGet`
- `PathOpen`
- `Args*` / `Environ*` when linked capacity enables them

Not VM calls:

- `ProcExit` - this is `Event::Exit`
- `ProcRaise` - this is POSIX self-signal compatibility and is not supported by hibana-pico core
- `SchedYield` - this is POSIX scheduler-hint compatibility and is not supported by hibana-pico core
- `Sock*` - this is POSIX socket compatibility and is not supported by hibana-pico core
- `PathLink` / `PathSymlink` / `PathRename` / `PathUnlinkFile` / `PathCreateDirectory` / `PathRemoveDirectory`
- `FdAdvise` / `FdAllocate` / `FdDatasync` / `FdSync`
- `FdFdstatSet*` / `FdFilestatSet*`
- `FdRenumber`
- `Done` - this is `Event::Done`
- `BudgetExpired` - this is `Event::BudgetExpired`
- `SleepUntil`
- `GpioSet`
- traffic light / Baker / demo actions
- boundary device operations

`proc_raise` is deliberately unsupported for now.
The VM may recognize `proc_raise` as a WASI P1 import name, but active capacity
must reject it during import validation. It must not become a runtime `Call`,
must not be fake-successed, and must not be mapped into hibana abort / fence /
control choreography. WASI `proc_raise` is local POSIX self-signal compatibility;
hibana abort / fence / topology control stays explicit raw choreography.

`sched_yield` is deliberately unsupported for now.
The VM may recognize `sched_yield` as a WASI P1 import name, but active capacity
must reject it during import validation. It must not become a runtime `Call`,
must not be fake-successed, and must not be mapped into hibana resolver / route
frontier / policy-yield internals. WASI `sched_yield` is a local POSIX scheduler
hint; hibana resolver semantics stay explicit raw choreography and policy
configuration. `EngineReq::Yield` / `EngineRet::Yielded` must not be used as a
WASI `sched_yield` compatibility path; if they have no separate choreography use,
they are deletion candidates.

`sock_*` is deliberately unsupported in hibana-pico core.
The VM may recognize WASI P1 socket import names for diagnostics, but active
capacity must reject them during import validation. It must not become
`Call::Socket`, must not expose raw socket authority, and must not fake POSIX
network compatibility. Network returns through hibana-pico as explicit
choreography: ChoreoFS may expose bounded network / listener object facts when a
Capsule configures them, the ledger materializes object fd / rights / generation /
derived route witness, and `fd_read` / `fd_write` / `poll_oneoff` progress only
when raw hibana choreography admits that phase. POSIX `sock_*` is not the
authority model.

Host filesystem mutation is deliberately unsupported in hibana-pico core.
`path_link`, `path_symlink`, `path_rename`, `path_unlink_file`,
`path_create_directory`, and `path_remove_directory` may be recognized as WASI P1
import names for diagnostics, but active capacity must reject them during import
validation. ChoreoFS is a bounded path/object fact resolver, not a host
filesystem or POSIX namespace mutation layer. Object creation, topology changes,
and management operations must be explicit raw hibana choreography and
Capsule-local boundary behavior, not WASI path mutation.

POSIX file tuning / sync / metadata mutation is deliberately unsupported in
hibana-pico core. `fd_advise`, `fd_allocate`, `fd_datasync`, `fd_sync`,
`fd_fdstat_set_flags`, `fd_fdstat_set_rights`, `fd_filestat_set_size`, and
`fd_filestat_set_times` may be recognized for diagnostics, but active capacity
must reject them. They do not clarify choreography authority; they imply a POSIX
file abstraction that hibana-pico does not provide.

`fd_renumber` is deliberately unsupported in hibana-pico core.
The guest does not own the authoritative fd table. The ledger materializes fd /
rights / resource identity / generation / route witness from choreography-open
facts. Guest-driven fd table rewiring would move authority into the guest and is
therefore rejected.

### Decoded IR Requirement

Decoded IR is mandatory.
The VM must not repeatedly decode raw opcodes or scan block structure in the hot resume path.

Parse / validation must produce decoded instruction state:

```rust
enum Instr {
    Nop,
    Drop,
    LocalGet(LocalId),
    LocalSet(LocalId),
    GlobalGet(GlobalId),
    GlobalSet(GlobalId),
    I32Const(u32),
    I64Const(u64),
    Load(MemLoad),
    Store(MemStore),
    I32(I32Op),
    I64(I64Op),
    F32(F32Op),
    F64(F64Op),
    Br(BranchTarget),
    BrIf(BranchTarget),
    BrTable(BrTableId),
    Call(FuncId),
    CallIndirect(TypeId),
    Return,
    End,
}
```

Required precomputation:

- import intern and signature validation
- function/local layout decode
- branch target precompute
- br_table predecode
- block result shape validation
- call / call_indirect type preconditions
- memory/table/global bounds preconditions where statically available

Forbidden in the hot path:

- raw opcode decode per step
- `find_matching_end`
- `find_matching_else`
- structural block scan
- raw import name byte comparison
- allocation in instruction step
- allocation in memory load/store

The hot path advances decoded instructions only:

```text
Guest::resume(BudgetRun)
  -> Machine::step(decoded Instr)
  -> StepEffect::{Continue, Host, Done, Exit}
```

### WASI Import Authority

WASI import names have one authority: `Wasip1ImportName`.
Parse time interns imports into typed import slots.
Runtime execution must not compare raw import-name bytes.

```text
module/name bytes
  -> Wasip1ImportName
  -> ImportSlot
  -> decoded call target
  -> Call::*
```

Duplicate `WASIP1_IMPORT_*` byte constants are deletion targets unless they are
private generated aliases of `Wasip1ImportName` with no independent authority.

### VM Hot Path

Memory and numeric operations stay direct and allocation-free.

- active memory length is cached
- load/store uses direct slice checks
- `memory.grow` alone updates active length
- WASI iovec/path/subscription decode is separate from Wasm load/store primitive
- numeric ops use small inline primitives matching Wasm wrapping/trap semantics

### Fixture Quarantine

Production VM files must not contain demo fixture byte arrays.
Fixture modules belong in `#[cfg(test)]` fixture modules or external test assets.

Deletion targets:

- `DEMO_*` guest byte arrays
- traffic light byte fixtures
- bad route demo byte fixtures
- sleep/timer demo byte fixtures
- budget accounting by `BudgetRun`

---

## 17. ChoreoFS

ChoreoFS is a bounded path/object fact resolver.
ChoreoFS は削除対象ではありません。public Manifest API として外へ出さず、権威を剥がして `DriverCtx` / appkit internal / Capsule-local facts として残す対象です。

It is not:

- host filesystem
- route owner
- protocol authority
- public Manifest API
- POSIX compatibility layer
- hidden fallback

ChoreoFS provides:

- path -> object facts
- preopen/object namespace facts
- bounded static/config/log/directory object facts
- network/listener/remote/management object facts when configured by the Capsule

ChoreoFS does not provide liveness authority.
For network / swarm / remote objects, a catalog entry means the object is known
or configured, not that the peer is currently reachable.

Link / carrier facts provide:

- reachability
- peer/session generation
- last observed epoch
- disconnect / reconnect observation
- carrier-local readiness facts

Ledger materializes:

- fd
- rights
- resource identity
- object generation
- derived route witness
- route witness generation

Choreography owns:

- RouteDecision
- LoopContinue / LoopBreak
- StateSnapshot / StateRestore
- TopologyBegin / TopologyAck / TopologyCommit
- CapDelegate
- AbortBegin / AbortAck
- Fence
- TxCommit / TxAbort
- legal order of path_open / fd_read / fd_write / poll / boundary action
- phase authority

ChoreoFS chain はこうです。

```text
ChoreoFS:
  path string -> selector -> object facts

Link / carrier facts:
  reachability -> peer/session generation -> observed epoch

Ledger:
  object facts + link generation facts -> fd materialized view

Choreography:
  builtin ControlOp transitions / legal order / phase authority
```

RouteDecision を ChoreoFS chain の中に見せすぎない。
RouteDecision は choreography の側に戻す。
Topology / abort / fence / transaction / loop control も ChoreoFS や carrier の責務ではありません。
それらは hibana builtin control messages として raw choreography の側にあります。

残るべき ChoreoFS の責務:

- preopen root facts
- path selector
- bounded object table
- object id / object generation
- object rights facts
- object kind facts
- bounded read/write/append/directory facts
- host filesystem fallback rejection
- path_open admit/reject material
- fd minting input facts

ChoreoFS に残さない責務:

- route selection
- protocol phase legality
- fd authority by itself
- device authority
- network authority
- liveness authority
- transport authority
- scheduler policy
- retry/fallback policy
- host filesystem authority

重要な点:

「public Manifest を消す」と「ChoreoFS を消す」は違います。

```text
public Manifest を消す:
  yes

ChoreoFS facts を消す:
  no
```

Manifest を public concept にすると、ユーザーが choreography とは別に authority table を持つように見えます。
だから public API としては出さない。

でも `DriverCtx` / appkit internal / Capsule-local facts として、ChoreoFS object store は残す。
そして、それは choreography-open phase でしか消費できない。

正確な invariant:

```text
Only choreography can authorize protocol progress.
ChoreoFS facts are consumed only at choreography-open phases.

Network disconnect / reconnect handling:

- ChoreoFS catalog entries may remain after disconnect.
- disconnect / reconnect observations update link/carrier reachability facts and observed peer/session generation.
- link/carrier observations never update topology, route witnesses, or ledger generations by themselves.
- link/carrier observations can only be consumed as inputs to choreography-open builtin control transitions.
- topology / recovery / route witness transition is represented by hibana builtin control messages:
  `RouteDecision`, `TopologyBegin`, `TopologyAck`, `TopologyCommit`, `AbortBegin`,
  `AbortAck`, `Fence`, `StateSnapshot`, `StateRestore`, `TxCommit`, `TxAbort`,
  `LoopContinue`, and `LoopBreak`.
- `CapDelegate` is not an app-authored `g::send` control message. It is the
  lower-layer endpoint token delegation path. Examples must not fake
  activation or re-entry by wrapping `CapDelegate` in ordinary choreography
  messages.
- `StateSnapshot` / `StateRestore` are state-generation controls for the role
  endpoint that owns the state. They are not a cross-site transfer shortcut and
  must not be used as "snapshot on one core, restore on another" proof.
- ledger route witness generation changes only after the projected choreography admits the corresponding control transition.
- ledger fd views hold the object generation and derived route witness generation observed at mint time.
- `fd_read` / `fd_write` / `poll_oneoff` must validate the current generation/reachability before progress.
- stale or unreachable routes are reported through explicit typed result / choreography branch.
- no hidden retry, no automatic route repair, no stale fd rebinding to a new peer.
```

最終形:

- ChoreoFS は bounded path/object fact resolver。
- ChoreoFS は protocol authority ではない。
- ChoreoFS は route owner ではない。
- ChoreoFS は public Manifest API ではない。
- ChoreoFS facts は `DriverCtx` が choreography-open phase でだけ消費する。
- fd / rights / generation / route witness は ledger が materialize する。
- legal order は raw hibana choreography が決める。

---

## 18. RouteKey

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

ただし app author に通常 path で RouteKey を手書きさせません。

RouteKey は導出 witness。

```text
raw hibana choreography
projection
placement
role grouping
label/lane
generation
policy slot
link / peer generation facts
```

削除。

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

policy-less shortcut なし。
必要なら `PolicySlot::ZERO` を明示。

---

## 19. Projection Metadata / Capacity Derivation

ここは blocking item です。
この計画の成否はここで決まります。

fragment trait で補いません。
helper-name inference で補いません。
label 文字列 inference で補いません。
appkit DSL で補いません。

capacity / imports / roles / routes は raw hibana choreography または projected `RoleProgram` の metadata から導きます。

必要な metadata。

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

もし hibana 側に十分な metadata visitor がないなら、足す場所は appkit ではありません。
hibana / projection 側です。
ただし hibana を変えるのは真に中立な projection value がある場合だけです。

足すとしても、WASI / board / Capsule / site の知識は足しません。
中立な projection metadata visitor だけです。

appkit は official projection metadata を読むだけです。
label 文字列や helper fragment 名から意味を再発明しません。

capacity は例としてこう導きます。

```text
Msg<LABEL_WASI_FD_WRITE, EngineReq>      -> fd_write
Msg<LABEL_WASI_PATH_OPEN, EngineReq>     -> path_open
Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>   -> poll_oneoff
Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleep> -> timer-like boundary
Msg<LABEL_GPIO_SET, GpioSet>             -> gpio-like boundary
control Msg<K>                           -> control capacity
policy::<ID>()                           -> resolver capacity
g::par                                   -> parallel capacity
```

capacity は helper 関数名ではなく、raw `hibana::g` term / projected RoleProgram metadata からだけ導きます。

---

## 20. Label Universe

固定 universe を強制しません。

```rust
type Universe: hibana::integration::runtime::LabelUniverse;
```

built-in vocabulary は予約領域を持つ。
user labels は Capsule universe が持つ。
collision は Cargo test gate で拒否します。

---

## 21. External and Temporal Semantics

hibana-pico core は domain semantics を持ちません。

- no realtime semantics
- no LLM semantics
- no agent semantics
- no browser semantics
- no settlement semantics
- no service taxonomy
- no timeout policy
- no hidden fallback

任意の external boundary は、普通の projected role と sealed BoundaryCtx です。

```text
A Capsule may attach any projected role to a site-local external boundary.
That boundary is driven only through Endpoint and sealed localside context.
```

timeouts, slow engines, model calls, browser actions, settlement actions, audit sinks, sensors, actuators は appkit concept ではありません。
必要なら examples / Capsule 内の raw hibana labels, payloads, routes, policy points, localside behavior として表現します。

---

## 22. Carrier

carrier は authority ではありません。

Carrier can:

- move typed frames
- preserve ordering / bounds
- report readiness

Carrier cannot:

- infer protocol
- repair mismatch
- choose route
- synthesize approval
- complete syscall

SIO FIFO, mailbox, RTOS queue, UART, USB, TCP, UDP mesh, in-process queue は全部 carrier。

これらの名前は `site` の site facts であり、`appkit` の public enum variant ではありません。
`appkit` は carrier identity の照合と manifest 化だけを行い、site 固有 carrier の意味を知りません。

RP2040 Baker example では example-local `rp2040_sio::SIO` が core0/core1 logical images を接続する
materialized carrier です。
`appkit` は SIO FIFO register / RP2040 core id / RP2040 carrier semantics を知りません。
SIO carrier は Baker example の local module が `hibana::integration::Transport` として提供します。
この carrier は protocol authority ではなく、typed frame を運ぶだけです。

---

## 23. ImageManifest

各 logical image は manifest を持ってよい。
これは build / attach metadata であり、authority ではありません。

```text
capsule_hash
choreography_hash
placement_hash
logical_image_id
site_id
requested_role_set
projected_role_set
label_set
wasi_imports
object_caps
derived_route_keys
memory_budget
carrier_kind
peer_images
```

attach 時に一致確認。

- capsule_hash
- choreography_hash
- placement_hash
- label universe hash
- carrier shape
- peer image ids

不一致なら attach しません。
吸収 loop は作りません。

manifest は Rust/Cargo 内で扱う。
外部 generator は不要です。

---

## 24. Proof は Examples / Tests

`pub mod proof` はありません。

配置。

- examples
- tests

example はこれだけ import する。

```rust
use hibana_pico::{choreography, appkit, site};
```

proof は public API ではなく、public API が十分であることの証明です。
examples に入れるべき domain semantics を core 設計に入れません。

---

## 25. Example Packages

example package は demo / hardware proof の binary glue。
root `src/projects` module は置きません。

```rust
fn main() -> ! {
    appkit::run::<
        site::Local<image::Engine>,
        baker_link_traffic::Traffic,
    >(
        ARTIFACTS.for_image::<image::Engine>(),
    )
}
```

core API ではなく、`appkit::run::<LogicalImage, Capsule>(artifact)` を使う完成例として置きます。

---

## 26. Cargo Features

Cargo feature は implementation capacity だけ。

許す。

- wasm-engine-core
- wasip1-sys-args-env
- wasip1-sys-fd-write
- wasip1-sys-fd-read
- wasip1-sys-fd-readdir
- wasip1-sys-fd-fdstat-get
- wasip1-sys-fd-close
- wasip1-sys-clock-res-get
- wasip1-sys-clock-time-get
- wasip1-sys-path-open
- wasip1-sys-poll-oneoff
- wasip1-sys-random-get
- wasip1-sys-proc-exit

Board / carrier / demo names are not hibana-pico core features.
If an example needs local conditional compilation, it defines that feature in
the example crate without forwarding it to core.

禁止。

- platform-host-native
- platform-cortex-m
- platform-linux
- demo selection feature
- testcase feature
- agent mode feature
- baker bad-path feature
- approved-checkout feature
- wasip1-sys-sock
- wasip1-sys-proc-raise
- wasip1-sys-sched-yield
- wasip1-sys-path-link
- wasip1-sys-path-symlink
- wasip1-sys-path-rename
- wasip1-sys-path-unlink-file
- wasip1-sys-path-create-directory
- wasip1-sys-path-remove-directory
- wasip1-sys-fd-advise
- wasip1-sys-fd-allocate
- wasip1-sys-fd-datasync
- wasip1-sys-fd-sync
- wasip1-sys-fd-fdstat-set-flags
- wasip1-sys-fd-fdstat-set-rights
- wasip1-sys-fd-filestat-set-size
- wasip1-sys-fd-filestat-set-times
- wasip1-sys-fd-renumber
- unsafe-safe feature

Capsule meaning は Cargo feature に置きません。

---

## 27. Deletion List

削除。

- `pub mod kernel`
- `pub mod machine`
- `pub mod port`
- `pub mod projects`
- `pub mod proof`
- appkit build CLI
- xtask requirement
- custom generator
- external manifest generator
- proc_macro choreography
- public `HostRunner`
- public `GuestLedger` internals
- public resolver internals
- public transport assembly
- public machine pin constants outside site family
- public general Wasm runtime
- public VM profile / handler-set selection
- public `TinyWasm` / `CoreWasm` / `CoreWasip1` route concepts
- `wasm-engine-tiny` as public profile concept
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
- `RemoteControl::cap_grant_remote`
- `RemoteObjectTable::apply_cap_grant_with_policy`
- direct host demo execution path
- direct transaction runner from host
- runtime route-depth hints
- unsupported syscall fake-success
- unsupported import absorption
- host filesystem fallback
- protocol inference
- lane mismatch recovery loop
- `Guest::complete`
- root public `complete_*`
- `Call::ProcRaise`
- `Pending<ProcRaise>`
- `wasip1-sys-proc-raise`
- `Call::SchedYield`
- `Pending<SchedYield>`
- `wasip1-sys-sched-yield`
- `EngineReq::Yield` / `EngineRet::Yielded` when only retained for WASI `sched_yield`
- `Call::Socket`
- `Pending<Socket>`
- `wasip1-sys-sock`
- POSIX socket compatibility path in core
- `Call::PathLink` / `Pending<PathLink>`
- `Call::PathSymlink` / `Pending<PathSymlink>`
- `Call::PathRename` / `Pending<PathRename>`
- `Call::PathUnlinkFile` / `Pending<PathUnlinkFile>`
- `Call::PathCreateDirectory` / `Pending<PathCreateDirectory>`
- `Call::PathRemoveDirectory` / `Pending<PathRemoveDirectory>`
- `wasip1-sys-path-link`
- `wasip1-sys-path-symlink`
- `wasip1-sys-path-rename`
- `wasip1-sys-path-unlink-file`
- `wasip1-sys-path-create-directory`
- `wasip1-sys-path-remove-directory`
- host filesystem mutation path in core
- `Call::FdAdvise` / `Pending<FdAdvise>`
- `Call::FdAllocate` / `Pending<FdAllocate>`
- `Call::FdDatasync` / `Pending<FdDatasync>`
- `Call::FdSync` / `Pending<FdSync>`
- `Call::FdFdstatSetFlags` / `Pending<FdFdstatSetFlags>`
- `Call::FdFdstatSetRights` / `Pending<FdFdstatSetRights>`
- `Call::FdFilestatSetSize` / `Pending<FdFilestatSetSize>`
- `Call::FdFilestatSetTimes` / `Pending<FdFilestatSetTimes>`
- `Call::FdRenumber` / `Pending<FdRenumber>`
- `wasip1-sys-fd-advise`
- `wasip1-sys-fd-allocate`
- `wasip1-sys-fd-datasync`
- `wasip1-sys-fd-sync`
- `wasip1-sys-fd-fdstat-set-flags`
- `wasip1-sys-fd-fdstat-set-rights`
- `wasip1-sys-fd-filestat-set-size`
- `wasip1-sys-fd-filestat-set-times`
- `wasip1-sys-fd-renumber`
- POSIX file tuning / sync / metadata mutation path in core
- guest-driven fd table mutation path in core
- `resume_with_fuel`
- `resume_with_budget`
- `run_until_*`
- trap API returning `EngineReq`
- `appkit::Choreo`
- `appkit::Program`
- `appkit::support`
- type-level choreography DSL
- macro choreography DSL
- `project_role`
- public `g::steps`
- user-facing `Program<steps::...>`

互換 layer なし。
alias なし。
legacy public path なし。

---

## 28. Gates

### Public Root Gate

`lib.rs` must contain:

```rust
pub mod choreography;
pub mod appkit;
pub mod site;
mod kernel;
```

must not contain:

```rust
mod machine;
mod port;
mod projects;
pub mod kernel;
pub mod machine;
pub mod port;
pub mod projects;
pub mod proof;
```

### Capsule Shape Gate

Capsule must return a projectable raw hibana choreography without exposing concrete `steps` types.

```rust
pub trait Capsule {
    type Universe;
    type Placement;
    type Local;
    type Report;

    fn choreography() -> impl Projectable<Self::Universe>;
}
```

Forbidden:

- `fn choreography() -> appkit::Choreo`
- `fn choreography() -> appkit::Program`
- `fn choreography() -> macro-generated DSL node`
- `type Program` as user-facing concrete choreography
- user-facing `g::steps` / `Program<steps::...>` concrete type names

### Hibana API Gate

hibana changes must be neutral and valuable to hibana itself.

Forbidden:

- `project_role`
- role-erased projection shortcut for appkit
- appkit-specific trait in hibana
- public `g::steps`
- asking app users to name `Program<steps::...>`
- macro-generated choreography escape hatch

### LogicalImage Requested-Role Gate

LogicalImage uses requested projection slice naming.

```rust
const REQUESTED_ROLES: RoleSet;
```

Forbidden:

- `const ROLES` as authority
- using `REQUESTED_ROLES` without validating against Placement
- using `REQUESTED_ROLES` without validating against projection metadata
- using `REQUESTED_ROLES` outside the current typed hibana role domain

### LogicalImage Attach-Slice Gate

`appkit::run::<LogicalImage, Capsule>()` must attach only the requested role slice for that
logical image.

Forbidden:

- attaching peer core roles in the same `run`
- attaching peer process roles in the same `run`
- using a composite role attach path as a hidden fallback for split-site images
- special-casing `REQUESTED_ROLES == 0b11` as engine+driver co-location
- driving core1 WASI engine and core0 kernel-driver by direct call

Required:

- core/process/address-space split is represented as distinct logical images
- each logical image validates `REQUESTED_ROLES` against Placement and projection metadata
- each logical image attaches only its validated requested roles
- cross-image progress crosses Endpoint/carrier

### RP2040 SIO Carrier Gate

RP2040 core split uses example-local `rp2040_sio::SIO` as a real site carrier.

Forbidden:

- RP2040 SIO vocabulary inside `appkit`
- appkit-provided in-process carrier queues or RefCell-backed carrier storage
- bypassing SIO with same-firmware direct calls
- image-id-specific session ids that prevent peer logical images from joining the same session

Required:

- example-local `rp2040_sio::SIO` is implemented as `hibana::integration::Transport`
- core0 and core1 logical images attach their own role slices independently
- both peer images for one Capsule/site use a shared site-local session identity
- SIO only moves typed choreography frames
- SIO does not authorize imports, select routes, or repair protocol mismatch

### Cargo-Only Gate

禁止。

- appkit build
- custom CLI
- xtask required for normal image build
- generated choreography source
- external projection generator
- proc_macro choreography

許可。

- `cargo build -p example-package --target target-triple --release`
- `cargo test`
- `cargo doc`
- `cargo metadata`
- target-specific dependencies
- capacity-only features
- `build.rs` only for Cargo-native link/env metadata

### Hibana Completeness Gate

External capsule must use all of these successfully:

- `g::send`
- `g::seq`
- `g::route`
- `g::par`
- `policy::<ID>()`
- custom payload
- custom control kind
- resolver
- binding evidence
- transport observation

If any raw hibana feature cannot be used in a Capsule, appkit is incomplete.

### Metadata Derivation Gate

appkit must derive capacity from official hibana/projection metadata.

Forbidden:

- capacity inferred from appkit helper names
- capacity inferred from label strings
- capacity inferred only from fragment trait
- capacity requiring appkit DSL

If metadata is missing, add neutral value to hibana projection metadata only when it benefits hibana itself.
Do not add appkit syntax or appkit-only hibana APIs.

### Attached Engine Gate

Every import must be observed as:

```text
EngineReq through endpoint/carrier
EngineRet through endpoint/carrier
```

The appkit bridge may normalize WASI P1 imports into typed `EngineReq`.
It must complete the VM pending token only after receiving the matching `EngineRet` through Endpoint / carrier.

### Canonical WASI Engine Gate

For a WASI P1 engine role, the engine side is not example-defined localside code.
If a logical image supplies a `WasiImage`, `appkit::run` must select the canonical
appkit WASI P1 VM driver for that engine role before calling `Localside::engine`.

Allowed:

- appkit-owned `Guest::init_in_place(ptr, bytes)` / `Guest::resume(BudgetRun)`
- appkit-owned WASI P1 import dispatch / normalization into typed `EngineReq`
- appkit-owned completion of VM pending tokens after matching `EngineRet`
- logical-image supplied in-place guest storage
- logical-image supplied execution budget facts
- site/example debug markers outside protocol progress

Forbidden:

- public or example-side `EngineCtx::drive_wasi_guest(...)`
- public or example-side WASI import drive loop
- calling `Localside::engine` for a role whose logical image artifact is `WasiImage`
- example / demo cycle counters in engine localside
- `path_open` / `fd_write` / `poll_oneoff` / reentry counting in engine localside
- `ChoreoFsObject` / ChoreoFS / ledger / device facts in engine localside
- LED / GPIO / timer / boundary facts in engine localside
- Baker-specific or example-specific loop control messages in engine localside
- route branch selection from device or demo semantics in engine localside
- direct calls to driver / boundary / site device code from engine localside
- import completion without matching `EngineRet` through Endpoint / carrier

`Localside::engine` remains for non-WASI engine roles only. A NoWasi engine role
may be a pure choreography endpoint, but it is not a WASI P1 VM runner.

If an example needs repeated visible behavior, the WASI P1 guest must produce repeated imports.
The choreography may admit those imports with a loop / route / reentry structure, and the driver /
boundary side may validate object and device facts at open phases.
The engine side must not synthesize the repetition. The only loop on the WASI side is the
appkit canonical resume loop that keeps executing the guest until it exits, traps, or parks
after terminal completion.

Canonical WASI loop control is not a heuristic. If a `WasiImage` capsule has no
projected `WasiImportLoopContinue` / `WasiImportLoopBreak` control labels,
appkit must fail closed. If those controls are projected, appkit may send
`LoopContinue` only when the current endpoint frontier admits that control. If
the optional loop-control `flow` is not admitted, appkit must not inspect
endpoint error internals to recover. It must fall through to the real WASI
import, and that import must then succeed or fail through the projected
endpoint frontier. This preserves straight-line phases such as ChoreoFS
`path_open` and mid-iteration phases such as `fd_write -> poll_oneoff`, while
keeping repeated visible behavior under explicit route / loop / reentry
choreography.

### WASI P1 VM Boundary Gate

hibana-pico executes only WASI Preview 1 artifacts.

Allowed:

- `wasm32-wasip1` artifacts
- private VM implementation for WASI P1 execution
- `Guest::init_in_place(ptr, bytes)`
- `Guest::resume(BudgetRun)`
- typed affine `Pending<K>::complete(...)`

Forbidden:

- Preview 2 public path
- WIT / Component Model public path
- raw core-wasm runnable public path
- public general Wasm runtime
- public VM profile selection
- `Guest::new(bytes)` as appkit runtime construction path
- `Box<Guest>` / heap guest allocation
- stack-allocated `Guest`
- `Guest::complete`
- root public `complete_*`
- multiple resume entrypoints
- `Call::ProcRaise` / `Pending<ProcRaise>` as active runtime surface
- `Call::SchedYield` / `Pending<SchedYield>` as active runtime surface
- `Call::Socket` / `Pending<Socket>` as active runtime surface
- trap API returning `EngineReq`
- `proc_raise` completion, fake-success, or implicit abort/fence/control mapping
- `sched_yield` completion, fake-success, or implicit resolver/policy-yield mapping
- `sock_*` completion, fake-success, raw socket authority, or POSIX network compatibility mapping
- host filesystem mutation through `path_link` / `path_symlink` / `path_rename` / `path_unlink_file` / `path_create_directory` / `path_remove_directory`
- POSIX file tuning / sync / metadata mutation through `fd_advise` / `fd_allocate` / `fd_datasync` / `fd_sync` / `fd_fdstat_set_*` / `fd_filestat_set_*`
- guest-driven fd table mutation through `fd_renumber`
- unsupported import absorption
- unsupported syscall fake-success
- stale memory views after `memory.grow`

### Same-Artifact Boundary Gate

A physical Cargo artifact containing multiple logical images must still use endpoint/carrier boundaries.

These:

- same ELF
- same firmware
- same process
- same address space

must not imply:

- direct call
- authority merge
- syscall shortcut

### Site Gate

site must not complete or authorize WASI P1 imports.

Allowed:

- linked engine implementation capacity
- carrier facts
- typed boundary handles
- site-local facts

Forbidden:

- WASI P1 import completion
- WASI P1 import authorization
- site-specific carrier enum variants in appkit
- EngineReq business matching
- GuestLedger internals
- protocol authority

### AppKit Gate

appkit must not contain:

- board-specific pin constants
- Baker-specific names
- domain-specific vocabulary
- protocol inference
- route mismatch recovery
- timeout heuristic
- raw host FS fallback
- direct syscall completion

### Localside Context Gate

Localside must not receive:

- raw Site
- raw Machine
- raw Transport internals
- raw host filesystem
- raw socket authority
- raw MMIO outside typed BoundaryCtx

### Hibana Error Evidence Gate

Project localside code must propagate hibana failures with plain `?`.

Required hibana error domains:

- `EndpointError` for `flow` / `send` / `recv` / `offer` / `decode`
- `ResolverError` for resolver decisions and policy registration
- `AttachError` for rendezvous / role enter

Each error must carry:

- domain-specific operation identity
- error kind for `Debug` / diagnostics
- caller location captured at the public fallible API boundary

Forbidden:

- one wide `HibanaError` as the primary public error type
- public `EndpointErrorKind` / `ResolverErrorKind` / `AttachErrorKind` as user decision surfaces
- Baker-specific wrappers around `flow` / `send` / `recv` / `offer` / `decode`
- appkit wrappers around hibana endpoint progress only to record callsite
- public appkit failure-hook traits for firmware-specific RAM markers
- repeated `map_err` / stage-code plumbing at project call sites
- losing async callsite location by capturing location only after polling
- public timeout APIs such as `recv_timeout` / `send_timeout` / `offer_timeout`
- public cancel / reconnect / same-generation recovery APIs

The top-level logical image entry may unwrap / panic after recording the
propagated error evidence. Intermediate localside code must return `Result` and
use `?`.

Firmware panic evidence must have its own verification artifact. Do not prove
panic marker behavior by corrupting a normal choreography/WASI demo. A dedicated
panic-marker binary may intentionally panic; the firmware panic handler must
record file hash, line, column, message hash, message length, and message bytes
into RAM markers, and the hardware script must verify those markers as an
expected failure result with no hardfault marker.

### Failure / Deadline / Cancellation Gate

Required:

- wait semantics are `Progress | Fault`
- public Rust result is `Ok(progress) | Err(domain evidence)`
- committed-progress `Err` is terminal evidence, not a protocol branch
- preview/probe `Err` is non-progress and cannot select hidden progress
- operational deadline expiry poisons the current session generation
- retry starts a new session generation
- operational timeout/fuse proof is the `deadline-fault` kind of proof
- protocol-visible timeout is a separate Timer / clock / interrupt route proof
- protocol-visible timeout uses resolver-selected explicit route arm
- `?` is the ordinary localside propagation path

Forbidden:

- `recv_timeout` / `send_timeout` / `offer_timeout` / `decode_timeout`
- public `cancel`
- same-generation `try_recover` / `reconnect` / `ignore_fault`
- no public timeout API
- no public cancel / reconnect / same-generation recovery API
- no public wide `HibanaError`
- no public `EndpointErrorKind` / `ResolverErrorKind` / `AttachErrorKind` decision surface
- matching public error kind to choose an alternate branch
- treating `DeadlineExceeded` / transport close / peer reset as route authority
- transport or appkit selecting fallback progress after operational failure
- treating preview/probe mismatch as permission to infer a route or policy
- calling a resolver-selected Timer / clock / interrupt route proof an
  operational timeout fault proof

### WASI Gate

Every artifact must satisfy:

- wasm32-wasip1
- no Preview 2
- no WIT
- no Component Model
- only imports allowed by choreography-derived capacity
- unsupported imports reject
- no raw core-wasm runnable artifact path
- no host filesystem fallback

### Examples Boundary Gate

Examples may contain domain semantics.
Core may not.

Domain examples:

- LLM
- agent
- browser
- settlement
- realtime policy
- approval
- audit

These may appear in examples/tests for specific Capsules.
They must not become appkit/site/choreography public concepts.

### Implementation Hygiene Gate

No implementation may hide unfinished work with surface noise.

Forbidden:

- unnecessary `_name` bindings to silence warnings
- `let _ = ...` escape hatches when the value should be named or handled
- `#[allow(dead_code)]`
- `#[allow(unused)]`
- dead code kept as compatibility surface
- macros used to hide type obligations or generate choreography DSL
- compatibility aliases for deleted public paths

---

## 29. Implementation Phases

初手は明確です。

1. root freeze: choreography / appkit / site only
2. delete `appkit::Choreo` and `appkit::Program`
3. delete `pub mod proof`
4. introduce `Capsule` / `LogicalImage` / `Placement` / `ArtifactBundle` / `run`
5. move Baker to examples/tests proof
6. move metadata derivation to hibana/projection side only if it adds neutral hibana value
7. Cargo workspace physical artifact packages
8. WASI P1-only artifact and import validation
9. private `Guest::resume(BudgetRun)` VM boundary
10. appkit-internal canonical WASI engine bridge through Endpoint / carrier
11. VM fixture quarantine
12. WASI import intern with `Wasip1ImportName` as sole authority
13. typed `Pending<K>` completion only; no root complete methods
14. `proc_exit` as `Event::Exit`, not `Call`
15. mandatory decoded IR; no hot-path opcode decode or block scan
16. allocation-free memory / numeric hot path
17. attached engine invariant
18. same-artifact boundary preservation
19. sealed Localside contexts
20. domain-specific hibana error evidence for `?` propagation
21. raw hibana helper functions only
22. projection-derived logical images
23. `RouteKey` unification
24. ChoreoFS as fact resolver
25. site families
26. heterogeneous example
27. deletion / hygiene

No custom CLI phase.
No codegen phase.
No macro DSL phase.
No general Wasm runtime phase.

---

## 30. Final Examples

Single host native composite:

```rust
fn main() -> host_smoke::Report {
    appkit::run::<
        site::Local<image::Composite>,
        host_smoke::Wasip1Smoke,
    >(
        ARTIFACTS.for_image::<image::Composite>(),
    )
}
```

Build:

```text
cargo build -p host-smoke-example \
  --target x86_64-unknown-linux-gnu \
  --release
```

RP2040 dual-core single firmware:

```rust
fn reset_entry() -> ! {
    match rp2040_sio::core_id() {
        0 => appkit::run::<
            site::Local<image::Driver>,
            baker_link_traffic::Traffic,
        >(ARTIFACTS.for_image::<image::Driver>()),
        _ => appkit::run::<
            site::Local<image::Engine>,
            baker_link_traffic::Traffic,
        >(ARTIFACTS.for_image::<image::Engine>()),
    }
}
```

Build:

```text
cargo build -p baker-firmware \
  --bin baker-traffic \
  --target thumbv6m-none-eabi \
  --release
```

One selected bin target is one physical artifact.
Two logical images.
Same choreography.
Endpoint/carrier boundary still enforced.

Heterogeneous generic split:

```rust
fn site_a_main() -> ! {
    appkit::run::<
        site::Local<image::A>,
        my_capsule::Control,
    >(
        ARTIFACTS.for_image::<image::A>(),
    )
}

fn site_b_main() -> ! {
    appkit::run::<
        site::Local<image::B>,
        my_capsule::Control,
    >(
        ARTIFACTS.for_image::<image::B>(),
    )
}
```

Build:

```text
cargo build -p site-a-artifact \
  --target x86_64-unknown-linux-gnu \
  --release
cargo build -p site-b-artifact \
  --target thumbv7em-none-eabihf \
  --release
```

Same Capsule.
Same raw hibana choreography.
Different physical artifacts.
Different target triples.

---

## 31. Final Invariant List

- There is one projectable raw hibana choreography per Capsule.
- Capsule returns that choreography without user-facing concrete `steps` types.
- Every hibana feature remains usable.
- hibana changes are minimal, neutral, and valuable to hibana itself.
- No `project_role`.
- No public `g::steps`.
- No user-facing `Program<steps::...>`.
- hibana-pico executes only `wasm32-wasip1` / WASI Preview 1 artifacts.
- There is no public general Wasm runtime.
- There may be many WASI P1 engines per Capsule.
- Every WASI P1 engine is attached to the choreography.
- Every import completion crosses endpoint/carrier.
- If a logical image artifact is `WasiImage`, appkit uses the canonical WASI P1 engine driver.
- WASI P1 VM driving is not an example / Capsule localside responsibility.
- Engine logical images execute WASI P1 VM and normalize imports only.
- Engine localside does not own demo loop / ChoreoFS / ChoreoFsObject / ledger / device semantics.
- Repeated visible behavior must be driven by repeated WASI imports admitted by choreography, not synthesized by engine-side counters.
- The VM boundary is private `Guest::init_in_place(ptr, bytes)` and `Guest::resume(BudgetRun)`.
- `Guest::new(bytes)` is not a runtime construction path.
- WASI guest storage is supplied by `LogicalImage` / site-local storage facts as an in-place arena lease.
- appkit never selects stack / heap / static guest construction by platform feature.
- VM completion is typed affine `Pending<K>::complete(...)`, never `Guest::complete`.
- `proc_exit` is `Event::Exit`, not `Call`.
- `proc_raise` is unsupported POSIX self-signal compatibility; it is validation-rejected and never mapped to abort / fence / control choreography.
- `sched_yield` is unsupported POSIX scheduler-hint compatibility; it is validation-rejected and never mapped to hibana resolver / route frontier internals.
- `sock_*` is unsupported POSIX socket compatibility in core; it is validation-rejected and never becomes raw socket authority.
- Network, when needed, is modeled as ChoreoFS object facts + ledger fd materialization + explicit raw hibana choreography, not POSIX socket compatibility.
- Network / swarm liveness is link/carrier fact state, not ChoreoFS authority.
- ChoreoFS catalog entries may remain across disconnect; ledger generation validation prevents stale route use.
- Link/carrier observations never update topology or route witnesses by themselves.
- Topology / recovery / route witness changes require projected hibana builtin control messages.
- `RouteDecision`, `TopologyBegin/Ack/Commit`, `AbortBegin/Ack`, `Fence`, `StateSnapshot/Restore`, `TxCommit/Abort`, and `LoopContinue/Break` stay on the choreography side.
- Host filesystem mutation imports are unsupported in core; ChoreoFS is not a POSIX namespace mutation layer.
- POSIX file tuning / sync / metadata mutation imports are unsupported in core.
- `fd_renumber` is unsupported in core; fd table authority stays in the ledger, not the guest.
- WASI P1 import support is kept only when it has high core value; otherwise the import is deleted or explicitly unsupported.
- WASI is an import namespace, not an engine route.
- Decoded IR is mandatory; hot path does not decode raw opcodes or scan block structure.
- WASI import names are interned through `Wasip1ImportName`; runtime does not compare raw import-name bytes.
- Production VM files do not contain demo fixture byte arrays.
- Every logical image is projection-derived.
- `LogicalImage::REQUESTED_ROLES` is a requested projection slice, not authority.
- `REQUESTED_ROLES` is validated against Placement and projection metadata.
- Each `appkit::run::<LogicalImage, Capsule>()` attaches only that logical image's requested role slice.
- Different cores/processes/address spaces are different logical images.
- A physical Cargo artifact may contain one or more logical images.
- Same artifact never means direct call or authority merge.
- RP2040 core0/core1 split uses `example-defined rp2040_sio::SIO` as a real materialized carrier.
- Every cross-site link carries typed choreography frames only.
- Every external boundary is typed and endpoint-driven.
- Site may host engine capacity but may not complete or authorize WASI P1 imports.
- `RouteKey<Target>` is a derived witness, not app-level authority.
- ChoreoFS is a bounded path/object fact resolver, not host filesystem / POSIX compatibility / hidden fallback.
- ChoreoFS is not route owner, protocol authority, or public Manifest API.
- Public Manifest API is removed, but ChoreoFS facts remain as `DriverCtx` / appkit internal / Capsule-local facts.
- ChoreoFS facts are consumed only at choreography-open phases.
- RouteDecision stays on the choreography side; ChoreoFS does not select routes.
- Ledger materializes fd / rights / resource identity / object generation / derived route witness / route witness generation.
- Link/carrier facts materialize reachability / peer generation / observed epoch; no hidden reconnect or route repair.
- Site provides site facts only.
- Placement decides location, not legality.
- Localside receives sealed contexts only.
- Localside propagates failures with `?`; top-level logical image entry is the only unwrap / panic boundary.
- Hibana error evidence is domain-specific: `EndpointError`, `ResolverError`, and `AttachError`, sharing only caller-location evidence.
- `EndpointError` covers `flow` / `send` / `recv` / `offer` / `decode`; project code must not need wrappers or repeated `map_err` to preserve origin.
- Endpoint error internals are not an appkit decision surface; optional WASI loop-control preview failure falls through to the real import, which remains the authority check.
- Hibana wait outcomes are only `Progress | Fault`.
- committed-progress `Err(domain evidence)` is terminal evidence for the current generation, not a protocol branch.
- preview/probe `Err(domain evidence)` is non-progress and must not infer fallback progress.
- Operational deadlines are internal fuses that poison the generation; they are not public timeout APIs.
- Protocol timeout must be written as choreography with a Timer role / explicit route arm.
- Retry / recovery after operational fault starts a new session generation.
- Public timeout, cancel, reconnect, and same-generation recovery APIs do not exist.
- Public error-kind matching must not become route authority.
- Firmware panic marker behavior is verified by a separate panic-marker artifact, not by bending a successful choreography/WASI demo into failure.
- Capacity derives from hibana/projection metadata, not appkit DSL.
- Metadata derivation is a blocking item.
- Cargo features select implementation capacity, not Capsule meaning.
- Rust/Cargo build physical artifacts; no custom CLI exists.
- Domain semantics live in examples/Capsules, not core.
- No public kernel/machine/port/projects/proof path.
- No compatibility aliases.
- No Preview 2 / WIT / Component Model public path.
- No dead code / allow / underscore escape hatches.
- No macro DSL.
- No heuristic recovery.

---

## 最終結論

これが最小です。

```text
projectable raw hibana in
projection-derived logical images
Cargo-built physical artifacts
every WASI P1 import completion through Endpoint/carrier
```

ユーザーが用意するものはこれだけです。

- raw hibana choreography
- localside
- WASI P1 artifacts
- associated placement

必要な場合だけ custom site family を用意します。

appkit は choreography を包まない。
appkit は projectable raw hibana choreography を project / attach / run する。
site は site facts だけを持つ。ただし engine capacity を含んでもよい。
site は WASI P1 import を完了も authorize もしない。
kernel は private WASI P1 VM / appkit service implementation。
machine/port/projects は空 placeholder として置かない。
proof は examples/tests。
build は Cargo だけ。
domain semantics は examples / user Capsules だけ。
capacity は hibana/projection metadata から導く。
metadata visitor は blocking item。
hibana の fallible boundary error は domain-specific family にする。
`EndpointError` / `ResolverError` / `AttachError` が caller location を持ち、
project localside は wrapper なしで `?` 伝播する。
Wasm execution は private WASI P1 VM boundary に閉じる。
VM construction は in-place のみで、`Guest::new(bytes)` を runtime path にしない。
VM hot path は decoded IR 必須で、raw opcode decode / block scan / allocation を置かない。
WASI import は `Wasip1ImportName` に intern し、runtime で raw import-name bytes を比較しない。
すべての protocol progress の権威は projected hibana choreography だけ。

この形が、実現可能性を保ったまま最も洗練され、最もシンプルです。
