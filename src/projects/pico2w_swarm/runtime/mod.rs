#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::naked_asm,
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr::{read_volatile, write_volatile},
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::{
    Endpoint,
    g::Msg,
    substrate::{
        AttachError, CpError, SessionKit,
        binding::NoBinding,
        ids::SessionId,
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::{
    choreography::protocol::{
        EngineLabelUniverse, EngineReq, EngineRet, FdWrite, FdWriteDone, LABEL_MEM_BORROW_READ,
        LABEL_MEM_RELEASE, LABEL_MGMT_IMAGE_ACTIVATE, LABEL_MGMT_IMAGE_BEGIN,
        LABEL_MGMT_IMAGE_CHUNK, LABEL_MGMT_IMAGE_END, LABEL_MGMT_IMAGE_STATUS,
        LABEL_NET_DATAGRAM_SEND, LABEL_NET_STREAM_WRITE, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, MemBorrow, MemReadGrantControl, MemRelease, MemRights,
        MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd, MgmtStatus,
        MgmtStatusCode, NetworkDatagramSendRouteControl, NetworkStreamWriteRouteControl,
    },
    choreography::swarm::{
        coordinator_program_6, coordinator_program_for, role1_program_6, role1_program_for,
        role2_program_6, role2_program_for, role3_program_6, role3_program_for, role4_program_6,
        role4_program_for, role5_program_6, role5_program_for,
    },
    kernel::metrics::{
        PICO2W_SWARM_DEFAULT_NODES, PICO2W_SWARM_MIN_NODES, pico2w_swarm_expected_aggregate,
        pico2w_swarm_sample_value,
    },
    kernel::mgmt::{ActivationBoundary, ImageSlotTable, MgmtControl},
    kernel::network::{
        DatagramAck, DatagramAckMsg, DatagramSend, DatagramSendMsg, NET_STREAM_FLAG_FIN,
        NetworkObjectReadRoute, NetworkObjectTable, NetworkObjectWriteRoute, NetworkRights,
        NetworkRoute, StreamAck, StreamAckMsg, StreamWrite, StreamWriteMsg,
    },
    kernel::policy::{
        NodeImageUpdated, NodeImageUpdatedMsg, NodeRole, RoleMask, SwarmTelemetry,
        SwarmTelemetryMsg,
    },
    kernel::remote::{
        RemoteActuateAck, RemoteActuateReqMsg, RemoteActuateRequest, RemoteActuateRetMsg,
        RemoteSample, RemoteSampleReqMsg, RemoteSampleRequest, RemoteSampleRetMsg,
    },
    kernel::swarm::{NodeId, SwarmCredential, SwarmSecurity},
    kernel::wasi::{MemoryLeaseTable, Wasip1StdoutModule},
    machine::rp2350::cyw43439::{self, QEMU_CYW43439_MAX_ROLES, QemuCyw43439Transport},
    substrate::exec::{park, run_current_task, signal, wait_until},
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SwarmKernelRole {
    Dynamic,
    Coordinator,
    Sensor,
    Coordinator6,
    Sensor2Of6,
    Sensor3Of6,
    Sensor4Of6,
    Sensor5Of6,
    Sensor6Of6,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn configured_node_role(qemu_role: u8) -> u8 {
    match crate::SWARM_KERNEL_ROLE {
        SwarmKernelRole::Dynamic => qemu_role,
        SwarmKernelRole::Coordinator | SwarmKernelRole::Coordinator6 => {
            cyw43439::NODE_ROLE_COORDINATOR
        }
        SwarmKernelRole::Sensor
        | SwarmKernelRole::Sensor2Of6
        | SwarmKernelRole::Sensor3Of6
        | SwarmKernelRole::Sensor4Of6
        | SwarmKernelRole::Sensor5Of6
        | SwarmKernelRole::Sensor6Of6 => cyw43439::NODE_ROLE_SENSOR,
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fixed_node_count() -> Option<u8> {
    match crate::SWARM_KERNEL_ROLE {
        SwarmKernelRole::Coordinator6
        | SwarmKernelRole::Sensor2Of6
        | SwarmKernelRole::Sensor3Of6
        | SwarmKernelRole::Sensor4Of6
        | SwarmKernelRole::Sensor5Of6
        | SwarmKernelRole::Sensor6Of6 => Some(6),
        _ => None,
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fixed_sensor_hibana_role() -> Option<u8> {
    match crate::SWARM_KERNEL_ROLE {
        SwarmKernelRole::Sensor2Of6 => Some(1),
        SwarmKernelRole::Sensor3Of6 => Some(2),
        SwarmKernelRole::Sensor4Of6 => Some(3),
        SwarmKernelRole::Sensor5Of6 => Some(4),
        SwarmKernelRole::Sensor6Of6 => Some(5),
        _ => None,
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" {
    static __stack_top: u32;
    static __core1_stack_top: u32;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_BASE: usize = 0xD000_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SIO_CPUID: *const u32 = SIO_BASE as *const u32;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_BASE: usize = 0x4007_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTDR: *mut u32 = UART0_BASE as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTFR: *const u32 = (UART0_BASE + 0x18) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTIBRD: *mut u32 = (UART0_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTFBRD: *mut u32 = (UART0_BASE + 0x28) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTLCR_H: *mut u32 = (UART0_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTCR: *mut u32 = (UART0_BASE + 0x30) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART_TXFF: u32 = 1 << 5;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const COORDINATOR: NodeId = NodeId::new(1);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SENSOR: NodeId = NodeId::new(2);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SESSION_GENERATION: u16 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SWARM_CREDENTIAL: SwarmCredential = SwarmCredential::new(0x4849_4241);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SECURITY: SwarmSecurity = SwarmSecurity::Secure(SWARM_CREDENTIAL);
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESULT_SUCCESS: u32 = 0x4849_4f4b;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const RESULT_FAILURE: u32 = 0x4849_4641;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SLAB_BYTES: usize = 262 * 1024;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "embed-wasip1-artifacts"
))]
const WASIP1_SENSOR_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/swarm-sensor.wasm"
));
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "embed-wasip1-artifacts")
))]
const WASIP1_SENSOR_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stdout\n";
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "embed-wasip1-artifacts"
))]
const WASIP1_SENSOR_STDOUT_MARKER: &[u8] = b"hibana swarm sensor";
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "embed-wasip1-artifacts")
))]
const WASIP1_SENSOR_STDOUT_MARKER: &[u8] = b"hibana wasip1 stdout\n";
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WASI_STDOUT_FD: u8 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WASI_MEMORY_LEN: u32 = 4096;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WASI_MEMORY_EPOCH: u32 = 1;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WASI_STDOUT_PTR: u32 = 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const WASI_START_VALUE: u32 = 0x5741_5349;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const REMOTE_ACTUATOR_FD: u8 = 21;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const QEMU_MGMT_IMAGE_SLOT: u8 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const QEMU_MGMT_IMAGE_GENERATION: u32 = 7;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const QEMU_MGMT_FENCE_EPOCH: u32 = 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const QEMU_MGMT_IMAGE: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1";

#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoTransport = QemuCyw43439Transport;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoKit = SessionKit<'static, DemoTransport, EngineLabelUniverse, CounterClock, 1>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore0Endpoint = Endpoint<'static, 0>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore1Endpoint = Endpoint<'static, 1>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore2Endpoint = Endpoint<'static, 2>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore3Endpoint = Endpoint<'static, 3>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore4Endpoint = Endpoint<'static, 4>;
#[cfg(all(target_arch = "arm", target_os = "none"))]
type DemoCore5Endpoint = Endpoint<'static, 5>;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
static mut HIBANA_DEMO_RESULT: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut NODE_ROLE: u32 = cyw43439::NODE_ROLE_DUAL_CORE as u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut NODE_ID: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut NODE_COUNT: u32 = 2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut RUNTIME_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_LOCK_WANT: [u32; 2] = [0; 2];
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_LOCK_TURN: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[repr(C)]
struct VectorTable {
    initial_stack_pointer: *const u32,
    reset: unsafe extern "C" fn() -> !,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for VectorTable {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".vector_table.reset_vector")]
#[used]
static VECTOR_TABLE: VectorTable = VectorTable {
    initial_stack_pointer: core::ptr::addr_of!(__stack_top) as *const u32,
    reset,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntime {
    clock: CounterClock,
    tap: [TapEvent; 128],
    slab: [u8; SLAB_BYTES],
    kit: MaybeUninit<DemoKit>,
    core0_endpoint: MaybeUninit<DemoCore0Endpoint>,
    core1_endpoint: MaybeUninit<DemoCore1Endpoint>,
    core2_endpoint: MaybeUninit<DemoCore2Endpoint>,
    core3_endpoint: MaybeUninit<DemoCore3Endpoint>,
    core4_endpoint: MaybeUninit<DemoCore4Endpoint>,
    core5_endpoint: MaybeUninit<DemoCore5Endpoint>,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl SharedRuntime {
    const fn new() -> Self {
        Self {
            clock: CounterClock::new(),
            tap: [TapEvent::zero(); 128],
            slab: [0; SLAB_BYTES],
            kit: MaybeUninit::uninit(),
            core0_endpoint: MaybeUninit::uninit(),
            core1_endpoint: MaybeUninit::uninit(),
            core2_endpoint: MaybeUninit::uninit(),
            core3_endpoint: MaybeUninit::uninit(),
            core4_endpoint: MaybeUninit::uninit(),
            core5_endpoint: MaybeUninit::uninit(),
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct SharedRuntimeCell(UnsafeCell<SharedRuntime>);

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe impl Sync for SharedRuntimeCell {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
static SHARED_RUNTIME: SharedRuntimeCell = SharedRuntimeCell(UnsafeCell::new(SharedRuntime::new()));

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(naked)]
#[unsafe(export_name = "Reset")]
pub unsafe extern "C" fn reset() -> ! {
    naked_asm!(
        "ldr r0, =0xD0000000",
        "ldr r0, [r0]",
        "ldr r2, ={entry}",
        "cmp r0, #0",
        "beq 1f",
        "ldr r1, ={core1_stack_top}",
        "mov sp, r1",
        "bx r2",
        "1:",
        "bx r2",
        core1_stack_top = sym __core1_stack_top,
        entry = sym reset_entry,
    )
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe extern "C" fn reset_entry() -> ! {
    match core_id() {
        0 => core0_main(),
        _ => core1_main(),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core_id() -> u32 {
    unsafe { read_volatile(SIO_CPUID) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_lock() {
    let me = core_id() as usize;
    let other = 1usize.saturating_sub(me);
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_WANT[me]), 1);
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_TURN), other as u32);
    }
    while unsafe { read_volatile(core::ptr::addr_of!(UART_LOCK_WANT[other])) } != 0
        && unsafe { read_volatile(core::ptr::addr_of!(UART_LOCK_TURN)) } == other as u32
    {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_unlock() {
    let me = core_id() as usize;
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_WANT[me]), 0);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn shared_runtime_ptr() -> *mut SharedRuntime {
    SHARED_RUNTIME.0.get()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core0_endpoint() -> &'static mut DemoCore0Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core0_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core1_endpoint() -> &'static mut DemoCore1Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core1_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core2_endpoint() -> &'static mut DemoCore2Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core2_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core3_endpoint() -> &'static mut DemoCore3Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core3_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core4_endpoint() -> &'static mut DemoCore4Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core4_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn shared_core5_endpoint() -> &'static mut DemoCore5Endpoint {
    unsafe { &mut *(*shared_runtime_ptr()).core5_endpoint.as_mut_ptr() }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn sample_value_for(node: NodeId) -> u32 {
    pico2w_swarm_sample_value(node.raw())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn expected_swarm_sum(node_count: u8) -> u32 {
    pico2w_swarm_expected_aggregate(node_count)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_init() {
    unsafe {
        write_volatile(UARTCR, 0);
        write_volatile(UARTIBRD, 81);
        write_volatile(UARTFBRD, 24);
        write_volatile(UARTLCR_H, 0x60);
        write_volatile(UARTCR, 0x101);
        UART_READY = 1;
    }
    signal();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_putc(byte: u8) {
    while unsafe { read_volatile(UARTFR) } & UART_TXFF != 0 {}
    unsafe { write_volatile(UARTDR, byte as u32) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_puts(text: &str) {
    for byte in text.bytes() {
        if byte == b'\n' {
            uart_putc(b'\r');
        }
        uart_putc(byte);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_bytes(bytes: &[u8]) {
    for byte in bytes {
        if *byte == b'\n' {
            uart_putc(b'\r');
        }
        uart_putc(*byte);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_hex(value: u32) {
    for shift in (0..8).rev() {
        let nibble = ((value >> (shift * 4)) & 0xf) as u8;
        let ch = match nibble {
            0..=9 => b'0' + nibble,
            _ => b'a' + (nibble - 10),
        };
        uart_putc(ch);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_line(text: &str) {
    uart_lock();
    uart_puts(text);
    uart_puts("\n");
    uart_unlock();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn uart_hex_line(prefix: &str, value: u32) {
    uart_lock();
    uart_puts(prefix);
    uart_hex(value);
    uart_puts("\n");
    uart_unlock();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn fail_closed(stage: &str) -> ! {
    unsafe {
        HIBANA_DEMO_RESULT = RESULT_FAILURE;
    }
    uart_lock();
    uart_puts(stage);
    uart_puts(" fail\n");
    uart_unlock();
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn must_attach<T>(result: Result<T, AttachError>, stage: &str) -> T {
    match result {
        Ok(value) => value,
        Err(AttachError::Control(CpError::ResourceExhausted)) => {
            uart_line("[attach] control resource exhausted");
            fail_closed(stage)
        }
        Err(AttachError::Control(_)) => fail_closed(stage),
        Err(AttachError::Rendezvous(_)) => fail_closed(stage),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn init_radio_once() -> (u8, NodeId, u8) {
    if cyw43439::init().is_err() {
        fail_closed("[core0] cyw43439 init");
    }
    uart_line("[core0] cyw43439 firmware ready");
    let role = configured_node_role(cyw43439::node_role());
    let raw_node = cyw43439::node_id();
    let local_node = if raw_node == 0 {
        if role == cyw43439::NODE_ROLE_COORDINATOR {
            COORDINATOR
        } else {
            SENSOR
        }
    } else {
        NodeId::new(raw_node as u16)
    };
    let mut node_count = cyw43439::node_count();
    if node_count < PICO2W_SWARM_MIN_NODES {
        node_count = PICO2W_SWARM_MIN_NODES;
    }
    if node_count > PICO2W_SWARM_DEFAULT_NODES {
        node_count = PICO2W_SWARM_DEFAULT_NODES;
    }
    if let Some(expected) = fixed_node_count() {
        if node_count != expected {
            fail_closed("[core0] fixed swarm node count");
        }
    }

    match role {
        cyw43439::NODE_ROLE_COORDINATOR => uart_line("[core0] node role coordinator"),
        cyw43439::NODE_ROLE_SENSOR => uart_line("[core0] node role sensor"),
        cyw43439::NODE_ROLE_DUAL_CORE => uart_line("[core0] node role dual"),
        _ => fail_closed("[core0] node role"),
    }
    uart_hex_line("[core0] local node 0x", local_node.raw() as u32);
    uart_hex_line("[core0] swarm nodes 0x", node_count as u32);

    unsafe {
        NODE_ROLE = role as u32;
        NODE_ID = local_node.raw() as u32;
        NODE_COUNT = node_count as u32;
    }
    (role, local_node, node_count)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn role_nodes() -> [NodeId; QEMU_CYW43439_MAX_ROLES] {
    [
        NodeId::new(1),
        NodeId::new(2),
        NodeId::new(3),
        NodeId::new(4),
        NodeId::new(5),
        NodeId::new(6),
    ]
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn local_hibana_role(local_node: NodeId, node_count: u8) -> u8 {
    let raw = local_node.raw();
    if raw < PICO2W_SWARM_MIN_NODES as u16 || raw > node_count as u16 {
        fail_closed("[runtime] local sensor role");
    }
    (raw as u8).saturating_sub(1)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn expect_qemu_rx_meta(
    local_role: u8,
    local_node: NodeId,
    source_node: NodeId,
    lane: u8,
    stage: &str,
) {
    let Some(meta) = cyw43439::qemu_last_rx_meta(local_role) else {
        fail_closed(stage);
    };
    if !meta.matches(source_node, local_node, lane) {
        uart_hex_line("[rx] actual src 0x", meta.src_node().raw() as u32);
        uart_hex_line("[rx] expected src 0x", source_node.raw() as u32);
        uart_hex_line("[rx] actual dst 0x", meta.dst_node().raw() as u32);
        uart_hex_line("[rx] expected dst 0x", local_node.raw() as u32);
        uart_hex_line("[rx] actual lane 0x", meta.lane() as u32);
        uart_hex_line("[rx] expected lane 0x", lane as u32);
        fail_closed(stage);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn install_runtime_session(role: u8, local_node: NodeId, node_count: u8) {
    let runtime = shared_runtime_ptr();
    unsafe {
        for byte in (*runtime).slab.iter_mut() {
            *byte = 0;
        }
        for event in (*runtime).tap.iter_mut() {
            *event = TapEvent::zero();
        }

        let kit_ptr = (*runtime).kit.as_mut_ptr();
        kit_ptr.write(SessionKit::new(&(*runtime).clock));
        let kit = &*kit_ptr;
        let rv = match kit.add_rendezvous_from_config(
            Config::new(&mut (*runtime).tap, &mut (*runtime).slab)
                .with_lane_range(0..25)
                .with_universe(EngineLabelUniverse),
            QemuCyw43439Transport::new_role_map(
                role_nodes(),
                node_count,
                SESSION_GENERATION,
                SECURITY,
            ),
        ) {
            Ok(rv) => rv,
            Err(_) => fail_closed("[runtime] add rendezvous"),
        };

        let sid = SessionId::new(2350);
        match crate::SWARM_KERNEL_ROLE {
            SwarmKernelRole::Coordinator6 => {
                if role != cyw43439::NODE_ROLE_COORDINATOR || local_node != COORDINATOR {
                    fail_closed("[runtime] coordinator6 role");
                }
                (*runtime).core0_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, coordinator_program_6(), NoBinding),
                    "[core0] attach endpoint",
                ));
            }
            SwarmKernelRole::Sensor2Of6 => {
                if local_hibana_role(local_node, node_count) != 1 {
                    fail_closed("[runtime] sensor2 role");
                }
                (*runtime).core1_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, role1_program_6(), NoBinding),
                    "[core1] attach endpoint",
                ));
            }
            SwarmKernelRole::Sensor3Of6 => {
                if local_hibana_role(local_node, node_count) != 2 {
                    fail_closed("[runtime] sensor3 role");
                }
                (*runtime).core2_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, role2_program_6(), NoBinding),
                    "[core2] attach endpoint",
                ));
            }
            SwarmKernelRole::Sensor4Of6 => {
                if local_hibana_role(local_node, node_count) != 3 {
                    fail_closed("[runtime] sensor4 role");
                }
                (*runtime).core3_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, role3_program_6(), NoBinding),
                    "[core3] attach endpoint",
                ));
            }
            SwarmKernelRole::Sensor5Of6 => {
                if local_hibana_role(local_node, node_count) != 4 {
                    fail_closed("[runtime] sensor5 role");
                }
                (*runtime).core4_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, role4_program_6(), NoBinding),
                    "[core4] attach endpoint",
                ));
            }
            SwarmKernelRole::Sensor6Of6 => {
                if local_hibana_role(local_node, node_count) != 5 {
                    fail_closed("[runtime] sensor6 role");
                }
                (*runtime).core5_endpoint.as_mut_ptr().write(must_attach(
                    kit.enter(rv, sid, role5_program_6(), NoBinding),
                    "[core5] attach endpoint",
                ));
            }
            _ => {
                if role == cyw43439::NODE_ROLE_COORDINATOR || role == cyw43439::NODE_ROLE_DUAL_CORE
                {
                    let program = coordinator_program_for(node_count)
                        .unwrap_or_else(|| fail_closed("[runtime] coordinator program"));
                    (*runtime).core0_endpoint.as_mut_ptr().write(must_attach(
                        kit.enter(rv, sid, program, NoBinding),
                        "[core0] attach endpoint",
                    ));
                }
                if role == cyw43439::NODE_ROLE_SENSOR || role == cyw43439::NODE_ROLE_DUAL_CORE {
                    match local_hibana_role(local_node, node_count) {
                        1 => (*runtime).core1_endpoint.as_mut_ptr().write(must_attach(
                            kit.enter(
                                rv,
                                sid,
                                role1_program_for(node_count)
                                    .unwrap_or_else(|| fail_closed("[runtime] role1 program")),
                                NoBinding,
                            ),
                            "[core1] attach endpoint",
                        )),
                        2 => (*runtime).core2_endpoint.as_mut_ptr().write(must_attach(
                            kit.enter(
                                rv,
                                sid,
                                role2_program_for(node_count)
                                    .unwrap_or_else(|| fail_closed("[runtime] role2 program")),
                                NoBinding,
                            ),
                            "[core2] attach endpoint",
                        )),
                        3 => (*runtime).core3_endpoint.as_mut_ptr().write(must_attach(
                            kit.enter(
                                rv,
                                sid,
                                role3_program_for(node_count)
                                    .unwrap_or_else(|| fail_closed("[runtime] role3 program")),
                                NoBinding,
                            ),
                            "[core3] attach endpoint",
                        )),
                        4 => (*runtime).core4_endpoint.as_mut_ptr().write(must_attach(
                            kit.enter(
                                rv,
                                sid,
                                role4_program_for(node_count)
                                    .unwrap_or_else(|| fail_closed("[runtime] role4 program")),
                                NoBinding,
                            ),
                            "[core4] attach endpoint",
                        )),
                        5 => (*runtime).core5_endpoint.as_mut_ptr().write(must_attach(
                            kit.enter(
                                rv,
                                sid,
                                role5_program_for(node_count)
                                    .unwrap_or_else(|| fail_closed("[runtime] role5 program")),
                                NoBinding,
                            ),
                            "[core5] attach endpoint",
                        )),
                        _ => fail_closed("[runtime] sensor endpoint"),
                    }
                }
            }
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn publish_runtime_ready() {
    unsafe {
        RUNTIME_READY = 1;
    }
    signal();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_session(endpoint: &mut DemoCore0Endpoint, node_count: u8) {
    let mut aggregate = 0u32;
    let mut node = PICO2W_SWARM_MIN_NODES;
    while node <= node_count {
        let sensor_node = NodeId::new(node as u16);
        let request = RemoteSampleRequest::new(1, 1, sensor_node.raw() as u8);
        uart_hex_line(
            "[core0] remote sample req node 0x",
            sensor_node.raw() as u32,
        );
        match endpoint
            .flow::<RemoteSampleReqMsg>()
            .expect("core0 flow<remote sample>")
            .send(&request)
            .await
        {
            Ok(_) => {}
            Err(_) => fail_closed("[core0] send remote sample"),
        }

        let sample = match endpoint.recv::<RemoteSampleRetMsg>().await {
            Ok(sample) => sample,
            Err(_) => fail_closed("[core0] recv remote sample"),
        };
        uart_hex_line("[core0] sample value 0x", sample.value());
        let expected = sample_value_for(sensor_node);

        if sample.sensor_id() != sensor_node.raw() as u8 || sample.value() != expected {
            unsafe {
                HIBANA_DEMO_RESULT = RESULT_FAILURE;
            }
            uart_line("[core0] hibana pico2w cyw43439 swarm fail");
            fail_closed("[core0] sample mismatch");
        }
        aggregate = aggregate.wrapping_add(sample.value());
        node = node.saturating_add(1);
    }

    if aggregate != expected_swarm_sum(node_count) {
        fail_closed("[core0] aggregate mismatch");
    }
    uart_hex_line("[core0] swarm aggregate 0x", aggregate);

    let mut node = PICO2W_SWARM_MIN_NODES;
    while node <= node_count {
        let sensor_node = NodeId::new(node as u16);
        core0_start_wasip1(endpoint, sensor_node).await;
        core0_wasip1_fd_write(endpoint, sensor_node).await;
        node = node.saturating_add(1);
    }

    let mut node = PICO2W_SWARM_MIN_NODES;
    while node <= node_count {
        let sensor_node = NodeId::new(node as u16);
        let command = RemoteActuateRequest::new(2, 1, sensor_node.raw() as u8, aggregate);
        match endpoint
            .flow::<RemoteActuateReqMsg>()
            .expect("core0 flow<aggregate actuate>")
            .send(&command)
            .await
        {
            Ok(_) => {}
            Err(_) => fail_closed("[core0] send aggregate command"),
        }

        let ack = match endpoint.recv::<RemoteActuateRetMsg>().await {
            Ok(ack) => ack,
            Err(_) => fail_closed("[core0] recv aggregate ack"),
        };
        if ack.channel() != sensor_node.raw() as u8 || ack.result() != 0 {
            fail_closed("[core0] aggregate ack mismatch");
        }
        uart_hex_line("[core0] aggregate ack node 0x", sensor_node.raw() as u32);
        node = node.saturating_add(1);
    }

    if node_count == QEMU_CYW43439_MAX_ROLES as u8 {
        core0_remote_actuator(endpoint, NodeId::new(3), aggregate).await;
        core0_expect_gateway_telemetry(endpoint, NodeId::new(3)).await;
        core0_network_object(endpoint, NodeId::new(4), aggregate).await;
        core0_remote_management(endpoint, NodeId::new(5)).await;
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_remote_actuator(
    endpoint: &mut DemoCore0Endpoint,
    actuator_node: NodeId,
    aggregate: u32,
) {
    let command = RemoteActuateRequest::new(
        REMOTE_ACTUATOR_FD,
        SESSION_GENERATION,
        actuator_node.raw() as u8,
        aggregate ^ 0x0000_a5a5,
    );
    match endpoint
        .flow::<RemoteActuateReqMsg>()
        .expect("core0 flow<remote actuator>")
        .send(&command)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send remote actuator command"),
    }
    let ack = match endpoint.recv::<RemoteActuateRetMsg>().await {
        Ok(ack) => ack,
        Err(_) => fail_closed("[core0] recv remote actuator ack"),
    };
    if ack.channel() != actuator_node.raw() as u8 || ack.result() != 0 {
        fail_closed("[core0] remote actuator ack mismatch");
    }
    uart_hex_line(
        "[core0] remote actuator route ack node 0x",
        actuator_node.raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_expect_gateway_telemetry(endpoint: &mut DemoCore0Endpoint, source_node: NodeId) {
    let telemetry = match endpoint.recv::<SwarmTelemetryMsg>().await {
        Ok(telemetry) => telemetry,
        Err(_) => fail_closed("[core0] recv gateway telemetry acceptance"),
    };
    if telemetry.node_id() != source_node
        || !telemetry.role_mask().contains(NodeRole::Actuator)
        || telemetry.session_generation() != SESSION_GENERATION
    {
        fail_closed("[core0] gateway telemetry acceptance mismatch");
    }
    uart_hex_line(
        "[core0] gateway telemetry accepted node 0x",
        telemetry.node_id().raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_network_object(
    endpoint: &mut DemoCore0Endpoint,
    gateway_node: NodeId,
    aggregate: u32,
) {
    let mut network_objects: NetworkObjectTable<2> = NetworkObjectTable::new();
    let datagram_fd = network_objects
        .apply_cap_grant_datagram(
            COORDINATOR,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            gateway_node,
            22,
            LABEL_NET_DATAGRAM_SEND,
            NetworkRights::Send,
        )
        .unwrap_or_else(|_| fail_closed("[core0] grant datagram network object"));
    let stream_fd = network_objects
        .apply_cap_grant_stream(
            COORDINATOR,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            gateway_node,
            23,
            LABEL_NET_STREAM_WRITE,
            NetworkRights::Send,
        )
        .unwrap_or_else(|_| fail_closed("[core0] grant stream network object"));

    let datagram_fd = match network_objects.route_fd_write_routed(
        datagram_fd.fd(),
        datagram_fd.generation(),
        datagram_fd.route_key(),
    ) {
        NetworkObjectWriteRoute::Datagram(fd) => fd,
        NetworkObjectWriteRoute::Stream(_) => fail_closed("[core0] datagram selected stream route"),
        NetworkObjectWriteRoute::Rejected(_) => fail_closed("[core0] datagram route rejected"),
    };
    match endpoint
        .flow::<NetworkDatagramSendRouteControl>()
        .expect("core0 flow<datagram route control>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] select datagram route"),
    }
    let datagram_payload = b"qemu datagram fd";
    let datagram = DatagramSend::new(
        datagram_fd.fd(),
        datagram_fd.generation(),
        datagram_fd.route(),
        datagram_payload,
    )
    .unwrap_or_else(|_| fail_closed("[core0] make datagram fd send"));
    match endpoint
        .flow::<DatagramSendMsg>()
        .expect("core0 flow<datagram send>")
        .send(&datagram)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send datagram fd"),
    }
    let datagram_ack = match endpoint.recv::<DatagramAckMsg>().await {
        Ok(ack) => ack,
        Err(_) => fail_closed("[core0] recv datagram ack"),
    };
    expect_qemu_rx_meta(
        0,
        COORDINATOR,
        gateway_node,
        22,
        "[core0] datagram ack source",
    );
    if !datagram_ack.accepted_for(datagram_fd.fd(), datagram_fd.generation()) {
        fail_closed("[core0] datagram ack mismatch");
    }
    uart_hex_line(
        "[core0] network datagram fd ack node 0x",
        gateway_node.raw() as u32,
    );

    let stream_fd = match network_objects.route_fd_write_routed(
        stream_fd.fd(),
        stream_fd.generation(),
        stream_fd.route_key(),
    ) {
        NetworkObjectWriteRoute::Stream(fd) => fd,
        NetworkObjectWriteRoute::Datagram(_) => {
            fail_closed("[core0] stream selected datagram route")
        }
        NetworkObjectWriteRoute::Rejected(_) => fail_closed("[core0] stream route rejected"),
    };
    match endpoint
        .flow::<NetworkStreamWriteRouteControl>()
        .expect("core0 flow<stream route control>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] select stream route"),
    }
    let sequence = (aggregate as u16).wrapping_add(gateway_node.raw());
    let stream = StreamWrite::new(
        stream_fd.fd(),
        stream_fd.generation(),
        stream_fd.route(),
        sequence,
        NET_STREAM_FLAG_FIN,
        b"qemu stream fd",
    )
    .unwrap_or_else(|_| fail_closed("[core0] make stream fd write"));
    match endpoint
        .flow::<StreamWriteMsg>()
        .expect("core0 flow<stream write>")
        .send(&stream)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send stream fd"),
    }
    let stream_ack = match endpoint.recv::<StreamAckMsg>().await {
        Ok(ack) => ack,
        Err(_) => fail_closed("[core0] recv stream ack"),
    };
    expect_qemu_rx_meta(
        0,
        COORDINATOR,
        gateway_node,
        23,
        "[core0] stream ack source",
    );
    if !stream_ack.accepted_for(stream_fd.fd(), stream_fd.generation(), sequence) {
        fail_closed("[core0] stream ack mismatch");
    }
    uart_hex_line(
        "[core0] network stream fd ack node 0x",
        gateway_node.raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_remote_management(endpoint: &mut DemoCore0Endpoint, managed_node: NodeId) {
    let begin = MgmtImageBegin::new(
        QEMU_MGMT_IMAGE_SLOT,
        QEMU_MGMT_IMAGE.len() as u32,
        QEMU_MGMT_IMAGE_GENERATION,
    );
    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
        .expect("core0 flow<mgmt begin>")
        .send(&begin)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send mgmt image begin"),
    }
    core0_expect_mgmt_status(endpoint, MgmtStatusCode::Ok, "[core0] mgmt begin status").await;

    let chunk = MgmtImageChunk::new(QEMU_MGMT_IMAGE_SLOT, 0, QEMU_MGMT_IMAGE)
        .unwrap_or_else(|_| fail_closed("[core0] make mgmt image chunk"));
    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
        .expect("core0 flow<mgmt chunk>")
        .send(&chunk)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send mgmt image chunk"),
    }
    core0_expect_mgmt_status(endpoint, MgmtStatusCode::Ok, "[core0] mgmt chunk status").await;

    let end = MgmtImageEnd::new(QEMU_MGMT_IMAGE_SLOT, QEMU_MGMT_IMAGE.len() as u32);
    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
        .expect("core0 flow<mgmt end>")
        .send(&end)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send mgmt image end"),
    }
    core0_expect_mgmt_status(endpoint, MgmtStatusCode::Ok, "[core0] mgmt end status").await;

    let activate = MgmtImageActivate::new(QEMU_MGMT_IMAGE_SLOT, QEMU_MGMT_FENCE_EPOCH);
    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
        .expect("core0 flow<mgmt activate need fence>")
        .send(&activate)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send mgmt activate need fence"),
    }
    core0_expect_mgmt_status(
        endpoint,
        MgmtStatusCode::NeedFence,
        "[core0] mgmt need-fence status",
    )
    .await;

    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
        .expect("core0 flow<mgmt activate>")
        .send(&activate)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send mgmt activate"),
    }
    core0_expect_mgmt_status(endpoint, MgmtStatusCode::Ok, "[core0] mgmt activate status").await;

    let update = match endpoint.recv::<NodeImageUpdatedMsg>().await {
        Ok(update) => update,
        Err(_) => fail_closed("[core0] recv node image update"),
    };
    if update.node_id() != managed_node
        || update.slot() != QEMU_MGMT_IMAGE_SLOT
        || update.image_generation() != QEMU_MGMT_IMAGE_GENERATION
        || !update.accepted()
    {
        fail_closed("[core0] node image update mismatch");
    }
    uart_hex_line(
        "[core0] remote management image updated node 0x",
        managed_node.raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_expect_mgmt_status(
    endpoint: &mut DemoCore0Endpoint,
    expected: MgmtStatusCode,
    context: &str,
) {
    let status = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
        .await
    {
        Ok(status) => status,
        Err(_) => fail_closed(context),
    };
    if status.slot() != QEMU_MGMT_IMAGE_SLOT || status.code() != expected {
        fail_closed(context);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_start_wasip1(endpoint: &mut DemoCore0Endpoint, sensor_node: NodeId) {
    let command = RemoteActuateRequest::new(0, 1, sensor_node.raw() as u8, WASI_START_VALUE);
    match endpoint
        .flow::<RemoteActuateReqMsg>()
        .expect("core0 flow<wasip1 start>")
        .send(&command)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send wasip1 start"),
    }
    let ack = match endpoint.recv::<RemoteActuateRetMsg>().await {
        Ok(ack) => ack,
        Err(_) => fail_closed("[core0] recv wasip1 start ack"),
    };
    if ack.channel() != sensor_node.raw() as u8 || ack.result() != 0 {
        fail_closed("[core0] wasip1 start ack mismatch");
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core0_wasip1_fd_write(endpoint: &mut DemoCore0Endpoint, sensor_node: NodeId) {
    let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(WASI_MEMORY_LEN, WASI_MEMORY_EPOCH);

    uart_hex_line(
        "[core0] wait wasip1 fd_write guest node 0x",
        sensor_node.raw() as u32,
    );
    let borrow = match endpoint
        .recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .await
    {
        Ok(borrow) => borrow,
        Err(_) => fail_closed("[core0] recv wasip1 mem borrow"),
    };
    if borrow.ptr() != WASI_STDOUT_PTR
        || borrow.len() as usize != WASIP1_SENSOR_STDOUT_MARKER.len()
        || borrow.epoch() != WASI_MEMORY_EPOCH
    {
        fail_closed("[core0] wasip1 mem borrow mismatch");
    }
    let grant = leases
        .grant_read(borrow)
        .unwrap_or_else(|_| fail_closed("[core0] grant wasip1 read lease"));
    if grant.rights() != MemRights::Read {
        fail_closed("[core0] wasip1 grant rights mismatch");
    }
    match endpoint
        .flow::<MemReadGrantControl>()
        .expect("coordinator flow<wasip1 read grant>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send wasip1 read grant"),
    }

    let request = match endpoint.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>().await {
        Ok(request) => request,
        Err(_) => fail_closed("[core0] recv wasip1 fd_write"),
    };
    let write = match request {
        EngineReq::FdWrite(write) => write,
        EngineReq::LogU32(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::Yield => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1Stdout(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1Stderr(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1Stdin(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1ClockNow => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1RandomSeed => fail_closed("[core0] expected fd_write"),
        EngineReq::Wasip1Exit(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::TimerSleepUntil(_) => fail_closed("[core0] expected fd_write"),
        EngineReq::GpioSet(_) => fail_closed("[core0] expected fd_write"),
        _ => fail_closed("unexpected wasi p1 request"),
    };
    if write.fd() != WASI_STDOUT_FD
        || write.lease_id() != grant.lease_id()
        || write.len() as u8 > grant.len()
        || write.as_bytes() != WASIP1_SENSOR_STDOUT_MARKER
    {
        fail_closed("[core0] wasip1 fd_write mismatch");
    }
    uart_lock();
    uart_puts("[core0] wasip1 fd_write node 0x");
    uart_hex(sensor_node.raw() as u32);
    uart_puts(": ");
    uart_bytes(write.as_bytes());
    uart_unlock();

    let reply = EngineRet::FdWriteDone(FdWriteDone::new(write.fd(), write.len() as u8));
    match endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("coordinator flow<wasip1 fd_write ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core0] send wasip1 fd_write ret"),
    }

    let release = match endpoint.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>().await {
        Ok(release) => release,
        Err(_) => fail_closed("[core0] recv wasip1 mem release"),
    };
    if release.lease_id() != write.lease_id() {
        fail_closed("[core0] wasip1 release lease mismatch");
    }
    leases
        .release(release)
        .unwrap_or_else(|_| fail_closed("[core0] release wasip1 read lease"));
    uart_hex_line(
        "[core0] wasip1 guest exchange done node 0x",
        sensor_node.raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_session<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
    node_count: u8,
) {
    uart_line("[core1] cyw43439 wait remote sample");
    let request = match endpoint.recv::<RemoteSampleReqMsg>().await {
        Ok(request) => request,
        Err(_) => fail_closed("[core1] recv remote sample"),
    };
    uart_hex_line("[core1] sensor id 0x", request.sensor_id() as u32);
    if request.sensor_id() != local_node.raw() as u8 {
        fail_closed("[core1] sample request sensor mismatch");
    }

    let value = sample_value_for(local_node);
    let sample = RemoteSample::new(request.sensor_id(), 0, value, 2350);
    match endpoint
        .flow::<RemoteSampleRetMsg>()
        .expect("core1 flow<remote sample ret>")
        .send(&sample)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send remote sample ret"),
    }
    uart_hex_line("[core1] sent sample 0x", value);

    let start = match endpoint.recv::<RemoteActuateReqMsg>().await {
        Ok(start) => start,
        Err(_) => fail_closed("[core1] recv wasip1 start"),
    };
    if start.fd() != 0
        || start.generation() != 1
        || start.channel() != local_node.raw() as u8
        || start.value() != WASI_START_VALUE
    {
        fail_closed("[core1] wasip1 start mismatch");
    }
    let ack = RemoteActuateAck::new(local_node.raw() as u8, 0);
    match endpoint
        .flow::<RemoteActuateRetMsg>()
        .expect("core1 flow<wasip1 start ack>")
        .send(&ack)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send wasip1 start ack"),
    }

    core1_wasip1_fd_write(endpoint, local_node).await;

    let aggregate = match endpoint.recv::<RemoteActuateReqMsg>().await {
        Ok(aggregate) => aggregate,
        Err(_) => fail_closed("[core1] recv aggregate command"),
    };
    if aggregate.fd() != 2
        || aggregate.generation() != 1
        || aggregate.channel() != local_node.raw() as u8
        || aggregate.value() != expected_swarm_sum(node_count)
    {
        fail_closed("[core1] aggregate command mismatch");
    }
    uart_hex_line("[core1] aggregate accepted 0x", aggregate.value());
    let ack = RemoteActuateAck::new(local_node.raw() as u8, 0);
    match endpoint
        .flow::<RemoteActuateRetMsg>()
        .expect("core1 flow<aggregate ack>")
        .send(&ack)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send aggregate ack"),
    }

    if node_count == QEMU_CYW43439_MAX_ROLES as u8 {
        match local_node.raw() {
            3 => {
                core1_remote_actuator(endpoint, local_node).await;
                core1_send_gateway_telemetry(endpoint, local_node).await;
            }
            4 => {
                core1_recv_gateway_telemetry(endpoint).await;
                core1_network_object(endpoint, local_node).await;
            }
            5 => {
                core1_remote_management(endpoint, local_node).await;
            }
            _ => {}
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_remote_actuator<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
) {
    let command = match endpoint.recv::<RemoteActuateReqMsg>().await {
        Ok(command) => command,
        Err(_) => fail_closed("[core1] recv remote actuator"),
    };
    if command.fd() != REMOTE_ACTUATOR_FD
        || command.generation() != SESSION_GENERATION
        || command.channel() != local_node.raw() as u8
    {
        fail_closed("[core1] remote actuator command mismatch");
    }
    uart_hex_line("[core1] actuator set value 0x", command.value());
    let ack = RemoteActuateAck::new(local_node.raw() as u8, 0);
    match endpoint
        .flow::<RemoteActuateRetMsg>()
        .expect("core1 flow<remote actuator ack>")
        .send(&ack)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send remote actuator ack"),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_send_gateway_telemetry<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
) {
    let telemetry = SwarmTelemetry::new(
        local_node,
        RoleMask::single(NodeRole::Actuator),
        1,
        0,
        512,
        23_500,
        SESSION_GENERATION,
    );
    match endpoint
        .flow::<SwarmTelemetryMsg>()
        .expect("core1 flow<gateway telemetry>")
        .send(&telemetry)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send gateway telemetry"),
    }
    uart_hex_line(
        "[core1] gateway telemetry sent node 0x",
        local_node.raw() as u32,
    );
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_recv_gateway_telemetry<const ROLE: u8>(endpoint: &mut Endpoint<'static, ROLE>) {
    let telemetry = match endpoint.recv::<SwarmTelemetryMsg>().await {
        Ok(telemetry) => telemetry,
        Err(_) => fail_closed("[core1] recv gateway telemetry"),
    };
    if telemetry.node_id() != NodeId::new(3)
        || !telemetry.role_mask().contains(NodeRole::Actuator)
        || telemetry.session_generation() != SESSION_GENERATION
    {
        fail_closed("[core1] gateway telemetry mismatch");
    }
    uart_hex_line(
        "[core1] gateway telemetry accepted node 0x",
        telemetry.node_id().raw() as u32,
    );
    match endpoint
        .flow::<SwarmTelemetryMsg>()
        .expect("core1 flow<gateway telemetry acceptance>")
        .send(&telemetry)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send gateway telemetry acceptance"),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_network_object<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
) {
    let mut network_objects: NetworkObjectTable<2> = NetworkObjectTable::new();
    network_objects
        .apply_cap_grant_datagram(
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            COORDINATOR,
            22,
            LABEL_NET_DATAGRAM_SEND,
            NetworkRights::Receive,
        )
        .unwrap_or_else(|_| fail_closed("[core1] grant datagram network object"));
    network_objects
        .apply_cap_grant_stream(
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            COORDINATOR,
            23,
            LABEL_NET_STREAM_WRITE,
            NetworkRights::Receive,
        )
        .unwrap_or_else(|_| fail_closed("[core1] grant stream network object"));

    let datagram_branch = match endpoint.offer().await {
        Ok(branch) => branch,
        Err(_) => fail_closed("[core1] offer datagram route"),
    };
    let datagram = match datagram_branch.decode::<DatagramSendMsg>().await {
        Ok(datagram) => datagram,
        Err(_) => fail_closed("[core1] recv datagram fd"),
    };
    let datagram_route = match network_objects.route_receive_routed(NetworkRoute::new(
        COORDINATOR,
        22,
        LABEL_NET_DATAGRAM_SEND,
        SESSION_GENERATION,
    )) {
        NetworkObjectReadRoute::Datagram(fd) => fd,
        NetworkObjectReadRoute::Stream(_) => fail_closed("[core1] datagram selected stream route"),
        NetworkObjectReadRoute::Rejected(_) => fail_closed("[core1] datagram route rejected"),
    };
    expect_qemu_rx_meta(
        ROLE,
        local_node,
        datagram_route.target_node(),
        datagram_route.lane(),
        "[core1] datagram source",
    );
    if datagram.generation() == 0
        || datagram.route() != datagram_route.route()
        || datagram.payload() != b"qemu datagram fd"
    {
        fail_closed("[core1] datagram fd mismatch");
    }
    let ack = DatagramAck::new(datagram.fd(), datagram.generation(), true);
    match endpoint
        .flow::<DatagramAckMsg>()
        .expect("core1 flow<datagram ack>")
        .send(&ack)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send datagram ack"),
    }
    uart_line("[core1] network datagram fd accepted");

    let stream = match endpoint.recv::<StreamWriteMsg>().await {
        Ok(stream) => stream,
        Err(_) => fail_closed("[core1] recv stream fd"),
    };
    let stream_route = match network_objects.route_receive_routed(NetworkRoute::new(
        COORDINATOR,
        23,
        LABEL_NET_STREAM_WRITE,
        SESSION_GENERATION,
    )) {
        NetworkObjectReadRoute::Stream(fd) => fd,
        NetworkObjectReadRoute::Datagram(_) => {
            fail_closed("[core1] stream selected datagram route")
        }
        NetworkObjectReadRoute::Rejected(_) => fail_closed("[core1] stream route rejected"),
    };
    expect_qemu_rx_meta(
        ROLE,
        local_node,
        stream_route.target_node(),
        stream_route.lane(),
        "[core1] stream source",
    );
    if stream.generation() == 0
        || stream.route() != stream_route.route()
        || !stream.is_fin()
        || stream.payload() != b"qemu stream fd"
    {
        fail_closed("[core1] stream fd mismatch");
    }
    let ack = StreamAck::new(stream.fd(), stream.generation(), stream.sequence(), true);
    match endpoint
        .flow::<StreamAckMsg>()
        .expect("core1 flow<stream ack>")
        .send(&ack)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send stream ack"),
    }
    uart_line("[core1] network stream fd accepted");
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_remote_management<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
) {
    let mut images: ImageSlotTable<2, 64> = ImageSlotTable::new();
    let grant = MgmtControl::install_grant(
        local_node,
        SWARM_CREDENTIAL,
        SESSION_GENERATION,
        QEMU_MGMT_IMAGE_SLOT,
        QEMU_MGMT_IMAGE_GENERATION,
    );

    let begin = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
        .await
    {
        Ok(begin) => begin,
        Err(_) => fail_closed("[core1] recv mgmt image begin"),
    };
    let status = images
        .begin_with_control(
            grant,
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            begin,
        )
        .unwrap_or_else(|error| error.status(begin.slot()));
    core1_send_mgmt_status(endpoint, status, "[core1] send mgmt begin status").await;

    let chunk = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
        .await
    {
        Ok(chunk) => chunk,
        Err(_) => fail_closed("[core1] recv mgmt image chunk"),
    };
    let status = images
        .chunk_with_control(
            grant,
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            chunk,
        )
        .unwrap_or_else(|error| error.status(chunk.slot()));
    core1_send_mgmt_status(endpoint, status, "[core1] send mgmt chunk status").await;

    let end = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
        .await
    {
        Ok(end) => end,
        Err(_) => fail_closed("[core1] recv mgmt image end"),
    };
    let status = images
        .end_with_control(grant, local_node, SWARM_CREDENTIAL, SESSION_GENERATION, end)
        .unwrap_or_else(|error| error.status(end.slot()));
    core1_send_mgmt_status(endpoint, status, "[core1] send mgmt end status").await;

    let activate = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
        .await
    {
        Ok(activate) => activate,
        Err(_) => fail_closed("[core1] recv mgmt activate need fence"),
    };
    let status = images
        .activate_with_control(
            grant,
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            activate,
            ActivationBoundary::new(false, true, true, QEMU_MGMT_FENCE_EPOCH),
        )
        .unwrap_or_else(|error| error.status(activate.slot()));
    if status.code() != MgmtStatusCode::NeedFence {
        fail_closed("[core1] mgmt need-fence mismatch");
    }
    core1_send_mgmt_status(endpoint, status, "[core1] send mgmt need-fence status").await;

    let activate = match endpoint
        .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
        .await
    {
        Ok(activate) => activate,
        Err(_) => fail_closed("[core1] recv mgmt activate"),
    };
    let status = images
        .activate_with_control(
            grant,
            local_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            activate,
            ActivationBoundary::new(true, true, true, QEMU_MGMT_FENCE_EPOCH),
        )
        .unwrap_or_else(|error| error.status(activate.slot()));
    if status.code() != MgmtStatusCode::Ok || images.active_slot() != Some(QEMU_MGMT_IMAGE_SLOT) {
        fail_closed("[core1] mgmt activate mismatch");
    }
    core1_send_mgmt_status(endpoint, status, "[core1] send mgmt activate status").await;

    let update = NodeImageUpdated::new(
        local_node,
        QEMU_MGMT_IMAGE_SLOT,
        QEMU_MGMT_IMAGE_GENERATION,
        true,
    );
    match endpoint
        .flow::<NodeImageUpdatedMsg>()
        .expect("core1 flow<node image updated>")
        .send(&update)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send node image update"),
    }
    uart_line("[core1] remote management image activated");
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_send_mgmt_status<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    status: MgmtStatus,
    context: &str,
) {
    match endpoint
        .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
        .expect("core1 flow<mgmt status>")
        .send(&status)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed(context),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn core1_wasip1_fd_write<const ROLE: u8>(
    endpoint: &mut Endpoint<'static, ROLE>,
    local_node: NodeId,
) {
    let module = Wasip1StdoutModule::parse(WASIP1_SENSOR_GUEST)
        .unwrap_or_else(|_| fail_closed("[core1] parse wasip1 stdout guest"));
    let chunk = module
        .stdout_chunk_for(WASIP1_SENSOR_STDOUT_MARKER)
        .unwrap_or_else(|_| fail_closed("[core1] make wasip1 stdout chunk"));
    if chunk.as_bytes() != WASIP1_SENSOR_STDOUT_MARKER {
        fail_closed("[core1] wasip1 stdout marker mismatch");
    }

    uart_hex_line(
        "[core1] wasip1 guest fd_write node 0x",
        local_node.raw() as u32,
    );
    let borrow = MemBorrow::new(WASI_STDOUT_PTR, chunk.len() as u8, WASI_MEMORY_EPOCH);
    match endpoint
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("sensor flow<wasip1 mem borrow read>")
        .send(&borrow)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send wasip1 mem borrow"),
    }

    let grant = match endpoint.recv::<MemReadGrantControl>().await {
        Ok(grant) => grant,
        Err(_) => fail_closed("[core1] recv wasip1 read grant"),
    };
    let (rights, lease_id) = grant
        .decode_handle()
        .unwrap_or_else(|_| fail_closed("[core1] decode wasip1 read grant"));
    if rights != MemRights::Read.tag() || lease_id > u8::MAX as u64 {
        fail_closed("[core1] wasip1 read grant mismatch");
    }
    let lease_id = lease_id as u8;
    let write = FdWrite::new_with_lease(WASI_STDOUT_FD, lease_id, chunk.as_bytes())
        .unwrap_or_else(|_| fail_closed("[core1] make wasip1 fd_write"));
    let request = EngineReq::FdWrite(write);
    match endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .expect("sensor flow<wasip1 fd_write>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send wasip1 fd_write"),
    }

    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => fail_closed("[core1] recv wasip1 fd_write ret"),
    };
    match reply {
        EngineRet::FdWriteDone(done) => {
            if done.fd() != WASI_STDOUT_FD || done.written() != chunk.len() as u8 {
                fail_closed("[core1] wasip1 fd_write ret mismatch");
            }
        }
        EngineRet::Logged(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Yielded => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Wasip1StdoutWritten(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Wasip1StderrWritten(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Wasip1StdinRead(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Wasip1ClockNow(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::Wasip1RandomSeed(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::TimerSleepDone(_) => fail_closed("[core1] expected fd_write ret"),
        EngineRet::GpioSetDone(_) => fail_closed("[core1] expected fd_write ret"),
        _ => fail_closed("unexpected wasi p1 reply"),
    }

    let release = MemRelease::new(lease_id);
    match endpoint
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("sensor flow<wasip1 mem release>")
        .send(&release)
        .await
    {
        Ok(_) => {}
        Err(_) => fail_closed("[core1] send wasip1 mem release"),
    }
    uart_line("[core1] wasip1 guest fd_write done");
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core0_main() -> ! {
    uart_init();
    uart_line("[core0] hibana pico2w cyw43439 swarm");
    uart_line("[core0] init rp2350 + cyw43439 runtime");
    let (role, local_node, node_count) = init_radio_once();
    if role == cyw43439::NODE_ROLE_COORDINATOR {
        install_runtime_session(role, local_node, node_count);
        publish_runtime_ready();
        let endpoint = unsafe { shared_core0_endpoint() };
        run_current_task(core0_session(endpoint, node_count));
        uart_hex_line(
            "[core0] completed sensors 0x",
            node_count.saturating_sub(1) as u32,
        );
        unsafe {
            HIBANA_DEMO_RESULT = RESULT_SUCCESS;
        }
        uart_line("[core0] hibana pico2w cyw43439 swarm ok");
    } else if role == cyw43439::NODE_ROLE_SENSOR {
        install_runtime_session(role, local_node, node_count);
        publish_runtime_ready();
    } else if role == cyw43439::NODE_ROLE_DUAL_CORE {
        install_runtime_session(role, local_node, node_count);
        publish_runtime_ready();
        let endpoint = unsafe { shared_core0_endpoint() };
        run_current_task(core0_session(endpoint, node_count));
        unsafe {
            HIBANA_DEMO_RESULT = RESULT_SUCCESS;
        }
        uart_line("[core0] hibana pico2w cyw43439 swarm ok");
    }
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn core1_main() -> ! {
    wait_until(|| unsafe { read_volatile(core::ptr::addr_of!(UART_READY)) } != 0);
    wait_until(|| unsafe { read_volatile(core::ptr::addr_of!(RUNTIME_READY)) } != 0);
    let role = unsafe { read_volatile(core::ptr::addr_of!(NODE_ROLE)) as u8 };
    let local_node = unsafe { NodeId::new(read_volatile(core::ptr::addr_of!(NODE_ID)) as u16) };
    let node_count = unsafe { read_volatile(core::ptr::addr_of!(NODE_COUNT)) as u8 };
    if role == cyw43439::NODE_ROLE_SENSOR {
        let sensor_role =
            fixed_sensor_hibana_role().unwrap_or_else(|| local_hibana_role(local_node, node_count));
        if local_hibana_role(local_node, node_count) != sensor_role {
            fail_closed("[core1] fixed sensor role");
        }
        match sensor_role {
            1 => run_current_task(core1_session(
                unsafe { shared_core1_endpoint() },
                local_node,
                node_count,
            )),
            2 => run_current_task(core1_session(
                unsafe { shared_core2_endpoint() },
                local_node,
                node_count,
            )),
            3 => run_current_task(core1_session(
                unsafe { shared_core3_endpoint() },
                local_node,
                node_count,
            )),
            4 => run_current_task(core1_session(
                unsafe { shared_core4_endpoint() },
                local_node,
                node_count,
            )),
            5 => run_current_task(core1_session(
                unsafe { shared_core5_endpoint() },
                local_node,
                node_count,
            )),
            _ => fail_closed("[core1] sensor role"),
        }
    } else if role == cyw43439::NODE_ROLE_DUAL_CORE {
        let endpoint = unsafe { shared_core1_endpoint() };
        run_current_task(core1_session(endpoint, local_node, node_count));
    }
    park();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail_closed("[panic]")
}
