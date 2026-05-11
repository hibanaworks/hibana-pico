# plan_wasi.md -- hibana-pico WASI/Wasm engine 破壊的リファクタリング計画

## 0. 憲法

余分なものは削りきる。

`src/kernel/engine/wasm` は、互換性を守るための engine ではない。`hibana` の choreography を Wasm guest から駆動するための、最小で、型安全で、読める VM boundary として作り直す。

この計画のゴールは「できることを増やす」ことではない。

ゴールは次の 1 行である。

```rust
let event = guest.resume(budget)?;
```

外側が覚える入口は `wasm::Guest` と `Guest::resume` だけにする。

`tiny`、`core`、`wasip1`、`full`、`std profile`、`completion API`、`trap API`、`legacy route`、`shape heuristic` は public concept として残さない。

Pico firmware が stack を増やさず `Guest` を static slot に置くための placement
capacity は、project 側の guest loader に閉じる。engine public API に hidden
constructor を出してはいけない。guest を進める入口は常に
`Guest::resume(BudgetRun)` だけである。

削除は互換性より優先する。

---

## 1. Fame Boy 記事から取り込む原則

この記事から取り込むのは Game Boy 固有の実装ではない。取り込むのは emulator 設計の骨格である。

### 1.1 境界は極小にする

Fame Boy は frontend/core 境界をほぼ「buffer と stepper」だけにした。

`hibana-pico` ではこれを次に読み替える。

```text
frontend/core boundary
  ↓
host/choreography/guest boundary
```

Wasm engine の public surface は、guest を進めて event を返すだけでよい。

```rust
pub struct Guest<'a> { /* private */ }

impl<'a> Guest<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error>;
    pub fn resume(&mut self, budget: BudgetRun) -> Result<Event<'_, 'a>, Error>;
}
```

`complete_fd_write` のような root public method は削除する。

completion は `Guest` の method ではなく、`resume` が返した affine pending token の method にする。

### 1.2 stepper を 1 箇所に置く

Fame Boy の `stepper` は CPU、timer、serial、APU、PPU の同期権威源だった。

`hibana-pico` では `Guest::resume` だけが次を管理する。

- instruction 実行
- fuel 消費
- budget expiry
- host import 到達
- pending host call
- done / exit / trap

複数の `resume_with_fuel`、`resume_with_budget`、`run_until_*` は持たない。

### 1.3 domain は型で絞る

Fame Boy は CPU opcode をそのまま 512 個並べず、`From` / `To` のような domain type へ畳み、違法状態を型で閉じた。

`hibana-pico` では次を型で閉じる。

- raw import name bytes → `Import`
- raw opcode stream → `Instr`
- host call kind → typed `Pending<K>`
- completion kind → `Pending<K>::complete(...)`
- guest pointer → boundary-only typed offset
- profile feature → compile-time derived capacity

### 1.4 hot path は飾らない

Fame Boy では memory access ごとの `MemoryRegion` domain object が性能を壊した。

`hibana-pico` でも interpreter hot path に domain wrapper を置きすぎない。

型は parse / validation / host boundary に置く。実行 loop と memory load/store は direct にする。

### 1.5 driver の権威源は 1 つ

Fame Boy の audio-driven/frame-driven 問題は、driver が複数あると同期が壊れることを示している。

`hibana-pico` では driver は `BudgetRun` だけ。

- host 側が勝手に timeout を延ばさない
- engine 側が request shape を推測しない
- import mismatch を吸収しない
- pending kind を runtime fallback で直さない

### 1.6 read semantics は cache しない

Fame Boy の joypad register は「読む瞬間」に更新する必要があった。

WASI でも同じ。

- iovec は call 開始時に正しく decode する
- guest memory の view は pending token の lifetime に閉じる
- `memory.grow` 後に古い slice/view を使わない
- path/env/fd view は推測 cache しない

### 1.7 correctness は spec test で固定する

Fame Boy は Tetris ROM 駆動から spec test 駆動へ移った。

`hibana-pico` では refactor 前に、Wasm/WASI の仕様境界を test で固定する。

ただし legacy behavior を守る test は書かない。

守るべきなのは choreography boundary と typed guarantee だけである。

---

## 2. 最終構造

### 2.1 module tree

private module は 1 個だけにする。

```text
src/kernel/engine/
  mod.rs
  wasm/
    mod.rs          # public façade
    vm.rs           # private implementation, the only private module
```

`engine/mod.rs` はこれだけでよい。

```rust
pub mod wasm;
```

`wasm/mod.rs` から見える private module はこれだけ。

```rust
mod vm;
```

禁止する module 名:

```rust
mod tiny;    // 禁止: capacity profile を engine route にしてしまう
mod core;    // 禁止: core wasm と wasi wasm を別 engine に見せてしまう
mod wasip1;  // 禁止: WASI を engine route にしてしまう
mod full;    // 禁止: feature profile を public concept にしてしまう
mod compat;  // 禁止: legacy を温存する名前
```

`#[cfg(test)] mod tests` は許可する。ただし test module は engine concept ではない。fixture 専用 production module は作らない。

### 2.2 public façade

`wasm/mod.rs` は 150 行程度を目標にする。

最終 public surface:

```rust
use crate::choreography::protocol::{BudgetExpired, BudgetRun};

mod vm;

pub struct Guest<'a> {
    vm: vm::Vm<'a>,
}

pub enum Event<'g, 'a> {
    Call(Call<'g, 'a>),
    BudgetExpired(BudgetExpired),
    Done,
    Exit(ProcExit),
}

pub enum Call<'g, 'a> {
    FdWrite(Pending<'g, 'a, FdWrite>),
    FdRead(Pending<'g, 'a, FdRead>),
    PollOneoff(Pending<'g, 'a, PollOneoff>),
    SleepUntil(Pending<'g, 'a, SleepUntil>),
    GpioSet(Pending<'g, 'a, GpioSet>),
}

pub struct Pending<'g, 'a, K> {
    guest: &'g mut Guest<'a>,
    call: K,
}

impl<'a> Guest<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        Ok(Self { vm: vm::Vm::new(bytes)? })
    }

    pub fn resume(&mut self, budget: BudgetRun) -> Result<Event<'_, 'a>, Error> {
        let event = self.vm.resume(budget)?;
        Ok(Event::from_vm(self, event))
    }
}
```

`Call::ProcExit` は置かない。

`ProcExit` は completion を必要とする host call ではない。guest が終了を宣言した event である。

```rust
pub enum Event<'g, 'a> {
    Call(Call<'g, 'a>),
    BudgetExpired(BudgetExpired),
    Done,
    Exit(ProcExit),
}
```

この分類で意味が澄む。

```text
Event::Call = host に処理を委譲し、completion が必要なもの
Event::Exit = guest が終了を宣言したもの
Event::Done = guest code が自然終了したもの
```

`Guest::complete` は置かない。

completion は pending token に閉じる。

```rust
impl Pending<'_, '_, FdWrite> {
    pub fn fd(&self) -> Fd;
    pub fn bytes(&self) -> &[u8];

    pub fn complete(self, written: u32, errno: Errno) -> Result<(), Error> {
        self.guest.vm.complete_fd_write(self.call, written, errno)
    }
}
```

この形なら、`FdRead` 待ちに `FdWrite` completion を返すことは型で表現できない。

### 2.3 private VM

`vm.rs` は public API を持たない。

```rust
pub(super) struct Vm<'a> {
    module: Module<'a>,
    memory: Memory,
    values: ValueStack,
    calls: CallStack,
    controls: ControlStack,
    imports: ImportTable,
    pending: PendingSlot,
}

impl<'a> Vm<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, Error>;
    pub(super) fn resume(&mut self, budget: BudgetRun) -> Result<VmEvent<'a>, Error>;

    fn step(&mut self) -> Result<Step<'a>, Error>;
    fn begin_host_call(&mut self, import: ImportId) -> Result<VmEvent<'a>, Error>;
}
```

`vm::Vm` は 1 個の machine である。

`TinyWasmInstance`、`CoreWasmInstance`、`CoreWasip1Instance` のような複数 route は持たない。

### 2.4 VM event

`vm` は `Guest` を知らない。

```rust
enum VmEvent<'a> {
    Call(HostCall<'a>),
    BudgetExpired(BudgetExpired),
    Done,
    Exit(ProcExit),
}
```

`wasm::Guest::resume` が最後に `Pending { guest: self, call }` へ包む。

これにより self-referential 構造を避けつつ、pending token が `&mut Guest` を保持する affine API を成立させる。

---

## 3. 削除対象

### 3.1 削除する public concept

次は削除する。

```text
TinyWasmModule
TinyWasmInstance
CoreWasmModule
CoreWasmInstance
CoreWasip1Instance
CoreWasmTrap
CoreWasip1Trap
GuestTrap
EngineReq を直接返す trap API
resume_with_fuel
resume_with_budget
complete_fd_write
complete_fd_read
complete_poll_oneoff
complete_random_get
complete_path_*
complete_socket_*
Reply enum
Call::ProcExit
legacy demo import route
legacy poll_oneoff shape fallback
unsupported import absorption
route mismatch recovery
```

削除理由:

```text
Tiny/Core/Wasip1 = engine route ではない
Trap = public choreography boundary ではない
complete_* = completion mismatch を runtime に逃がす
Reply enum = kind mismatch を表現できてしまう
Call::ProcExit = completion 不要なのに Call に混ざる
legacy fallback = shape heuristic
```

### 3.2 削除しない guarantee

public surface を削っても、次は保持する。

- import signature validation
- memory bounds check
- memory grow fence
- WASI errno lowering
- fd/iovec/path decode safety
- budget accounting
- one pending at a time
- completion kind correctness
- no stale memory view after grow

surface を削ることは保証を削ることではない。

保証は hidden lower layer に退避する。

---

## 4. profile / feature の扱い

### 4.1 profile は user に ask しない

`Guest::new(bytes)` に profile 引数を渡さない。

```rust
pub fn new(bytes: &'a [u8]) -> Result<Self, Error>;
```

profile は feature matrix から derive する。

```rust
struct ActiveProfile;

impl ActiveProfile {
    const MAX_TYPES: usize = if cfg!(feature = "wasm-engine-wasip1-full") { 32 } else { 16 };
    const MAX_IMPORTS: usize = if cfg!(feature = "wasm-engine-wasip1-full") { 64 } else { 16 };
    const MAX_FUNCTIONS: usize = if cfg!(feature = "wasm-engine-wasip1-full") { 192 } else { 32 };
    const MEMORY_PAGES: u32 = if cfg!(feature = "wasm-engine-wasip1-full") { 32 } else { 1 };
}
```

`Tiny` は profile 名ではなく capacity の結果である。

### 4.2 handler set は public config にしない

`Wasip1HandlerSet` は診断用 value としては残してよい。

ただし engine construction の引数にしない。

```rust
// 禁止
Guest::new(bytes, Wasip1HandlerSet { fd_write: true, ... })
```

静的な差分は `cfg!` / associated const で導く。

### 4.3 WASI は engine ではない

WASI は import namespace であり、engine route ではない。

```rust
enum Import {
    Hibana(HibanaImport),
    Wasip1(Wasip1Import),
}
```

`Vm` は import call に到達したとき、typed host call へ lower するだけ。

---

## 5. import model

### 5.1 byte match を parse phase に閉じる

raw bytes は parse 時点で捨てる。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Import {
    Hibana(HibanaImport),
    Wasip1(Wasip1ImportName),
}

#[derive(Clone, Copy)]
struct ImportSlot {
    func: FuncId,
    ty: TypeId,
    import: Import,
}
```

runtime hot path で `b"fd_write"` と比較しない。

### 5.2 signature table を唯一の権威源にする

```rust
struct Sig {
    params: &'static [ValType],
    results: &'static [ValType],
}

const WASIP1_SIGS: &[(Wasip1ImportName, Sig)] = &[
    (Wasip1ImportName::FdWrite, sig!([I32, I32, I32, I32] -> [I32])),
    (Wasip1ImportName::FdRead, sig!([I32, I32, I32, I32] -> [I32])),
    (Wasip1ImportName::PollOneoff, sig!([I32, I32, I32, I32] -> [I32])),
    (Wasip1ImportName::ProcExit, sig!([I32] -> [])),
];
```

validation と lowering は同じ table を見る。

二重管理は禁止。

### 5.3 unsupported import は吸収しない

unsupported import は validation error にする。

`TypedEnosys` として明示的にサポートするものだけ、typed errno を返す。

```rust
match coverage.effective(ActiveProfile::HANDLERS) {
    Supported => accept(),
    TypedEnosys => accept_typed_enosys(),
    TypedReject | UnsupportedByProfile => reject(),
}
```

「unknown だけど実行時に ENOSYS」にはしない。

---

## 6. decoded instruction IR

### 6.1 raw opcode stream 実行をやめる

実行時に raw bytecode を読み続けない。

parse 時に fixed-capacity IR へ decode する。

```rust
#[derive(Clone, Copy)]
enum Instr {
    Nop,
    Drop,
    Select,

    I32Const(u32),
    I64Const(u64),

    Local(LocalOp),
    Global(GlobalOp),

    Load(Load),
    Store(Store),

    Unary(Unary),
    Binary(Binary),
    Compare(Compare),
    Convert(Convert),

    Block(BlockId),
    Loop(BlockId),
    If(BlockId),
    Else(BlockId),
    Br(Target),
    BrIf(Target),
    BrTable(BrTableId),
    Return,

    Call(FuncId),
    CallIndirect(TypeId),
    HostCall(ImportId),

    MemorySize,
    MemoryGrow,

    End,
}
```

`find_matching_end` / `find_matching_else` は hot path から消す。

branch target は validation 時に確定する。

### 6.2 invalid state を IR で表現しない

次のような raw 状態を runtime に残さない。

```text
unknown opcode
branch to unknown block
call to missing function
import with wrong signature
load/store with invalid width encoding
blocktype mismatch
```

これらは `Guest::new(bytes)` で落とす。

`resume` 中に発生する error は実行時条件だけにする。

```text
memory out of bounds
division by zero
integer overflow trap
call stack overflow
value stack overflow
budget expired
```

### 6.3 no_alloc capacity

IR は `Vec` にしない。

```rust
struct Slice<T, const N: usize> {
    len: usize,
    items: [MaybeUninit<T>; N],
}
```

capacity 超過は validation error。

profile ごとに capacity を持つが、profile 名は public に出さない。

---

## 7. execution stepper

### 7.1 resume loop

`Guest::resume` の内側は 1 個の loop にする。

```rust
impl Vm<'_> {
    fn resume(&mut self, budget: BudgetRun) -> Result<VmEvent<'_>, Error> {
        let mut left = budget;

        loop {
            if left.is_expired() {
                return Ok(VmEvent::BudgetExpired(left.expired()));
            }

            let step = self.step()?;
            left.spend(step.cost)?;

            match step.kind {
                StepKind::Continue => {}
                StepKind::Host(call) => return self.begin_host_call(call),
                StepKind::Done => return Ok(VmEvent::Done),
                StepKind::Exit(code) => return Ok(VmEvent::Exit(code)),
            }
        }
    }
}
```

### 7.2 fuel 単位を固定する

fuel 単位は `1 decoded instruction = 1 fuel` を初期値にする。

cost table を入れるなら、table は private const であり、public mode にはしない。

```rust
impl Instr {
    #[inline(always)]
    fn cost(self) -> u32 {
        1
    }
}
```

### 7.3 timer winter を防ぐ invariant

Fame Boy の timer bug は「instruction 数」と「cycle 数」を混同したことが原因だった。

`hibana-pico` では次を invariant にする。

```text
resume loop は exactly one accounting point per decoded instruction を持つ。
host call は instruction として accounting される。
budget expiry は host call completion を飛ばさない。
pending がある状態では resume できない。
```

---

## 8. pending token

### 8.1 one pending at a time

host call に到達したら `Vm` は pending state に入る。

```rust
enum PendingSlot {
    Empty,
    FdWrite(FdWriteState),
    FdRead(FdReadState),
    PollOneoff(PollOneoffState),
    SleepUntil(SleepUntilState),
    GpioSet(GpioSetState),
}
```

`PendingSlot` は private。

public は typed token のみ。

```rust
pub struct Pending<'g, 'a, K> {
    guest: &'g mut Guest<'a>,
    call: K,
}
```

### 8.2 completion mismatch を型で消す

```rust
impl Pending<'_, '_, FdWrite> {
    pub fn complete(self, written: u32, errno: Errno) -> Result<(), Error>;
}

impl Pending<'_, '_, FdRead> {
    pub fn buffer(&mut self) -> &mut [u8];
    pub fn complete(self, read: u32, errno: Errno) -> Result<(), Error>;
}

impl Pending<'_, '_, PollOneoff> {
    pub fn timeout(&self) -> Option<TimerSleepUntil>;
    pub fn complete(self, result: PollResult) -> Result<(), Error>;
}
```

`Reply` enum は不要。

```rust
// 禁止
pub enum Reply {
    FdWrite { ... },
    FdRead { ... },
    PollOneoff { ... },
}
```

`Reply` enum は kind mismatch を表現できるため削除する。

### 8.3 ProcExit は pending ではない

`proc_exit` は host call 風の import だが、completion を必要としない。

```rust
match import {
    Import::Wasip1(Wasip1ImportName::ProcExit) => {
        let code = self.values.pop_i32()?;
        StepKind::Exit(ProcExit(code))
    }
    _ => StepKind::Host(call),
}
```

public では次になる。

```rust
Event::Exit(ProcExit(code))
```

`Call::ProcExit` は作らない。

---

## 9. memory model

### 9.1 hot path は direct slice

Memory は active length を cache する。

```rust
struct Memory {
    bytes: VmMemory,
    len: usize,
}

impl Memory {
    #[inline(always)]
    fn active(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    #[inline(always)]
    fn active_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.len]
    }

    #[inline(always)]
    fn check(&self, off: usize, width: usize) -> Result<usize, Error> {
        let end = off.checked_add(width).ok_or(Error::MemoryOutOfBounds)?;
        if end <= self.len { Ok(off) } else { Err(Error::MemoryOutOfBounds) }
    }

    #[inline(always)]
    fn read_u32(&self, off: usize) -> Result<u32, Error> {
        let off = self.check(off, 4)?;
        let m = self.active();
        Ok(u32::from_le_bytes([m[off], m[off + 1], m[off + 2], m[off + 3]]))
    }
}
```

`MemoryRegion` のような per-access enum object は作らない。

### 9.2 boundary だけ typed pointer

WASI boundary では typed offset を使う。

```rust
#[derive(Clone, Copy)]
struct GuestPtr<T> {
    off: u32,
    _ty: PhantomData<T>,
}

struct Iovec {
    ptr: GuestPtr<u8>,
    len: u32,
}
```

ただし interpreter hot path は `usize` offset と direct slice でよい。

型安全は boundary で担保する。

### 9.3 memory.grow fence

`memory.grow` 成功時だけ `len` を更新する。

pending token が memory view を保持している間、`resume` は呼べない。

したがって stale view は public API 上発生しない。

```text
Pending<'g, 'a, K> holds &'g mut Guest<'a>
  ↓
Guest::resume requires &mut self
  ↓
pending alive while resume impossible
  ↓
memory.grow cannot race with borrowed view
```

---

## 10. WASI request decoding

### 10.1 shape heuristic を削除する

`poll_oneoff`、fd view、path、iovec は WASI P1 layout だけを読む。

```rust
fn decode_poll_oneoff(memory: &Memory, in_ptr: u32, out_ptr: u32, nsubscriptions: u32) -> Result<PollOneoff, Error> {
    // WASI P1 subscription layout only.
}
```

禁止:

```text
legacy subscription shape fallback
old demo delay encoding
lane mismatch absorption
request shape inference
batch control heuristic
timeout extension
```

古い demo が壊れるなら壊す。

必要なら demo 側を明示 import に直す。

### 10.2 fd/iovec は call 開始時に freeze する

`fd_write` は guest memory から payload を decode し、pending token が読み出せる形にする。

```rust
pub struct FdWrite<'a> {
    fd: Fd,
    bytes: &'a [u8],
}
```

`fd_read` は mutable buffer view を pending token に渡す。

```rust
pub struct FdRead<'a> {
    fd: Fd,
    buf: &'a mut [u8],
}
```

実装上 `&'a mut [u8]` が borrow checker と衝突する場合は、public token には accessor を置き、内部では offset/len を持つ。

```rust
pub struct FdRead {
    fd: Fd,
    off: u32,
    len: u32,
}

impl Pending<'_, '_, FdRead> {
    pub fn buffer(&mut self) -> Result<&mut [u8], Error>;
}
```

この方が stale view と self-referential を避けやすい。

### 10.3 errno lowering は private pure function

```rust
#[inline(always)]
fn errno(e: Errno) -> u32 {
    e as u32
}
```

WASI errno table は 1 箇所に置く。

---

## 11. numeric primitives

### 11.1 inline pure functions

Fame Boy の flags module と同じく、numeric semantics は小さい inline pure function にする。

```rust
mod num {
    #[inline(always)]
    pub(super) fn i32_add(a: u32, b: u32) -> u32 {
        a.wrapping_add(b)
    }

    #[inline(always)]
    pub(super) fn i32_shl(a: u32, b: u32) -> u32 {
        a.wrapping_shl(b & 31)
    }

    #[inline(always)]
    pub(super) fn i32_shr_s(a: u32, b: u32) -> u32 {
        ((a as i32) >> (b & 31)) as u32
    }
}
```

ただし `mod num` を別ファイルにしない。

`vm.rs` 内の private module または private functions でよい。

### 11.2 heap allocation を hot path に入れない

禁止:

```text
Vec allocation during step
Box allocation during step
per-instruction String/Error formatting
function pointer dispatch for numeric ops
per-load domain enum allocation
```

---

## 12. error model

### 12.1 Error は 1 個

public error は 1 型だけ。

```rust
pub struct Error {
    kind: ErrorKind,
}
```

`ErrorKind` は必要なら public にしてよいが、まずは private を基本にする。

```rust
enum ErrorKind {
    InvalidModule,
    UnsupportedImport,
    InvalidSignature,
    CapacityExceeded,
    MemoryOutOfBounds,
    StackOverflow,
    StackUnderflow,
    DivideByZero,
    IntegerOverflow,
    PendingRequired,
    PendingMismatch,
}
```

`PendingMismatch` は public API からは到達不能にする。

内部 invariant check 用には残してよい。

### 12.2 trap API は public にしない

Wasm trap は public route ではない。

```rust
// 禁止
pub enum CoreWasmTrap
pub enum CoreWasip1Trap
pub enum GuestTrap
```

外側は `Error` と `Event` だけを見る。

---

## 13. choreography boundary

### 13.1 EngineReq を leak しない

`EngineReq` は choreography 側 protocol の内部 lower target である。

Wasm engine public API が `EngineReq` を直接返してはいけない。

```rust
// 禁止
Event::Host(EngineReq)
```

public は typed call を返す。

```rust
Event::Call(Call::FdWrite(pending))
```

choreography driver がそれを `flow().send()` / `recv()` / `offer()` / `decode()` に書き下す。

### 13.2 lower は一方向

```text
Wasm import
  → typed Call
  → choreography protocol
  → typed completion on Pending<K>
  → Wasm result writeback
```

逆方向の inference はしない。

---

## 14. tests

### 14.1 refactor 前に固定する tests

legacy preservation ではなく、保証 preservation を test する。

```text
tests/wasm_core/arithmetic.rs
tests/wasm_core/control.rs
tests/wasm_core/memory.rs
tests/wasm_core/call.rs
tests/wasm_import/signature.rs
tests/wasi/fd_write.rs
tests/wasi/fd_read.rs
tests/wasi/poll_oneoff.rs
tests/wasi/proc_exit.rs
tests/wasi/memory_grow_fence.rs
tests/wasi/no_legacy_shape.rs
tests/wasi/pending_affine.rs
```

### 14.2 compile-fail tests

`trybuild` を使えるなら、次を compile-fail にする。

```rust
// FdRead pending に FdWrite completion は存在しない。
call.complete_fd_write(...);
```

```rust
// pending を保持したまま resume できない。
let event = guest.resume(budget)?;
let pending = match event { Event::Call(Call::FdWrite(p)) => p, _ => unreachable!() };
guest.resume(budget)?;
pending.complete(...)?;
```

### 14.3 proc_exit test

`proc_exit` は `Call` ではなく `Event::Exit` を返す。

```rust
#[test]
fn proc_exit_is_exit_event_not_pending_call() {
    let mut guest = Guest::new(PROC_EXIT_WASM).unwrap();
    let event = guest.resume(BudgetRun::unbounded()).unwrap();
    assert!(matches!(event, Event::Exit(ProcExit(7))));
}
```

### 14.4 poll_oneoff legacy rejection

```rust
#[test]
fn poll_oneoff_accepts_only_wasi_subscription_layout() {
    let err = Guest::new(LEGACY_POLL_SHAPE_WASM).unwrap_err();
    assert!(err.is_invalid_module_or_unsupported_import());
}
```

または module 自体は valid でも call 時に invalid memory layout として落とす。

重要なのは fallback しないこと。

### 14.5 accounting invariant

```rust
#[test]
fn budget_is_spent_once_per_decoded_instruction() {
    let mut guest = Guest::new(THREE_NOP_WASM).unwrap();
    let event = guest.resume(BudgetRun::fuel(2)).unwrap();
    assert!(matches!(event, Event::BudgetExpired(_)));
}
```

---

## 15. benchmarks

### 15.1 benchmark は FPS ではなく engine invariant を測る

```text
benches/wasm_decode.rs
benches/wasm_step.rs
benches/memory_load_store.rs
benches/fd_write_roundtrip.rs
benches/poll_oneoff_roundtrip.rs
```

### 15.2 比較対象

refactor 前後で見るもの:

```text
module decode time
instructions / second
memory load/store throughput
host call roundtrip cost
fd_write iovec decode cost
poll_oneoff decode cost
binary size
stack usage
```

### 15.3 release build を基準にする

Fame Boy の反省と同じく、debug build の性能を根拠に設計判断しない。

---

## 16. migration phases

### Phase 1: façade を作る

- `src/kernel/engine/wasm.rs` を `src/kernel/engine/wasm/mod.rs` に移す
- `vm.rs` を追加する
- public façade と private `Vm` を作る
- 既存実装をまず `vm` に移す
- public route は `Guest::new` / `Guest::resume` に寄せ始める

完了条件:

```text
engine/mod.rs は pub mod wasm だけ
wasm/mod.rs は mod vm だけ
外側から tiny/core/wasip1 module が見えない
```

### Phase 2: Tiny/Core/Wasip1 型を消す

- `TinyWasmInstance` を `Vm` に統合
- `CoreWasmInstance` を `Vm` に統合
- `CoreWasip1Instance` を `Vm` に統合
- capacity 差分は `ActiveProfile` associated const に移す

完了条件:

```text
*WasmInstance という public 型が 1 個もない
Guest だけが engine handle
```

### Phase 3: import を intern する

- raw import byte const の重複を削除
- `Wasip1ImportName` を唯一の WASI import identity にする
- signature table を 1 個にする
- validation と lowering を同じ table に繋ぐ

完了条件:

```text
runtime で import name bytes を比較しない
unsupported import fallback がない
```

### Phase 4: Pending token 化

- `complete_*` root method を削除
- `Reply` enum を削除
- `Event::Call(Call::X(Pending<X>))` を導入
- `proc_exit` を `Event::Exit` にする
- one pending invariant を入れる

完了条件:

```text
Guest::complete がない
complete_fd_write など root completion がない
Call::ProcExit がない
Event::Exit がある
```

### Phase 5: shape heuristic 削除

- legacy `poll_oneoff` fallback を削除
- request shape inference を削除
- timeout extension / batch heuristic を削除
- old demo wasm は test fixture から外すか、明示 import に修正する

完了条件:

```text
WASI P1 layout 以外を受け入れない
legacy route test が存在しない
```

### Phase 6: decoded IR 化

- raw opcode stream execution を止める
- fixed-capacity `Instr` に decode する
- branch target を validation で解決する
- hot path の `find_matching_*` を消す

完了条件:

```text
step は Instr を match するだけ
resume 中に control structure scan がない
```

### Phase 7: memory hot path 直書き化

- active memory length を cache
- load/store を direct slice にする
- per-access domain enum を消す
- boundary-only `GuestPtr<T>` にする

完了条件:

```text
load/store path に allocation がない
load/store path に memory region mapping object がない
```

### Phase 8: tests / benches で固定

- spec tests を追加
- compile-fail tests を追加
- release benchmark を追加
- binary size / stack usage を測る

完了条件:

```text
削除してよいものが test に残っていない
保証が test で固定されている
```

---

## 17. 最終 API example

```rust
let mut guest = wasm::Guest::new(bytes)?;

loop {
    match guest.resume(budget)? {
        wasm::Event::Call(wasm::Call::FdWrite(call)) => {
            let written = flow.send(call.bytes())?;
            call.complete(written, Errno::Success)?;
        }
        wasm::Event::Call(wasm::Call::FdRead(mut call)) => {
            let read = flow.recv(call.buffer()?)?;
            call.complete(read, Errno::Success)?;
        }
        wasm::Event::Call(wasm::Call::PollOneoff(call)) => {
            let result = timer.wait(call.timeout())?;
            call.complete(result)?;
        }
        wasm::Event::Call(wasm::Call::SleepUntil(call)) => {
            timer.sleep_until(call.deadline())?;
            call.complete()?;
        }
        wasm::Event::Call(wasm::Call::GpioSet(call)) => {
            gpio.set(call.pin(), call.level())?;
            call.complete()?;
        }
        wasm::Event::BudgetExpired(expired) => {
            return Ok(EngineRet::BudgetExpired(expired));
        }
        wasm::Event::Exit(code) => {
            return Ok(EngineRet::Exit(code));
        }
        wasm::Event::Done => {
            return Ok(EngineRet::Done);
        }
    }
}
```

外側が覚える概念:

```text
Guest
resume
Event
Call
Pending
```

それ以外は VM 内部。

---

## 18. 禁止事項チェックリスト

実装 PR ごとに確認する。

```text
[ ] public API に tiny/core/wasip1/full/std が出ていない
[ ] public API に handler bool set が出ていない
[ ] Guest::complete がない
[ ] complete_* root method がない
[ ] Reply enum がない
[ ] Call::ProcExit がない
[ ] Event::Exit がある
[ ] trap enum が public に出ていない
[ ] EngineReq が wasm public API から返っていない
[ ] legacy fallback がない
[ ] request shape heuristic がない
[ ] import name byte match が runtime hot path にない
[ ] memory load/store hot path に allocation がない
[ ] resume の入口が 1 個だけ
[ ] budget/fuel accounting の権威源が 1 個だけ
[ ] pending 中に resume できない
[ ] completion mismatch が型で表現不能
```

---

## 19. 破壊的変更として受け入れるもの

次は壊してよい。

```text
old TinyWasm API
old CoreWasm API
old CoreWasip1 API
old trap matching code
old completion method call sites
old legacy demo modules
old poll_oneoff layout
old handler-set construction path
old profile selection path
```

互換 shim は作らない。

deprecated alias も作らない。

「残しておく」は禁止。

---

## 20. 完了条件

この refactor は、次を満たしたら完了とする。

```text
1. wasm public API が Guest::new と Guest::resume に収束している。
2. private module が `vm` 1 個だけである。
3. Tiny/Core/Wasip1 が engine route として存在しない。
4. completion は Pending<K>::complete(...) だけである。
5. ProcExit は Event::Exit であり、Call ではない。
6. Reply enum がない。
7. legacy fallback / heuristic がない。
8. import identity は Wasip1ImportName に一本化されている。
9. decoded IR により hot path から raw structure scan が消えている。
10. memory hot path は direct slice である。
11. budget/fuel accounting は resume loop の 1 箇所だけである。
12. spec tests / compile-fail tests / release benches がある。
```

---

## 21. 一文要約

`wasm.rs` を「Wasm/WASI 機能の寄せ集め」から、`Guest::resume` だけで choreography に接続する 1 個の affine VM boundary に作り直す。

型で違法状態を消す。

hot path は飾らない。

legacy は残さない。

`ProcExit` は call ではなく exit event。
