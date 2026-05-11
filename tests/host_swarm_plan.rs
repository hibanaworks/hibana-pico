use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use hibana::{
    Endpoint, RecvError, g,
    g::{Msg, Role, send},
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        program::{RoleProgram, project},
        runtime::{Config, CounterClock, LabelUniverse},
        tap::TapEvent,
        wire::{Payload, WireEncode, WirePayload},
    },
};
#[cfg(feature = "profile-host-qemu-swarm")]
use hibana_pico::choreography::swarm::{
    coordinator_program_6, role1_program_6, role2_program_6, role3_program_6, role4_program_6,
    role5_program_6,
};
use hibana_pico::{
    choreography::protocol::{
        EngineAbortRouteControl, EngineLabelUniverse, EngineNormalRouteControl, EngineReq,
        EngineRet, FdError, FdErrorMsg, FdRead, FdReadDone, FdWrite, FdWriteDone, GpioWait,
        LABEL_MEM_BORROW_READ, LABEL_MEM_FENCE, LABEL_MEM_RELEASE, LABEL_MGMT_IMAGE_ACTIVATE,
        LABEL_MGMT_IMAGE_BEGIN, LABEL_MGMT_IMAGE_CHUNK, LABEL_MGMT_IMAGE_END,
        LABEL_MGMT_IMAGE_STATUS, LABEL_NET_DATAGRAM_RECV, LABEL_NET_DATAGRAM_SEND,
        LABEL_NET_STREAM_WRITE, LABEL_REMOTE_ACTUATE_REQ, LABEL_REMOTE_SAMPLE_REQ,
        LABEL_SWARM_TELEMETRY, LABEL_WASI_FD_READ, LABEL_WASI_FD_READ_RET, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, LABEL_WASI_PROC_EXIT, MEM_LEASE_NONE, MGMT_IMAGE_CHUNK_CAPACITY,
        MemBorrow, MemFence, MemFenceReason, MemReadGrantControl, MemRelease, MemRights,
        MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd, MgmtStatus,
        MgmtStatusCode, NetworkDatagramRecvRouteControl, NetworkDatagramSendRouteControl,
        NetworkRejectRouteControl, NetworkStreamReadRouteControl, NetworkStreamWriteRouteControl,
        PublishAlertControl, PublishNormalControl, RemoteActuatorRouteControl,
        RemoteManagementRouteControl, RemoteRejectRouteControl, RemoteSensorRouteControl,
        RemoteTelemetryRouteControl, StateRestoreControl, StateSnapshotControl, TopologyAckControl,
        TopologyBeginControl, TopologyCommitControl, TxAbortControl, TxCommitControl,
    },
    kernel::app::{AppId, AppLeaseTable, AppScopeError, AppStreamTable},
    kernel::metrics::{PICO2W_SWARM_DEFAULT_AGGREGATE, pico2w_swarm_sample_value},
    kernel::mgmt::{
        ActivationBoundary, ImageSlotError, ImageSlotTable, ImageTransferPlan, MgmtControl,
    },
    kernel::network::{
        DatagramAck, DatagramAckMsg, DatagramRecv, DatagramRecvMsg, DatagramRecvRet,
        DatagramRecvRetMsg, DatagramSend, DatagramSendMsg, NET_DATAGRAM_PAYLOAD_CAPACITY,
        NET_STREAM_FLAG_FIN, NetworkError, NetworkObjectReadRoute, NetworkObjectTable,
        NetworkObjectWriteRoute, NetworkRights, NetworkRoleProtocol, NetworkRoute, StreamAck,
        StreamAckMsg, StreamRead, StreamReadMsg, StreamReadRet, StreamReadRetMsg, StreamWrite,
        StreamWriteMsg,
    },
    kernel::policy::{
        AppChoice, AppInstance, JoinAck, JoinAckMsg, JoinGrant, JoinGrantMsg, JoinOffer,
        JoinOfferMsg, JoinRequest, JoinRequestMsg, LeaveAck, LeaveAckMsg, MultiAppPolicyState,
        NodeImageUpdated, NodeImageUpdatedMsg, NodeRevoked, NodeRevokedMsg, NodeRole,
        PolicyApp0Msg, PolicyApp1Msg, PolicyError, PolicySlotTable, RemoteObjectsRevoke,
        RemoteObjectsRevokeMsg, RoleMask, SwarmSuspend, SwarmSuspendMsg, SwarmTelemetry,
        SwarmTelemetryMsg,
    },
    kernel::remote::{
        RemoteActuateAck, RemoteActuateReqMsg, RemoteActuateRequest, RemoteActuateRetMsg,
        RemoteControl, RemoteError, RemoteFdReadRoute, RemoteFdWriteRoute, RemoteObjectTable,
        RemoteResource, RemoteRights, RemoteRoute, RemoteSample, RemoteSampleReqMsg,
        RemoteSampleRequest, RemoteSampleRetMsg,
    },
    kernel::resolver::{InterruptEvent, PicoInterruptResolver, ResolvedInterrupt},
    kernel::swarm::{
        BleProvisioningBundle, HostSwarmMedium, HostSwarmRoleTransport, HostSwarmTransport,
        NeighborEntry, NeighborTable, NodeId, ProvisioningRecord, ReplayWindow,
        SWARM_FRAGMENT_CHUNK_CAPACITY, SWARM_FRAGMENT_HEADER_LEN, SWARM_FRAME_MAX_WIRE_LEN,
        SWARM_FRAME_PAYLOAD_CAPACITY, SwarmCredential, SwarmError, SwarmFragment, SwarmFrame,
        SwarmReassemblyBuffer, SwarmSecurity,
    },
    kernel::wasi::{MemoryLeaseTable, Wasip1StdoutModule},
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
    projects::baker_link_led::choreography::{
        BakerTrafficLoopBreakControl, BakerTrafficLoopContinueControl,
        POLICY_BAKER_ENGINE_ABORT_ROUTE, POLICY_BAKER_TRAFFIC_LOOP,
    },
};

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
type SwarmTestTransport<'a> = HostSwarmTransport<'a, 8>;
type SwarmTestKit<'a> =
    SessionKit<'a, SwarmTestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
type SwarmPingPongKit<'a> =
    SessionKit<'a, SwarmTestTransport<'a>, SwarmPingPongLabelUniverse, CounterClock, 1>;
type SwarmRoleTestTransport<'a> = HostSwarmRoleTransport<'a, 64, 4>;
type SwarmRoleTestKit<'a> =
    SessionKit<'a, SwarmRoleTestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;
type SwarmSixRoleTestTransport<'a> = HostSwarmRoleTransport<'a, 192, 6>;
type SwarmSixRoleTestKit<'a> =
    SessionKit<'a, SwarmSixRoleTestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;

const COORDINATOR: NodeId = NodeId::new(1);
const SENSOR: NodeId = NodeId::new(2);
const ACTUATOR: NodeId = NodeId::new(3);
const GATEWAY: NodeId = NodeId::new(4);
const SESSION_GENERATION: u16 = 7;
const SWARM_CREDENTIAL: SwarmCredential = SwarmCredential::new(0x4849_4241);
const SECURE: SwarmSecurity = SwarmSecurity::Secure(SWARM_CREDENTIAL);
#[cfg(feature = "profile-host-qemu-swarm")]
const QEMU_REMOTE_ACTUATOR_FD: u8 = 21;
const TEST_MEMORY_LEN: u32 = 4096;
const TEST_MEMORY_EPOCH: u32 = 1;
const TEST_STDOUT_PTR: u32 = 1024;
const TEST_STDOUT_TEXT: &[u8] = b"hibana wasip1 stdout\n";
const TEST_STDOUT_FD: u8 = 1;
const TEST_WASI_START_VALUE: u32 = 0x5741_5349;
const LABEL_SWARM_PING: u8 = 1;
const LABEL_SWARM_PONG: u8 = 2;
const SWARM_PING_VALUE: u8 = 0x2a;
const SWARM_PONG_VALUE: u8 = 0x55;
static WASIP1_STDOUT_GUEST: &[u8] =
    b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write environ_get hibana wasip1 stdout\n";

#[derive(Clone, Copy, Debug, Default)]
struct SwarmPingPongLabelUniverse;

impl LabelUniverse for SwarmPingPongLabelUniverse {
    const MAX_LABEL: u8 = LABEL_SWARM_PONG;
}

fn test_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
        test_raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    RawWaker::new(
        core::ptr::null(),
        &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
    )
}

fn poll_once<F: Future>(future: &mut F) -> Poll<F::Output> {
    let waker = unsafe { Waker::from_raw(test_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(future) };
    future.as_mut().poll(&mut cx)
}

macro_rules! seq_chain {
    ($head:expr, $($tail:expr),+ $(,)?) => {
        g::seq($head, seq_chain!($($tail),+))
    };
    ($last:expr $(,)?) => {
        $last
    };
}

macro_rules! swarm_sensor_exchange {
    ($role:literal) => {
        seq_chain!(
            send::<Role<0>, Role<$role>, RemoteSampleReqMsg, 0>(),
            send::<Role<$role>, Role<0>, RemoteSampleRetMsg, 0>(),
        )
    };
}

macro_rules! swarm_wasip1_exchange {
    ($role:literal) => {
        seq_chain!(
            send::<Role<$role>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
            send::<Role<0>, Role<$role>, MemReadGrantControl, 1>(),
            send::<Role<$role>, Role<0>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
            send::<Role<0>, Role<$role>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
            send::<Role<$role>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        )
    };
}

macro_rules! swarm_wasip1_start_exchange {
    ($role:literal) => {
        seq_chain!(
            send::<Role<0>, Role<$role>, RemoteActuateReqMsg, 1>(),
            send::<Role<$role>, Role<0>, RemoteActuateRetMsg, 1>(),
        )
    };
}

macro_rules! swarm_aggregate_exchange {
    ($role:literal) => {
        seq_chain!(
            send::<Role<0>, Role<$role>, RemoteActuateReqMsg, 1>(),
            send::<Role<$role>, Role<0>, RemoteActuateRetMsg, 1>(),
        )
    };
}

fn project_sample_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<0>, Role<1>, RemoteSampleReqMsg, 0>(),
        send::<Role<1>, Role<0>, RemoteSampleRetMsg, 0>(),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let sensor: RoleProgram<1> = project(&program);
    (coordinator, sensor)
}

fn project_swarm_ping_pong_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<1>, Role<0>, Msg<LABEL_SWARM_PING, u8>, 0>(),
        send::<Role<0>, Role<1>, Msg<LABEL_SWARM_PONG, u8>, 0>(),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let sensor: RoleProgram<1> = project(&program);
    (coordinator, sensor)
}

fn project_swarm_wasip1_fd_write_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<1>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
        g::seq(
            send::<Role<0>, Role<1>, MemReadGrantControl, 1>(),
            g::seq(
                send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
                g::seq(
                    send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
                    send::<Role<1>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
                ),
            ),
        ),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let sensor: RoleProgram<1> = project(&program);
    (coordinator, sensor)
}

fn project_actuator_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<0>, Role<1>, RemoteActuateReqMsg, 0>(),
        send::<Role<1>, Role<0>, RemoteActuateRetMsg, 0>(),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let actuator: RoleProgram<1> = project(&program);
    (coordinator, actuator)
}

fn project_remote_fd_read_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let read_remote_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteSensorRouteControl, 17>(),
        send::<Role<1>, Role<2>, RemoteSampleReqMsg, 17>(),
        send::<Role<2>, Role<1>, RemoteSampleRetMsg, 17>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 17>(),
    );
    let read_other_remote_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteActuatorRouteControl, 17>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 17>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 17>(),
    );
    let read_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_READ, EngineReq>, 17>(),
        g::route(read_remote_arm, read_other_remote_arm),
    );
    let engine: RoleProgram<0> = project(&read_then_route);
    let kernel: RoleProgram<1> = project(&read_then_route);
    let remote: RoleProgram<2> = project(&read_then_route);
    (engine, kernel, remote)
}

fn project_remote_fd_write_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let write_remote_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteActuatorRouteControl, 17>(),
        send::<Role<1>, Role<2>, RemoteActuateReqMsg, 17>(),
        send::<Role<2>, Role<1>, RemoteActuateRetMsg, 17>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 17>(),
    );
    let write_other_remote_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteSensorRouteControl, 17>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 17>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 17>(),
    );
    let write_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 17>(),
        g::route(write_remote_arm, write_other_remote_arm),
    );
    let engine: RoleProgram<0> = project(&write_then_route);
    let kernel: RoleProgram<1> = project(&write_then_route);
    let remote: RoleProgram<2> = project(&write_then_route);
    (engine, kernel, remote)
}

fn project_remote_management_fd_write_route_roles()
-> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let management_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteManagementRouteControl, 19>(),
        send::<Role<1>, Role<2>, Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>, 19>(),
        send::<Role<2>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 19>(),
    );
    let reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteRejectRouteControl, 19>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 19>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 19>(),
    );
    let write_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 19>(),
        g::route(management_arm, reject_arm),
    );
    let engine: RoleProgram<0> = project(&write_then_route);
    let kernel: RoleProgram<1> = project(&write_then_route);
    let management: RoleProgram<2> = project(&write_then_route);
    (engine, kernel, management)
}

fn project_remote_telemetry_fd_write_route_roles()
-> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let telemetry_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteTelemetryRouteControl, 20>(),
        send::<Role<1>, Role<2>, SwarmTelemetryMsg, 20>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 20>(),
    );
    let reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteRejectRouteControl, 20>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 20>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 20>(),
    );
    let write_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 20>(),
        g::route(telemetry_arm, reject_arm),
    );
    let engine: RoleProgram<0> = project(&write_then_route);
    let kernel: RoleProgram<1> = project(&write_then_route);
    let gateway: RoleProgram<2> = project(&write_then_route);
    (engine, kernel, gateway)
}

fn project_remote_fd_reject_route_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let local_ok_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteSensorRouteControl, 17>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 17>(),
    );
    let reject_read_arm = seq_chain!(
        send::<Role<1>, Role<1>, RemoteRejectRouteControl, 17>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 17>(),
    );
    let rejected_read_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_READ, EngineReq>, 17>(),
        g::route(local_ok_arm, reject_read_arm),
    );
    let engine: RoleProgram<0> = project(&rejected_read_then_route);
    let kernel: RoleProgram<1> = project(&rejected_read_then_route);
    (engine, kernel)
}

fn project_network_object_write_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let write_datagram_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkDatagramSendRouteControl, 22>(),
        send::<Role<1>, Role<2>, DatagramSendMsg, 22>(),
        send::<Role<2>, Role<1>, DatagramAckMsg, 22>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 22>(),
    );
    let write_reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkRejectRouteControl, 22>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 22>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 22>(),
    );
    let write_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 22>(),
        g::route(write_datagram_arm, write_reject_arm),
    );
    let engine: RoleProgram<0> = project(&write_then_route);
    let kernel: RoleProgram<1> = project(&write_then_route);
    let network: RoleProgram<2> = project(&write_then_route);
    (engine, kernel, network)
}

fn project_network_object_read_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let read_datagram_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkDatagramRecvRouteControl, 22>(),
        send::<Role<1>, Role<2>, DatagramRecvMsg, 22>(),
        send::<Role<2>, Role<1>, DatagramRecvRetMsg, 22>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 22>(),
    );
    let read_reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkRejectRouteControl, 22>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 22>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 22>(),
    );
    let read_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_READ, EngineReq>, 22>(),
        g::route(read_datagram_arm, read_reject_arm),
    );
    let engine: RoleProgram<0> = project(&read_then_route);
    let kernel: RoleProgram<1> = project(&read_then_route);
    let network: RoleProgram<2> = project(&read_then_route);
    (engine, kernel, network)
}

fn project_network_stream_fd_write_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>)
{
    let write_stream_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkStreamWriteRouteControl, 23>(),
        send::<Role<1>, Role<2>, StreamWriteMsg, 23>(),
        send::<Role<2>, Role<1>, StreamAckMsg, 23>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 23>(),
    );
    let write_reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkRejectRouteControl, 23>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 23>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 23>(),
    );
    let write_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 23>(),
        g::route(write_stream_arm, write_reject_arm),
    );
    let engine: RoleProgram<0> = project(&write_then_route);
    let kernel: RoleProgram<1> = project(&write_then_route);
    let network: RoleProgram<2> = project(&write_then_route);
    (engine, kernel, network)
}

fn project_network_stream_fd_read_route_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>)
{
    let read_stream_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkStreamReadRouteControl, 23>(),
        send::<Role<1>, Role<2>, StreamReadMsg, 23>(),
        send::<Role<2>, Role<1>, StreamReadRetMsg, 23>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 23>(),
    );
    let read_reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkRejectRouteControl, 23>(),
        send::<Role<1>, Role<2>, FdErrorMsg, 23>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 23>(),
    );
    let read_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_READ, EngineReq>, 23>(),
        g::route(read_stream_arm, read_reject_arm),
    );
    let engine: RoleProgram<0> = project(&read_then_route);
    let kernel: RoleProgram<1> = project(&read_then_route);
    let network: RoleProgram<2> = project(&read_then_route);
    (engine, kernel, network)
}

fn project_network_object_reject_route_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let local_ok_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkDatagramRecvRouteControl, 22>(),
        send::<Role<1>, Role<0>, Msg<LABEL_WASI_FD_READ_RET, EngineRet>, 22>(),
    );
    let reject_arm = seq_chain!(
        send::<Role<1>, Role<1>, NetworkRejectRouteControl, 22>(),
        send::<Role<1>, Role<0>, FdErrorMsg, 22>(),
    );
    let rejected_read_then_route = g::seq(
        send::<Role<0>, Role<1>, Msg<LABEL_WASI_FD_READ, EngineReq>, 22>(),
        g::route(local_ok_arm, reject_arm),
    );
    let engine: RoleProgram<0> = project(&rejected_read_then_route);
    let kernel: RoleProgram<1> = project(&rejected_read_then_route);
    (engine, kernel)
}

fn project_global_swarm_roles_4() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
) {
    let program = seq_chain!(
        swarm_sensor_exchange!(1),
        swarm_sensor_exchange!(2),
        swarm_sensor_exchange!(3),
        swarm_wasip1_start_exchange!(1),
        swarm_wasip1_exchange!(1),
        swarm_wasip1_start_exchange!(2),
        swarm_wasip1_exchange!(2),
        swarm_wasip1_start_exchange!(3),
        swarm_wasip1_exchange!(3),
        swarm_aggregate_exchange!(1),
        swarm_aggregate_exchange!(2),
        swarm_aggregate_exchange!(3),
    );
    (
        project(&program),
        project(&program),
        project(&program),
        project(&program),
    )
}

fn project_global_swarm_roles_6() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
    RoleProgram<4>,
    RoleProgram<5>,
) {
    let program = seq_chain!(
        swarm_sensor_exchange!(1),
        swarm_sensor_exchange!(2),
        swarm_sensor_exchange!(3),
        swarm_sensor_exchange!(4),
        swarm_sensor_exchange!(5),
        swarm_wasip1_start_exchange!(1),
        swarm_wasip1_exchange!(1),
        swarm_wasip1_start_exchange!(2),
        swarm_wasip1_exchange!(2),
        swarm_wasip1_start_exchange!(3),
        swarm_wasip1_exchange!(3),
        swarm_wasip1_start_exchange!(4),
        swarm_wasip1_exchange!(4),
        swarm_wasip1_start_exchange!(5),
        swarm_wasip1_exchange!(5),
        swarm_aggregate_exchange!(1),
        swarm_aggregate_exchange!(2),
        swarm_aggregate_exchange!(3),
        swarm_aggregate_exchange!(4),
        swarm_aggregate_exchange!(5),
    );
    (
        project(&program),
        project(&program),
        project(&program),
        project(&program),
        project(&program),
        project(&program),
    )
}

fn project_sensor_actuator_gateway_roles() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
) {
    let program = seq_chain!(
        send::<Role<0>, Role<1>, RemoteSampleReqMsg, 0>(),
        send::<Role<1>, Role<0>, RemoteSampleRetMsg, 0>(),
        send::<Role<0>, Role<2>, RemoteActuateReqMsg, 0>(),
        send::<Role<2>, Role<0>, RemoteActuateRetMsg, 0>(),
        send::<Role<0>, Role<3>, SwarmTelemetryMsg, 20>(),
    );
    (
        project(&program),
        project(&program),
        project(&program),
        project(&program),
    )
}

fn project_datagram_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<0>, Role<1>, DatagramSendMsg, 0>(),
        g::seq(
            send::<Role<1>, Role<0>, DatagramAckMsg, 0>(),
            g::seq(
                send::<Role<0>, Role<1>, DatagramRecvMsg, 0>(),
                send::<Role<1>, Role<0>, DatagramRecvRetMsg, 0>(),
            ),
        ),
    );
    let app: RoleProgram<0> = project(&program);
    let network: RoleProgram<1> = project(&program);
    (app, network)
}

fn project_policy_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        send::<Role<1>, Role<0>, SwarmTelemetryMsg, 0>(),
        send::<Role<0>, Role<1>, PolicyApp1Msg, 0>(),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let gateway: RoleProgram<1> = project(&program);
    (coordinator, gateway)
}

fn project_swarm_policy_route_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let app0_arm = g::seq(
        send::<Role<0>, Role<0>, PublishNormalControl, 20>(),
        send::<Role<0>, Role<1>, PolicyApp0Msg, 20>(),
    );
    let app1_arm = g::seq(
        send::<Role<0>, Role<0>, PublishAlertControl, 20>(),
        send::<Role<0>, Role<1>, PolicyApp1Msg, 20>(),
    );
    let program = g::seq(
        send::<Role<1>, Role<0>, SwarmTelemetryMsg, 20>(),
        g::route(app0_arm, app1_arm),
    );
    let coordinator: RoleProgram<0> = project(&program);
    let gateway: RoleProgram<1> = project(&program);
    (coordinator, gateway)
}

fn project_swarm_join_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = seq_chain!(
        send::<Role<0>, Role<1>, JoinOfferMsg, 16>(),
        send::<Role<1>, Role<0>, JoinRequestMsg, 16>(),
        send::<Role<0>, Role<1>, JoinGrantMsg, 16>(),
        send::<Role<1>, Role<0>, JoinAckMsg, 16>(),
    );
    let gateway: RoleProgram<0> = project(&program);
    let node: RoleProgram<1> = project(&program);
    (gateway, node)
}

fn project_swarm_leave_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let program = seq_chain!(
        send::<Role<0>, Role<1>, SwarmSuspendMsg, 16>(),
        send::<Role<0>, Role<1>, RemoteObjectsRevokeMsg, 16>(),
        send::<Role<0>, Role<2>, NodeRevokedMsg, 16>(),
        send::<Role<1>, Role<0>, LeaveAckMsg, 16>(),
    );
    let gateway: RoleProgram<0> = project(&program);
    let leaving_node: RoleProgram<1> = project(&program);
    let observer: RoleProgram<2> = project(&program);
    (gateway, leaving_node, observer)
}

fn project_remote_management_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = seq_chain!(
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MEM_FENCE, MemFence>, 1>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>, 1>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
    );
    let supervisor: RoleProgram<0> = project(&program);
    let management: RoleProgram<1> = project(&program);
    (supervisor, management)
}

fn project_remote_management_observer_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    let program = seq_chain!(
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<0>, Role<2>, NodeImageUpdatedMsg, 19>(),
    );
    let supervisor: RoleProgram<0> = project(&program);
    let management: RoleProgram<1> = project(&program);
    let observer: RoleProgram<2> = project(&program);
    (supervisor, management, observer)
}

fn project_remote_management_invalid_image_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = seq_chain!(
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
        send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>, 19>(),
        send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 19>(),
    );
    let supervisor: RoleProgram<0> = project(&program);
    let management: RoleProgram<1> = project(&program);
    (supervisor, management)
}

async fn exchange_swarm_wasip1_fd_write<const ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    sensor: &mut Endpoint<'_, ROLE>,
    sensor_node: NodeId,
    chunk_bytes: &[u8],
) {
    let start = RemoteActuateRequest::new(0, 1, sensor_node.raw() as u8, TEST_WASI_START_VALUE);
    (coordinator
        .flow::<RemoteActuateReqMsg>()
        .expect("coordinator flow<wasip1 start>")
        .send(&start))
    .await
    .expect("coordinator send wasip1 start");
    assert_eq!(
        (sensor.recv::<RemoteActuateReqMsg>())
            .await
            .expect("sensor recv wasip1 start"),
        start
    );
    let start_ack = RemoteActuateAck::new(sensor_node.raw() as u8, 0);
    (sensor
        .flow::<RemoteActuateRetMsg>()
        .expect("sensor flow<wasip1 start ack>")
        .send(&start_ack))
    .await
    .expect("sensor send wasip1 start ack");
    assert_eq!(
        (coordinator.recv::<RemoteActuateRetMsg>())
            .await
            .expect("coordinator recv wasip1 start ack"),
        start_ack
    );

    let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
    let borrow = MemBorrow::new(TEST_STDOUT_PTR, chunk_bytes.len() as u8, TEST_MEMORY_EPOCH);
    (sensor
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("sensor flow<mem borrow read>")
        .send(&borrow))
    .await
    .expect("sensor send memory borrow over global swarm");
    assert_eq!(
        (coordinator.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
            .await
            .expect("coordinator recv memory borrow"),
        borrow
    );

    let grant = leases.grant_read(borrow).expect("grant read lease");
    (coordinator
        .flow::<MemReadGrantControl>()
        .expect("coordinator flow<read grant>")
        .send(()))
    .await
    .expect("coordinator send read grant over global swarm");
    let received_grant = (sensor.recv::<MemReadGrantControl>())
        .await
        .expect("sensor recv read grant");
    let (rights, lease_id) = received_grant
        .decode_handle()
        .expect("decode read lease token");
    assert_eq!(rights, MemRights::Read.tag());
    assert_eq!(lease_id as u8, grant.lease_id());

    let write = FdWrite::new_with_lease(TEST_STDOUT_FD, lease_id as u8, chunk_bytes)
        .expect("fd_write request");
    let request = EngineReq::FdWrite(write);
    (sensor
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .expect("sensor flow<fd_write>")
        .send(&request))
    .await
    .expect("sensor send fd_write over global swarm");
    let received = (coordinator.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
        .await
        .expect("coordinator recv fd_write");
    assert_eq!(received, request);
    let EngineReq::FdWrite(received_write) = received else {
        panic!("expected fd_write request");
    };
    assert_eq!(received_write.fd(), TEST_STDOUT_FD);
    assert_eq!(received_write.lease_id(), grant.lease_id());
    assert_eq!(received_write.as_bytes(), TEST_STDOUT_TEXT);

    let reply = EngineRet::FdWriteDone(FdWriteDone::new(
        received_write.fd(),
        received_write.len() as u8,
    ));
    (coordinator
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("coordinator flow<fd_write ret>")
        .send(&reply))
    .await
    .expect("coordinator send fd_write ret over global swarm");
    assert_eq!(
        (sensor.recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
            .await
            .expect("sensor recv fd_write ret"),
        reply
    );

    let release = MemRelease::new(grant.lease_id());
    (sensor
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("sensor flow<mem release>")
        .send(&release))
    .await
    .expect("sensor send memory release over global swarm");
    assert_eq!(
        (coordinator.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
            .await
            .expect("coordinator recv memory release"),
        release
    );
    leases.release(release).expect("release read lease");
}

#[test]
fn swarm_frame_is_bounded_authenticated_and_label_hint_is_not_authority() {
    hibana_pico::port::exec::run_current_task(async {
        let mut encoded_payload = [0u8; 8];
        let sample_req = RemoteSampleRequest::new(4, 9, 3);
        let payload_len = sample_req
            .encode_into(&mut encoded_payload)
            .expect("encode remote sample request");

        let frame = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            91,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_ACTUATE_REQ,
            11,
            0,
            &encoded_payload[..payload_len],
            SECURE,
        )
        .expect("create secure frame");
        let mut wire = [0u8; SWARM_FRAME_MAX_WIRE_LEN];
        let wire_len = frame.encode_into(&mut wire).expect("encode frame");
        let decoded = SwarmFrame::decode(&wire[..wire_len]).expect("decode frame");
        decoded.verify(SECURE).expect("auth tag verifies");
        assert_eq!(decoded.label_hint(), LABEL_REMOTE_ACTUATE_REQ);
        assert_eq!(
            RemoteSampleRequest::decode_payload(Payload::new(decoded.payload()))
                .expect("payload remains sample request"),
            sample_req
        );
        assert!(RemoteActuateRequest::decode_payload(Payload::new(decoded.payload())).is_err());

        let mut tampered = wire;
        tampered[28] ^= 1;
        let tampered = SwarmFrame::decode(&tampered[..wire_len]).expect("decode tampered frame");
        assert_eq!(tampered.verify(SECURE), Err(SwarmError::AuthFailed));

        let oversized = [0u8; SWARM_FRAME_PAYLOAD_CAPACITY + 1];
        assert_eq!(
            SwarmFrame::new(
                COORDINATOR,
                SENSOR,
                91,
                SESSION_GENERATION,
                0,
                LABEL_REMOTE_SAMPLE_REQ,
                12,
                0,
                &oversized,
                SECURE,
            ),
            Err(SwarmError::PayloadTooLarge)
        );
    });
}

#[test]
fn swarm_auth_and_replay_failures_drop_and_update_telemetry_without_payload_authority() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<4> = HostSwarmMedium::new();
        let payload = [0xAA, 0xBB];
        let secure = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            93,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            1,
            0,
            &payload,
            SECURE,
        )
        .expect("secure frame");
        let mut wire = [0u8; SWARM_FRAME_MAX_WIRE_LEN];
        let wire_len = secure.encode_into(&mut wire).expect("encode frame");
        wire[28] ^= 1;
        let tampered = SwarmFrame::decode(&wire[..wire_len]).expect("decode tampered frame");
        medium.send(tampered).expect("queue tampered frame");

        let mut replay = ReplayWindow::new();
        assert_eq!(
            medium.recv(SENSOR, &mut replay, SECURE),
            Err(SwarmError::AuthFailed)
        );
        let telemetry = medium.drop_telemetry();
        assert_eq!(telemetry.auth_failed(), 1);
        assert_eq!(telemetry.replayed(), 0);
        assert_eq!(telemetry.total(), 1);
        assert_eq!(
            medium.recv(SENSOR, &mut replay, SECURE),
            Err(SwarmError::QueueEmpty)
        );

        let first = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            93,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            2,
            0,
            &payload,
            SECURE,
        )
        .expect("first frame");
        let duplicated = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            93,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            2,
            0,
            &payload,
            SECURE,
        )
        .expect("duplicated frame");
        medium.send(first).expect("queue first frame");
        medium.send(duplicated).expect("queue replay frame");
        assert_eq!(
            medium
                .recv(SENSOR, &mut replay, SECURE)
                .expect("first accepted")
                .payload(),
            &payload
        );
        assert_eq!(
            medium.recv(SENSOR, &mut replay, SECURE),
            Err(SwarmError::Replay)
        );
        let telemetry = medium.drop_telemetry();
        assert_eq!(telemetry.auth_failed(), 1);
        assert_eq!(telemetry.replayed(), 1);
        assert_eq!(telemetry.total(), 2);
        assert_eq!(
            medium.recv(SENSOR, &mut replay, SECURE),
            Err(SwarmError::QueueEmpty)
        );
    });
}

#[test]
fn swarm_transport_copies_payload_and_does_not_share_node_memory() {
    hibana_pico::port::exec::run_current_task(async {
        let mut source_memory = [0x11, 0x22, 0x33, 0x44];
        let original_payload = source_memory;
        let frame = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            230,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            1,
            0,
            &source_memory,
            SECURE,
        )
        .expect("build frame from source memory bytes");

        source_memory.fill(0);
        assert_eq!(frame.payload(), &original_payload);
        assert_ne!(frame.payload(), &source_memory);

        let medium: HostSwarmMedium<1> = HostSwarmMedium::new();
        medium.send(frame).expect("queue copied swarm frame");
        let mut replay = ReplayWindow::new();
        let received = medium
            .recv(SENSOR, &mut replay, SECURE)
            .expect("receive copied swarm frame");
        assert_eq!(received.payload(), &original_payload);
        assert_ne!(received.payload(), &source_memory);
    });
}

#[test]
fn swarm_fragmentation_is_explicit_bounded_and_reassembles_secure_frames() {
    hibana_pico::port::exec::run_current_task(async {
        const PAYLOAD_LEN: usize = SWARM_FRAGMENT_CHUNK_CAPACITY + 17;
        let mut payload = [0u8; PAYLOAD_LEN];
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(3).wrapping_add(1);
        }

        let count = SwarmFragment::fragment_count(payload.len()).expect("fragment count");
        assert_eq!(count, 2);
        assert_eq!(
            SwarmFragment::fragment_count(0).expect("empty payload needs no fragments"),
            0
        );

        let mut reassembly: SwarmReassemblyBuffer<PAYLOAD_LEN, 2> = SwarmReassemblyBuffer::new();
        for index in 0..count {
            let fragment =
                SwarmFragment::from_payload(9, &payload, index).expect("split payload fragment");
            let mut fragment_payload = [0u8; SWARM_FRAME_PAYLOAD_CAPACITY];
            let fragment_len = fragment
                .encode_into(&mut fragment_payload)
                .expect("encode fragment payload");
            assert!(fragment_len <= SWARM_FRAME_PAYLOAD_CAPACITY);
            assert!(fragment_len >= SWARM_FRAGMENT_HEADER_LEN);

            let frame = SwarmFrame::new(
                COORDINATOR,
                SENSOR,
                92,
                SESSION_GENERATION,
                0,
                LABEL_REMOTE_SAMPLE_REQ,
                20 + index as u32,
                0,
                &fragment_payload[..fragment_len],
                SECURE,
            )
            .expect("wrap fragment in secure swarm frame");
            let mut wire = [0u8; SWARM_FRAME_MAX_WIRE_LEN];
            let wire_len = frame.encode_into(&mut wire).expect("encode frame");
            let decoded = SwarmFrame::decode(&wire[..wire_len]).expect("decode frame");
            decoded
                .verify(SECURE)
                .expect("secure fragment frame verifies");
            assert_eq!(decoded.label_hint(), LABEL_REMOTE_SAMPLE_REQ);

            let decoded_fragment =
                SwarmFragment::decode(decoded.payload()).expect("decode explicit fragment payload");
            match reassembly
                .push(decoded_fragment)
                .expect("push fragment into bounded buffer")
            {
                Some(reassembled) => assert_eq!(reassembled, payload.as_slice()),
                None => assert_eq!(index, 0),
            }
        }
        assert_eq!(
            reassembly.finish().expect("complete reassembly"),
            payload.as_slice()
        );

        assert_eq!(
            reassembly
                .push(SwarmFragment::from_payload(9, &payload, 0).expect("duplicate fragment")),
            Err(SwarmError::FragmentDuplicate)
        );

        let first = SwarmFragment::from_payload(1, &payload, 0).expect("first fragment");
        let second_wrong_id =
            SwarmFragment::from_payload(2, &payload, 1).expect("second fragment with wrong id");
        let mut mismatch: SwarmReassemblyBuffer<PAYLOAD_LEN, 2> = SwarmReassemblyBuffer::new();
        assert!(mismatch.push(first).expect("push first").is_none());
        assert_eq!(
            mismatch.push(second_wrong_id),
            Err(SwarmError::FragmentSetMismatch)
        );

        let mut too_small: SwarmReassemblyBuffer<16, 2> = SwarmReassemblyBuffer::new();
        assert_eq!(
            too_small
                .push(SwarmFragment::from_payload(7, &payload, 0).expect("oversized fragment")),
            Err(SwarmError::PayloadTooLarge)
        );
    });
}

#[test]
fn swarm_medium_rejects_replay_and_revoked_neighbors() {
    hibana_pico::port::exec::run_current_task(async {
        let mut neighbors: NeighborTable<3> = NeighborTable::new();
        neighbors
            .add(NeighborEntry::new(COORDINATOR, 0, SESSION_GENERATION))
            .expect("add coordinator");
        neighbors
            .add(NeighborEntry::new(SENSOR, 1, SESSION_GENERATION))
            .expect("add sensor");
        assert_eq!(neighbors.node_for_role(1), Ok(SENSOR));
        assert_eq!(neighbors.validate(SENSOR, SESSION_GENERATION), Ok(()));
        neighbors.revoke(SENSOR).expect("revoke sensor");
        assert_eq!(
            neighbors.validate(SENSOR, SESSION_GENERATION),
            Err(SwarmError::Revoked)
        );

        let mut provisioning = ProvisioningRecord::new(
            SENSOR,
            SwarmCredential::new(0x5155_4943),
            NodeRole::Sensor.bit(),
        );
        assert_eq!(provisioning.trigger_join(), Err(SwarmError::BadNode));
        provisioning.install_wifi_credentials();
        provisioning
            .trigger_join()
            .expect("join after Wi-Fi config");
        assert!(provisioning.join_triggered());

        let medium: HostSwarmMedium<4> = HostSwarmMedium::new();
        let payload = [0xAA, 0xBB];
        let first = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            92,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            4,
            0,
            &payload,
            SECURE,
        )
        .expect("first frame");
        let replay = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            92,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            4,
            0,
            &payload,
            SECURE,
        )
        .expect("replay frame");
        medium.send(first).expect("send first frame");
        medium.send(replay).expect("send replay frame");

        let mut window = ReplayWindow::new();
        assert_eq!(
            medium
                .recv(SENSOR, &mut window, SECURE)
                .expect("first accepted")
                .payload(),
            &payload
        );
        assert_eq!(
            medium.recv(SENSOR, &mut window, SECURE),
            Err(SwarmError::Replay)
        );
    });
}

#[test]
fn phone_local_provisioning_triggers_wifi_join_but_swarm_grant_is_runtime_authority() {
    hibana_pico::port::exec::run_current_task(async {
        let sensor_roles = RoleMask::single(NodeRole::Sensor);
        let mut provisioning = ProvisioningRecord::new(
            SENSOR,
            SwarmCredential::new(0x5052_4f56),
            sensor_roles.bits(),
        );
        assert_eq!(provisioning.trigger_join(), Err(SwarmError::BadNode));
        provisioning.install_wifi_credentials();
        provisioning
            .trigger_join()
            .expect("phone-local Wi-Fi credentials permit join trigger");
        assert!(provisioning.join_triggered());
        assert_eq!(provisioning.node_id(), SENSOR);
        assert_eq!(RoleMask::new(provisioning.role_mask()), sensor_roles);

        let runtime_security = SwarmSecurity::Secure(provisioning.credential());
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        {
            let clock0 = CounterClock::new();
            let mut tap0 = [TapEvent::zero(); 128];
            let mut slab0 = vec![0u8; 262_144];
            let cluster0 = SwarmTestKit::new(&clock0);
            let rv0 = cluster0
                .add_rendezvous_from_config(
                    Config::new(&mut tap0, slab0.as_mut_slice())
                        .with_lane_range(0..21)
                        .with_universe(EngineLabelUniverse),
                    HostSwarmTransport::new(
                        &medium,
                        COORDINATOR,
                        SENSOR,
                        SESSION_GENERATION,
                        runtime_security,
                    ),
                )
                .expect("register gateway rendezvous");

            let clock1 = CounterClock::new();
            let mut tap1 = [TapEvent::zero(); 128];
            let mut slab1 = vec![0u8; 262_144];
            let cluster1 = SwarmTestKit::new(&clock1);
            let rv1 = cluster1
                .add_rendezvous_from_config(
                    Config::new(&mut tap1, slab1.as_mut_slice())
                        .with_lane_range(0..21)
                        .with_universe(EngineLabelUniverse),
                    HostSwarmTransport::new(
                        &medium,
                        SENSOR,
                        COORDINATOR,
                        SESSION_GENERATION,
                        runtime_security,
                    ),
                )
                .expect("register node rendezvous");

            let (gateway_program, node_program) = project_swarm_join_roles();
            let mut gateway = cluster0
                .enter(rv0, SessionId::new(207), &gateway_program, NoBinding)
                .expect("attach gateway");
            let mut node = cluster1
                .enter(rv1, SessionId::new(207), &node_program, NoBinding)
                .expect("attach node");

            let offer = JoinOffer::new(provisioning.node_id(), sensor_roles);
            (gateway
                .flow::<JoinOfferMsg>()
                .expect("gateway flow<join offer>")
                .send(&offer))
            .await
            .expect("gateway sends join offer over swarm");
            let received_offer = (node.recv::<JoinOfferMsg>())
                .await
                .expect("node receives join offer over swarm");
            assert_eq!(received_offer.node_id(), provisioning.node_id());
            assert_eq!(received_offer.role_mask(), sensor_roles);

            let request = JoinRequest::new(received_offer.node_id(), received_offer.role_mask());
            (node
                .flow::<JoinRequestMsg>()
                .expect("node flow<join request>")
                .send(&request))
            .await
            .expect("node sends join request over swarm");
            let received_request = (gateway.recv::<JoinRequestMsg>())
                .await
                .expect("gateway receives join request");
            assert_eq!(received_request.node_id(), provisioning.node_id());
            assert_eq!(received_request.role_mask(), sensor_roles);

            let grant = JoinGrant::new(
                received_request.node_id(),
                received_request.role_mask(),
                SESSION_GENERATION,
                true,
            );
            (gateway
                .flow::<JoinGrantMsg>()
                .expect("gateway flow<join grant>")
                .send(&grant))
            .await
            .expect("gateway sends join grant over swarm");
            let received_grant = (node.recv::<JoinGrantMsg>())
                .await
                .expect("node receives join grant");
            assert!(received_grant.accepted());
            assert_eq!(received_grant.node_id(), provisioning.node_id());
            assert_eq!(received_grant.role_mask(), sensor_roles);
            assert_eq!(received_grant.session_generation(), SESSION_GENERATION);

            let ack = JoinAck::new(
                received_grant.node_id(),
                received_grant.session_generation(),
            );
            (node
                .flow::<JoinAckMsg>()
                .expect("node flow<join ack>")
                .send(&ack))
            .await
            .expect("node sends join ack over swarm");
            let received_ack = (gateway.recv::<JoinAckMsg>())
                .await
                .expect("gateway receives join ack");
            assert_eq!(received_ack.node_id(), provisioning.node_id());
            assert_eq!(received_ack.session_generation(), SESSION_GENERATION);
        }

        let mut neighbors: NeighborTable<3> = NeighborTable::new();
        neighbors
            .add(NeighborEntry::new(COORDINATOR, 0, SESSION_GENERATION))
            .expect("install coordinator neighbor");
        neighbors
            .add(NeighborEntry::new(SENSOR, 1, SESSION_GENERATION))
            .expect("install provisioned sensor neighbor");
        assert_eq!(neighbors.validate(SENSOR, SESSION_GENERATION), Ok(()));

        {
            let clock0 = CounterClock::new();
            let mut tap0 = [TapEvent::zero(); 128];
            let mut slab0 = vec![0u8; 262_144];
            let cluster0 = SwarmTestKit::new(&clock0);
            let rv0 = cluster0
                .add_rendezvous_from_config(
                    Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                    HostSwarmTransport::new(
                        &medium,
                        COORDINATOR,
                        SENSOR,
                        SESSION_GENERATION,
                        runtime_security,
                    ),
                )
                .expect("register post-join coordinator rendezvous");

            let clock1 = CounterClock::new();
            let mut tap1 = [TapEvent::zero(); 128];
            let mut slab1 = vec![0u8; 262_144];
            let cluster1 = SwarmTestKit::new(&clock1);
            let rv1 = cluster1
                .add_rendezvous_from_config(
                    Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                    HostSwarmTransport::new(
                        &medium,
                        SENSOR,
                        COORDINATOR,
                        SESSION_GENERATION,
                        runtime_security,
                    ),
                )
                .expect("register post-join sensor rendezvous");

            let (coordinator_program, sensor_program) = project_sample_roles();
            let mut coordinator = cluster0
                .enter(rv0, SessionId::new(208), &coordinator_program, NoBinding)
                .expect("attach post-join coordinator");
            let mut sensor = cluster1
                .enter(rv1, SessionId::new(208), &sensor_program, NoBinding)
                .expect("attach post-join sensor");

            let sample_request = RemoteSampleRequest::new(1, 2, 11);
            (coordinator
                .flow::<RemoteSampleReqMsg>()
                .expect("coordinator flow<post-join sample req>")
                .send(&sample_request))
            .await
            .expect("coordinator sends post-join sample request over swarm");
            let _sample_req_frame_label = medium
                .peek_label(SENSOR)
                .expect("swarm transport exposes sample request frame label hint");
            assert_eq!(
                (sensor.recv::<RemoteSampleReqMsg>())
                    .await
                    .expect("sensor receives post-join sample request"),
                sample_request
            );

            let sample = RemoteSample::new(11, 0, 25_000, 900);
            (sensor
                .flow::<RemoteSampleRetMsg>()
                .expect("sensor flow<post-join sample ret>")
                .send(&sample))
            .await
            .expect("sensor sends post-join sample over swarm");
            assert_eq!(
                (coordinator.recv::<RemoteSampleRetMsg>())
                    .await
                    .expect("coordinator receives post-join sample"),
                sample
            );
        }
    });
}

#[test]
fn ble_provisioning_installs_local_config_but_swarm_join_remains_runtime_authority() {
    hibana_pico::port::exec::run_current_task(async {
        let sensor_roles = RoleMask::single(NodeRole::Sensor);
        let ble_bundle = BleProvisioningBundle::new(
            SENSOR,
            SwarmCredential::new(0x424c_4550),
            sensor_roles.bits(),
        );
        let mut missing_wifi = ProvisioningRecord::from_ble(ble_bundle);
        assert!(!missing_wifi.wifi_configured());
        assert_eq!(missing_wifi.trigger_join(), Err(SwarmError::BadNode));
        assert!(!missing_wifi.join_triggered());

        let mut provisioning = ProvisioningRecord::from_ble(ble_bundle.with_wifi_credentials());
        assert!(provisioning.wifi_configured());
        provisioning
            .trigger_join()
            .expect("BLE-installed Wi-Fi credentials may trigger join");
        assert!(provisioning.join_triggered());
        assert_eq!(provisioning.node_id(), SENSOR);
        assert_eq!(RoleMask::new(provisioning.role_mask()), sensor_roles);

        let mut neighbors: NeighborTable<2> = NeighborTable::new();
        assert_eq!(
            neighbors.validate(provisioning.node_id(), SESSION_GENERATION),
            Err(SwarmError::BadNode),
            "BLE provisioning is local config only; it does not grant runtime neighbor authority"
        );

        let runtime_security = SwarmSecurity::Secure(provisioning.credential());
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();
        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(
                    &medium,
                    COORDINATOR,
                    SENSOR,
                    SESSION_GENERATION,
                    runtime_security,
                ),
            )
            .expect("register BLE-provisioned gateway rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(
                    &medium,
                    SENSOR,
                    COORDINATOR,
                    SESSION_GENERATION,
                    runtime_security,
                ),
            )
            .expect("register BLE-provisioned node rendezvous");

        let (gateway_program, node_program) = project_swarm_join_roles();
        let mut gateway = cluster0
            .enter(rv0, SessionId::new(223), &gateway_program, NoBinding)
            .expect("attach BLE-provisioned gateway");
        let mut node = cluster1
            .enter(rv1, SessionId::new(223), &node_program, NoBinding)
            .expect("attach BLE-provisioned node");

        let offer = JoinOffer::new(provisioning.node_id(), sensor_roles);
        (gateway
            .flow::<JoinOfferMsg>()
            .expect("gateway flow<BLE join offer>")
            .send(&offer))
        .await
        .expect("gateway sends BLE-provisioned join offer over Wi-Fi substrate");
        let received_offer = (node.recv::<JoinOfferMsg>())
            .await
            .expect("node receives BLE-provisioned join offer");
        assert_eq!(received_offer, offer);

        let request = JoinRequest::new(received_offer.node_id(), received_offer.role_mask());
        (node
            .flow::<JoinRequestMsg>()
            .expect("node flow<BLE join request>")
            .send(&request))
        .await
        .expect("node sends BLE-provisioned join request over Wi-Fi substrate");
        assert_eq!(
            (gateway.recv::<JoinRequestMsg>())
                .await
                .expect("gateway receives BLE-provisioned join request"),
            request
        );

        let grant = JoinGrant::new(
            request.node_id(),
            request.role_mask(),
            SESSION_GENERATION,
            true,
        );
        (gateway
            .flow::<JoinGrantMsg>()
            .expect("gateway flow<BLE join grant>")
            .send(&grant))
        .await
        .expect("gateway sends BLE-provisioned join grant over Wi-Fi substrate");
        let received_grant = (node.recv::<JoinGrantMsg>())
            .await
            .expect("node receives BLE-provisioned join grant");
        assert!(received_grant.accepted());
        assert_eq!(received_grant.node_id(), provisioning.node_id());
        assert_eq!(received_grant.role_mask(), sensor_roles);

        let ack = JoinAck::new(
            received_grant.node_id(),
            received_grant.session_generation(),
        );
        (node
            .flow::<JoinAckMsg>()
            .expect("node flow<BLE join ack>")
            .send(&ack))
        .await
        .expect("node sends BLE-provisioned join ack over Wi-Fi substrate");
        assert_eq!(
            (gateway.recv::<JoinAckMsg>())
                .await
                .expect("gateway receives BLE-provisioned join ack"),
            ack
        );

        neighbors
            .add(NeighborEntry::new(
                received_grant.node_id(),
                1,
                received_grant.session_generation(),
            ))
            .expect("install neighbor only after accepted swarm grant");
        assert_eq!(
            neighbors.validate(provisioning.node_id(), SESSION_GENERATION),
            Ok(())
        );
    });
}

#[test]
fn swarm_leave_revoke_choreography_quiesces_objects_leases_and_neighbors() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let observer_node = ACTUATOR;
        let role_nodes = [COORDINATOR, SENSOR, observer_node, NodeId::new(4)];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register gateway rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register leaving-node rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    observer_node,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register observer rendezvous");

        let (gateway_program, leaving_program, observer_program) = project_swarm_leave_roles();
        let mut gateway = cluster0
            .enter(rv0, SessionId::new(209), &gateway_program, NoBinding)
            .expect("attach gateway");
        let mut leaving_node = cluster1
            .enter(rv1, SessionId::new(209), &leaving_program, NoBinding)
            .expect("attach leaving node");
        let mut observer = cluster2
            .enter(rv2, SessionId::new(209), &observer_program, NoBinding)
            .expect("attach observer");

        let mut node_leases = MemoryLeaseTable::<2>::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
        node_leases
            .grant_read(MemBorrow::new(TEST_STDOUT_PTR, 8, TEST_MEMORY_EPOCH))
            .expect("install outstanding node lease");
        assert!(node_leases.has_outstanding_leases());

        let mut node_objects: RemoteObjectTable<2> = RemoteObjectTable::new();
        let node_cap = node_objects
            .apply_cap_grant(
                SENSOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                NodeRole::Sensor.bit() as u8,
                0,
                LABEL_REMOTE_SAMPLE_REQ,
                RemoteRights::Read,
                RemoteResource::Sensor,
            )
            .expect("install active authenticated remote object on leaving node");
        assert_eq!(
            node_objects
                .resolve(
                    node_cap.fd(),
                    node_cap.generation(),
                    RemoteRights::Read,
                    SESSION_GENERATION
                )
                .expect("active cap resolves before revoke")
                .target_node(),
            SENSOR
        );

        let mut observer_neighbors: NeighborTable<4> = NeighborTable::new();
        observer_neighbors
            .add(NeighborEntry::new(COORDINATOR, 0, SESSION_GENERATION))
            .expect("observer installs coordinator neighbor");
        observer_neighbors
            .add(NeighborEntry::new(SENSOR, 1, SESSION_GENERATION))
            .expect("observer installs leaving neighbor");
        assert_eq!(
            observer_neighbors.validate(SENSOR, SESSION_GENERATION),
            Ok(())
        );

        let suspend = SwarmSuspend::new(SENSOR, SESSION_GENERATION);
        (gateway
            .flow::<SwarmSuspendMsg>()
            .expect("gateway flow<suspend>")
            .send(&suspend))
        .await
        .expect("gateway sends suspend over swarm");
        let received_suspend = (leaving_node.recv::<SwarmSuspendMsg>())
            .await
            .expect("leaving node receives suspend");
        assert_eq!(received_suspend.node_id(), SENSOR);
        assert_eq!(received_suspend.session_generation(), SESSION_GENERATION);
        node_leases.fence(MemFence::new(
            MemFenceReason::Suspend,
            TEST_MEMORY_EPOCH + 1,
        ));
        assert!(!node_leases.has_outstanding_leases());
        assert_eq!(node_leases.epoch(), TEST_MEMORY_EPOCH + 1);

        let revoke_objects = RemoteObjectsRevoke::new(SENSOR, SESSION_GENERATION);
        (gateway
            .flow::<RemoteObjectsRevokeMsg>()
            .expect("gateway flow<remote object revoke>")
            .send(&revoke_objects))
        .await
        .expect("gateway sends remote object revoke over swarm");
        let received_revoke = (leaving_node.recv::<RemoteObjectsRevokeMsg>())
            .await
            .expect("leaving node receives remote object revoke");
        assert_eq!(received_revoke.node_id(), SENSOR);
        assert_eq!(received_revoke.session_generation(), SESSION_GENERATION);
        assert_eq!(
            node_objects.revoke_node_generation(
                received_revoke.node_id(),
                received_revoke.session_generation().wrapping_add(1),
            ),
            Err(RemoteError::BadSessionGeneration)
        );
        assert!(node_objects.has_active());
        assert_eq!(
            node_objects.revoke_node_generation(
                received_revoke.node_id(),
                received_revoke.session_generation(),
            ),
            Ok(1)
        );
        assert_eq!(
            node_objects.resolve(
                node_cap.fd(),
                node_cap.generation(),
                RemoteRights::Read,
                SESSION_GENERATION
            ),
            Err(RemoteError::Revoked)
        );

        let revoked = NodeRevoked::new(SENSOR, SESSION_GENERATION);
        (gateway
            .flow::<NodeRevokedMsg>()
            .expect("gateway flow<node revoked>")
            .send(&revoked))
        .await
        .expect("gateway broadcasts node revoked over swarm");
        let received_revoked = (observer.recv::<NodeRevokedMsg>())
            .await
            .expect("observer receives node revoked");
        assert_eq!(received_revoked.node_id(), SENSOR);
        assert_eq!(received_revoked.session_generation(), SESSION_GENERATION);
        assert_eq!(
            observer_neighbors.revoke_generation(
                received_revoked.node_id(),
                received_revoked.session_generation().wrapping_add(1),
            ),
            Err(SwarmError::BadGeneration)
        );
        assert_eq!(
            observer_neighbors.validate(SENSOR, SESSION_GENERATION),
            Ok(())
        );
        observer_neighbors
            .revoke_generation(
                received_revoked.node_id(),
                received_revoked.session_generation(),
            )
            .expect("observer revokes neighbor");
        assert_eq!(
            observer_neighbors.validate(SENSOR, SESSION_GENERATION),
            Err(SwarmError::Revoked)
        );

        let ack = LeaveAck::new(SENSOR, SESSION_GENERATION);
        (leaving_node
            .flow::<LeaveAckMsg>()
            .expect("leaving node flow<leave ack>")
            .send(&ack))
        .await
        .expect("leaving node sends leave ack over swarm");
        let received_ack = (gateway.recv::<LeaveAckMsg>())
            .await
            .expect("gateway receives leave ack");
        assert_eq!(received_ack.node_id(), SENSOR);
        assert_eq!(received_ack.session_generation(), SESSION_GENERATION);
    });
}

#[test]
fn remote_sensor_and_actuator_objects_are_wired_through_hibana_messages() {
    hibana_pico::port::exec::run_current_task(async {
        let mut caps: RemoteObjectTable<4> = RemoteObjectTable::new();
        let sensor_cap = caps
            .apply_cap_grant(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                NodeRole::Sensor.bit() as u8,
                0,
                LABEL_REMOTE_SAMPLE_REQ,
                RemoteRights::Read,
                RemoteResource::Sensor,
            )
            .expect("install authenticated sensor cap");
        let actuator_cap = caps
            .apply_cap_grant(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                ACTUATOR,
                NodeRole::Actuator.bit() as u8,
                0,
                LABEL_REMOTE_ACTUATE_REQ,
                RemoteRights::Write,
                RemoteResource::Actuator,
            )
            .expect("install authenticated actuator cap");

        assert_eq!(
            caps.resolve(
                sensor_cap.fd(),
                sensor_cap.generation(),
                RemoteRights::Write,
                SESSION_GENERATION
            ),
            Err(RemoteError::PermissionDenied)
        );
        assert_eq!(
            caps.resolve(
                sensor_cap.fd(),
                sensor_cap.generation().wrapping_add(1),
                RemoteRights::Read,
                SESSION_GENERATION
            ),
            Err(RemoteError::BadGeneration)
        );

        let sensor_backend = HostQueueBackend::new();
        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&sensor_backend),
            )
            .expect("register coordinator rendezvous");
        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&sensor_backend),
            )
            .expect("register sensor rendezvous");
        let (coordinator_program, sensor_program) = project_sample_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(200), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(200), &sensor_program, NoBinding)
            .expect("attach sensor");

        let sample_req = RemoteSampleRequest::new(sensor_cap.fd(), sensor_cap.generation(), 5);
        let resolved = caps
            .resolve(
                sample_req.fd(),
                sample_req.generation(),
                RemoteRights::Read,
                SESSION_GENERATION,
            )
            .expect("resolve sensor cap before send");
        assert_eq!(resolved.target_node(), SENSOR);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sample req>")
            .send(&sample_req))
        .await
        .expect("send sample request");
        assert_eq!(
            (sensor.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor recv sample request"),
            sample_req
        );
        let sample = RemoteSample::new(5, 0, 42_000, 1000);
        (sensor
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor flow<sample ret>")
            .send(&sample))
        .await
        .expect("send sample");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample"),
            sample
        );

        let actuator_backend = HostQueueBackend::new();
        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&actuator_backend),
            )
            .expect("register coordinator rendezvous");
        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&actuator_backend),
            )
            .expect("register actuator rendezvous");
        let (coordinator_program, actuator_program) = project_actuator_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(201), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut actuator = cluster1
            .enter(rv1, SessionId::new(201), &actuator_program, NoBinding)
            .expect("attach actuator");

        let actuate = RemoteActuateRequest::new(actuator_cap.fd(), actuator_cap.generation(), 9, 1);
        assert_eq!(
            caps.resolve(
                actuate.fd(),
                actuate.generation(),
                RemoteRights::Write,
                SESSION_GENERATION
            )
            .expect("resolve actuator cap")
            .target_node(),
            ACTUATOR
        );
        (coordinator
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<actuate req>")
            .send(&actuate))
        .await
        .expect("send actuate");
        assert_eq!(
            (actuator.recv::<RemoteActuateReqMsg>())
                .await
                .expect("actuator recv actuate"),
            actuate
        );
        let ack = RemoteActuateAck::new(9, 0);
        (actuator
            .flow::<RemoteActuateRetMsg>()
            .expect("actuator flow<actuate ack>")
            .send(&ack))
        .await
        .expect("send actuate ack");
        assert_eq!(
            (coordinator.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator recv ack"),
            ack
        );

        assert_eq!(
            caps.revoke_node_generation(ACTUATOR, SESSION_GENERATION.wrapping_add(1)),
            Err(RemoteError::BadSessionGeneration)
        );
        assert_eq!(
            caps.resolve(
                actuator_cap.fd(),
                actuator_cap.generation(),
                RemoteRights::Write,
                SESSION_GENERATION
            )
            .expect("stale revoke must not drop actuator cap")
            .target_node(),
            ACTUATOR
        );
        assert_eq!(
            caps.revoke_node_generation(ACTUATOR, SESSION_GENERATION),
            Ok(1)
        );
        assert_eq!(
            caps.resolve(
                actuator_cap.fd(),
                actuator_cap.generation(),
                RemoteRights::Write,
                SESSION_GENERATION
            ),
            Err(RemoteError::Revoked)
        );
    });
}

#[test]
fn remote_object_control_selects_explicit_route_arm_without_bridge() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, GATEWAY, SENSOR, ACTUATOR];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register engine rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register kernel rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register sensor rendezvous");

        let mut caps: RemoteObjectTable<4> = RemoteObjectTable::new();
        let sensor_cap = caps
            .apply_cap_grant_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                NodeRole::Sensor.bit() as u8,
                17,
                LABEL_REMOTE_SAMPLE_REQ,
                2,
                RemoteRights::Read,
                RemoteResource::Sensor,
            )
            .expect("install authenticated remote sensor fd");
        assert_eq!(
            caps.apply_control(
                RemoteControl::cap_grant_remote_with_policy(
                    COORDINATOR,
                    SwarmCredential::new(0x5752_4f4e),
                    SESSION_GENERATION,
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    17,
                    LABEL_REMOTE_SAMPLE_REQ,
                    2,
                    RemoteRights::Read,
                    RemoteResource::Sensor,
                ),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(RemoteError::AuthFailed)
        );
        assert_eq!(
            caps.apply_control(
                RemoteControl::cap_grant_remote_with_policy(
                    COORDINATOR,
                    SWARM_CREDENTIAL,
                    SESSION_GENERATION.wrapping_add(1),
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    17,
                    LABEL_REMOTE_SAMPLE_REQ,
                    2,
                    RemoteRights::Read,
                    RemoteResource::Sensor,
                ),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(RemoteError::BadSessionGeneration)
        );
        assert_eq!(sensor_cap.policy_slot(), 2);
        let actuator_cap = caps
            .apply_cap_grant(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                NodeRole::Actuator.bit() as u8,
                17,
                LABEL_REMOTE_ACTUATE_REQ,
                RemoteRights::Write,
                RemoteResource::Actuator,
            )
            .expect("install authenticated remote actuator fd");
        let wrong_resource_cap = caps
            .apply_cap_grant(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                NodeRole::Sensor.bit() as u8,
                17,
                LABEL_REMOTE_SAMPLE_REQ,
                RemoteRights::ReadWrite,
                RemoteResource::Sensor,
            )
            .expect("install authenticated read-write sensor fd");
        let mut policy_slots: PolicySlotTable<4> = PolicySlotTable::new();
        policy_slots
            .allow(sensor_cap.policy_slot())
            .expect("allow remote sensor policy slot");
        policy_slots
            .allow(actuator_cap.policy_slot())
            .expect("allow remote actuator policy slot");
        assert_eq!(
            caps.route_fd_write(
                wrong_resource_cap.fd(),
                wrong_resource_cap.generation(),
                SESSION_GENERATION
            ),
            RemoteFdWriteRoute::Rejected(RemoteError::WrongResource)
        );
        assert_eq!(
            caps.route_fd_read_authorized(
                sensor_cap.fd(),
                sensor_cap.generation(),
                sensor_cap.route_key(),
                &policy_slots,
            ),
            RemoteFdReadRoute::RemoteSensor(sensor_cap)
        );
        policy_slots
            .deny(sensor_cap.policy_slot())
            .expect("deny remote sensor policy slot");
        assert_eq!(
            caps.route_fd_read_authorized(
                sensor_cap.fd(),
                sensor_cap.generation(),
                sensor_cap.route_key(),
                &policy_slots,
            ),
            RemoteFdReadRoute::Rejected(RemoteError::PolicyDenied)
        );
        policy_slots
            .allow(sensor_cap.policy_slot())
            .expect("re-allow remote sensor policy slot");
        assert_eq!(
            caps.route_fd_read_routed(
                sensor_cap.fd(),
                sensor_cap.generation(),
                RemoteRoute::new(
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    17,
                    LABEL_REMOTE_SAMPLE_REQ,
                    SESSION_GENERATION.wrapping_add(1),
                )
            ),
            RemoteFdReadRoute::Rejected(RemoteError::BadSessionGeneration)
        );
        assert_eq!(
            caps.route_fd_read_routed(
                sensor_cap.fd(),
                sensor_cap.generation(),
                RemoteRoute::new(
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    18,
                    LABEL_REMOTE_SAMPLE_REQ,
                    SESSION_GENERATION,
                )
            ),
            RemoteFdReadRoute::Rejected(RemoteError::BadRoute)
        );
        assert_eq!(
            caps.route_fd_read_routed(
                sensor_cap.fd(),
                sensor_cap.generation(),
                RemoteRoute::with_policy(
                    SENSOR,
                    NodeRole::Sensor.bit() as u8,
                    17,
                    LABEL_REMOTE_SAMPLE_REQ,
                    SESSION_GENERATION,
                    sensor_cap.policy_slot().wrapping_add(1),
                )
            ),
            RemoteFdReadRoute::Rejected(RemoteError::BadRoute)
        );

        let (engine_program, kernel_program, sensor_program) = project_remote_fd_read_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(217), &engine_program, NoBinding)
            .expect("attach engine read route projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(217), &kernel_program, NoBinding)
            .expect("attach kernel read route projection");
        let mut sensor = cluster2
            .enter(rv2, SessionId::new(217), &sensor_program, NoBinding)
            .expect("attach sensor read route projection");

        let read = FdRead::new_with_lease(sensor_cap.fd(), 1, 4).expect("remote fd_read");
        let read_request = EngineReq::FdRead(read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<fd_read>")
            .send(&read_request))
        .await
        .expect("engine sends fd_read to kernel role");
        let received_read = (kernel.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("kernel receives fd_read");
        assert_eq!(received_read, read_request);
        let EngineReq::FdRead(received_read) = received_read else {
            panic!("expected fd_read request");
        };

        let resolved_sensor = match caps.route_fd_read_authorized(
            received_read.fd(),
            sensor_cap.generation(),
            sensor_cap.route_key(),
            &policy_slots,
        ) {
            RemoteFdReadRoute::RemoteSensor(cap) => cap,
            RemoteFdReadRoute::Rejected(error) => {
                panic!("remote sensor fd should select sensor route: {error:?}")
            }
        };
        assert_eq!(resolved_sensor.target_node(), SENSOR);
        assert_eq!(resolved_sensor.target_role(), NodeRole::Sensor.bit() as u8);
        assert_eq!(resolved_sensor.lane(), 17);
        assert_eq!(resolved_sensor.route(), LABEL_REMOTE_SAMPLE_REQ);

        (kernel
            .flow::<RemoteSensorRouteControl>()
            .expect("kernel flow<remote sensor route>")
            .send(()))
        .await
        .expect("kernel selects remote sensor route");
        let sample_request =
            RemoteSampleRequest::new(resolved_sensor.fd(), resolved_sensor.generation(), 5);
        (kernel
            .flow::<RemoteSampleReqMsg>()
            .expect("kernel flow<remote sample req>")
            .send(&sample_request))
        .await
        .expect("kernel sends remote sample request");
        let sensor_branch = (sensor.offer())
            .await
            .expect("sensor offers remote sample route");
        assert_eq!(
            (sensor_branch.decode::<RemoteSampleReqMsg>())
                .await
                .expect("sensor decodes remote sample request"),
            sample_request
        );
        let sample = RemoteSample::new(5, 0, 0x1122_3344, 100);
        (sensor
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor flow<remote sample ret>")
            .send(&sample))
        .await
        .expect("sensor sends sample");
        assert_eq!(
            (kernel.recv::<RemoteSampleRetMsg>())
                .await
                .expect("kernel receives sample"),
            sample
        );
        let read_done = EngineRet::FdReadDone(
            FdReadDone::new_with_lease(
                received_read.fd(),
                received_read.lease_id(),
                &sample.value().to_be_bytes(),
            )
            .expect("fd_read done"),
        );
        (kernel
            .flow::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>()
            .expect("kernel flow<fd_read ret>")
            .send(&read_done))
        .await
        .expect("kernel sends fd_read ret");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers remote sample route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>())
                .await
                .expect("engine decodes fd_read ret"),
            read_done
        );
        assert!(
            kernel.flow::<RemoteRejectRouteControl>().is_err(),
            "unselected reject arm must not be reachable after sensor route"
        );
        drop(engine);
        drop(kernel);
        drop(sensor);

        let (engine_program, kernel_program, sensor_program) =
            project_remote_fd_write_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(218), &engine_program, NoBinding)
            .expect("attach engine write route projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(218), &kernel_program, NoBinding)
            .expect("attach kernel write route projection");
        let mut sensor = cluster2
            .enter(rv2, SessionId::new(218), &sensor_program, NoBinding)
            .expect("attach sensor write route projection");

        let write = FdWrite::new_with_lease(actuator_cap.fd(), 2, &[9, 0, 0, 0, 1])
            .expect("remote fd_write");
        let write_request = EngineReq::FdWrite(write);
        (engine
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine flow<fd_write>")
            .send(&write_request))
        .await
        .expect("engine sends fd_write to kernel role");
        let received_write = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("kernel receives fd_write");
        assert_eq!(received_write, write_request);
        let EngineReq::FdWrite(received_write) = received_write else {
            panic!("expected fd_write request");
        };

        let resolved_actuator = match caps.route_fd_write_authorized(
            received_write.fd(),
            actuator_cap.generation(),
            actuator_cap.route_key(),
            &policy_slots,
        ) {
            RemoteFdWriteRoute::RemoteActuator(cap) => cap,
            RemoteFdWriteRoute::RemoteManagement(cap) => {
                panic!("remote actuator fd selected management route: {cap:?}")
            }
            RemoteFdWriteRoute::RemoteTelemetry(cap) => {
                panic!("remote actuator fd selected telemetry route: {cap:?}")
            }
            RemoteFdWriteRoute::Rejected(error) => {
                panic!("remote actuator fd should select actuator route: {error:?}")
            }
        };
        assert_eq!(resolved_actuator.target_node(), SENSOR);
        assert_eq!(
            resolved_actuator.target_role(),
            NodeRole::Actuator.bit() as u8
        );
        assert_eq!(resolved_actuator.lane(), 17);
        assert_eq!(resolved_actuator.route(), LABEL_REMOTE_ACTUATE_REQ);

        (kernel
            .flow::<RemoteActuatorRouteControl>()
            .expect("kernel flow<remote actuator route>")
            .send(()))
        .await
        .expect("kernel selects remote actuator route");
        let actuate_request = RemoteActuateRequest::new(
            resolved_actuator.fd(),
            resolved_actuator.generation(),
            received_write.as_bytes()[0],
            u32::from_be_bytes([
                received_write.as_bytes()[1],
                received_write.as_bytes()[2],
                received_write.as_bytes()[3],
                received_write.as_bytes()[4],
            ]),
        );
        (kernel
            .flow::<RemoteActuateReqMsg>()
            .expect("kernel flow<remote actuate req>")
            .send(&actuate_request))
        .await
        .expect("kernel sends remote actuate request");
        let actuator_branch = (sensor.offer())
            .await
            .expect("actuator offers remote actuate route");
        assert_eq!(
            (actuator_branch.decode::<RemoteActuateReqMsg>())
                .await
                .expect("actuator decodes remote actuate request"),
            actuate_request
        );
        let actuate_ack = RemoteActuateAck::new(actuate_request.channel(), 0);
        (sensor
            .flow::<RemoteActuateRetMsg>()
            .expect("actuator flow<remote actuate ret>")
            .send(&actuate_ack))
        .await
        .expect("actuator sends ack");
        assert_eq!(
            (kernel.recv::<RemoteActuateRetMsg>())
                .await
                .expect("kernel receives actuator ack"),
            actuate_ack
        );
        let write_done = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("kernel flow<fd_write ret>")
            .send(&write_done))
        .await
        .expect("kernel sends fd_write ret");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers remote actuator route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("engine decodes fd_write ret"),
            write_done
        );
        assert!(
            kernel.flow::<RemoteRejectRouteControl>().is_err(),
            "unselected reject arm must not be reachable after actuator route"
        );
        drop(engine);
        drop(kernel);
        drop(sensor);

        let (engine_program, kernel_program) = project_remote_fd_reject_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(219), &engine_program, NoBinding)
            .expect("attach engine reject route projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(219), &kernel_program, NoBinding)
            .expect("attach kernel reject route projection");

        let stale_read = FdRead::new_with_lease(sensor_cap.fd(), 3, 4).expect("stale fd_read");
        let stale_request = EngineReq::FdRead(stale_read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<stale fd_read>")
            .send(&stale_request))
        .await
        .expect("engine sends stale fd_read to kernel role");
        let received_stale = (kernel.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("kernel receives stale fd_read");
        assert_eq!(received_stale, stale_request);
        let EngineReq::FdRead(received_stale) = received_stale else {
            panic!("expected stale fd_read request");
        };
        let rejected = match caps.route_fd_read(
            received_stale.fd(),
            sensor_cap.generation().wrapping_add(1),
            SESSION_GENERATION,
        ) {
            RemoteFdReadRoute::Rejected(error) => error,
            RemoteFdReadRoute::RemoteSensor(_) => {
                panic!("stale generation must select reject route")
            }
        };
        assert_eq!(rejected, RemoteError::BadGeneration);

        (kernel
            .flow::<RemoteRejectRouteControl>()
            .expect("kernel flow<remote reject route>")
            .send(()))
        .await
        .expect("kernel selects reject route");
        let fd_error = FdError::new(received_stale.fd(), 70);
        (kernel
            .flow::<FdErrorMsg>()
            .expect("kernel flow<fd error>")
            .send(&fd_error))
        .await
        .expect("kernel sends fd error");
        let engine_branch = (engine.offer()).await.expect("engine offers reject route");
        assert_eq!(
            (engine_branch.decode::<FdErrorMsg>())
                .await
                .expect("engine decodes fd error"),
            fd_error
        );
        assert!(
            kernel.flow::<RemoteSampleReqMsg>().is_err(),
            "rejected route must not allow remote sample request"
        );
    });
}

#[test]
fn remote_management_object_control_selects_management_route_arm_without_bridge() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, SENSOR, GATEWAY, ACTUATOR];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register management engine rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register management kernel rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register remote management rendezvous");

        let mut caps: RemoteObjectTable<2> = RemoteObjectTable::new();
        let management_cap = caps
            .apply_cap_grant_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                GATEWAY,
                NodeRole::Gateway.bit() as u8,
                19,
                LABEL_MGMT_IMAGE_BEGIN,
                3,
                RemoteRights::Write,
                RemoteResource::Management,
            )
            .expect("install authenticated remote management fd");
        assert_eq!(management_cap.policy_slot(), 3);
        let mut policy_slots: PolicySlotTable<2> = PolicySlotTable::new();
        policy_slots
            .allow(management_cap.policy_slot())
            .expect("allow management policy slot");
        assert_eq!(
            caps.route_fd_write_authorized(
                management_cap.fd(),
                management_cap.generation(),
                management_cap.route_key(),
                &policy_slots,
            ),
            RemoteFdWriteRoute::RemoteManagement(management_cap)
        );

        let (engine_program, kernel_program, management_program) =
            project_remote_management_fd_write_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(225), &engine_program, NoBinding)
            .expect("attach management engine projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(225), &kernel_program, NoBinding)
            .expect("attach management kernel projection");
        let mut management = cluster2
            .enter(rv2, SessionId::new(225), &management_program, NoBinding)
            .expect("attach remote management projection");

        let begin = MgmtImageBegin::new(0, 64, 44);
        let mut begin_wire = [0u8; 16];
        let begin_len = begin
            .encode_into(&mut begin_wire)
            .expect("encode management begin payload");
        let write = FdWrite::new_with_lease(management_cap.fd(), 4, &begin_wire[..begin_len])
            .expect("remote management fd_write");
        let write_request = EngineReq::FdWrite(write);
        (engine
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine flow<management fd_write>")
            .send(&write_request))
        .await
        .expect("engine sends management fd_write");
        let received_write = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("kernel receives management fd_write");
        let EngineReq::FdWrite(received_write) = received_write else {
            panic!("expected management fd_write request");
        };

        let resolved_management = match caps.route_fd_write_authorized(
            received_write.fd(),
            management_cap.generation(),
            management_cap.route_key(),
            &policy_slots,
        ) {
            RemoteFdWriteRoute::RemoteManagement(cap) => cap,
            RemoteFdWriteRoute::RemoteActuator(cap) => {
                panic!("remote management fd selected actuator route: {cap:?}")
            }
            RemoteFdWriteRoute::RemoteTelemetry(cap) => {
                panic!("remote management fd selected telemetry route: {cap:?}")
            }
            RemoteFdWriteRoute::Rejected(error) => {
                panic!("remote management fd should select management route: {error:?}")
            }
        };
        assert_eq!(resolved_management.target_node(), GATEWAY);
        assert_eq!(
            resolved_management.target_role(),
            NodeRole::Gateway.bit() as u8
        );
        assert_eq!(resolved_management.lane(), 19);
        assert_eq!(resolved_management.route(), LABEL_MGMT_IMAGE_BEGIN);

        (kernel
            .flow::<RemoteManagementRouteControl>()
            .expect("kernel flow<remote management route>")
            .send(()))
        .await
        .expect("kernel selects remote management route");
        let decoded_begin = MgmtImageBegin::decode_payload(Payload::new(received_write.as_bytes()))
            .expect("fd payload decodes as management begin");
        assert_eq!(decoded_begin, begin);
        (kernel
            .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
            .expect("kernel flow<management begin>")
            .send(&decoded_begin))
        .await
        .expect("kernel sends management begin");
        let management_branch = (management.offer()).await.expect("management offers route");
        assert_eq!(
            (management_branch.decode::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>())
                .await
                .expect("management decodes begin"),
            begin
        );
        let status = MgmtStatus::new(begin.slot(), MgmtStatusCode::Ok);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("management flow<status>")
            .send(&status))
        .await
        .expect("management sends status");
        assert_eq!(
            (kernel.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("kernel receives management status"),
            status
        );
        let write_done = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("kernel flow<management fd_write ret>")
            .send(&write_done))
        .await
        .expect("kernel returns management fd_write");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers management route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("engine decodes management fd_write ret"),
            write_done
        );
        assert!(
            kernel.flow::<RemoteRejectRouteControl>().is_err(),
            "management fd must not leave reject route reachable"
        );
    });
}

#[test]
fn remote_telemetry_object_control_selects_telemetry_route_arm_without_bridge() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, SENSOR, GATEWAY, ACTUATOR];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register telemetry engine rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register telemetry kernel rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register telemetry gateway rendezvous");

        let mut caps: RemoteObjectTable<2> = RemoteObjectTable::new();
        let telemetry_cap = caps
            .apply_cap_grant_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                GATEWAY,
                NodeRole::Gateway.bit() as u8,
                20,
                LABEL_SWARM_TELEMETRY,
                4,
                RemoteRights::Write,
                RemoteResource::Telemetry,
            )
            .expect("install authenticated remote telemetry fd");
        assert_eq!(telemetry_cap.policy_slot(), 4);
        let mut policy_slots: PolicySlotTable<2> = PolicySlotTable::new();
        policy_slots
            .allow(telemetry_cap.policy_slot())
            .expect("allow telemetry policy slot");
        assert_eq!(
            caps.route_fd_write_authorized(
                telemetry_cap.fd(),
                telemetry_cap.generation(),
                telemetry_cap.route_key(),
                &policy_slots,
            ),
            RemoteFdWriteRoute::RemoteTelemetry(telemetry_cap)
        );

        let (engine_program, kernel_program, gateway_program) =
            project_remote_telemetry_fd_write_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(226), &engine_program, NoBinding)
            .expect("attach telemetry engine projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(226), &kernel_program, NoBinding)
            .expect("attach telemetry kernel projection");
        let mut gateway = cluster2
            .enter(rv2, SessionId::new(226), &gateway_program, NoBinding)
            .expect("attach telemetry gateway projection");

        let telemetry = SwarmTelemetry::new(
            SENSOR,
            RoleMask::single(NodeRole::Sensor),
            2,
            0,
            512,
            36_00,
            SESSION_GENERATION,
        );
        let mut telemetry_wire = [0u8; 16];
        let telemetry_len = telemetry
            .encode_into(&mut telemetry_wire)
            .expect("encode telemetry payload");
        let write =
            FdWrite::new_with_lease(telemetry_cap.fd(), 5, &telemetry_wire[..telemetry_len])
                .expect("remote telemetry fd_write");
        let write_request = EngineReq::FdWrite(write);
        (engine
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine flow<telemetry fd_write>")
            .send(&write_request))
        .await
        .expect("engine sends telemetry fd_write");
        let received_write = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("kernel receives telemetry fd_write");
        let EngineReq::FdWrite(received_write) = received_write else {
            panic!("expected telemetry fd_write request");
        };

        let resolved_telemetry = match caps.route_fd_write_authorized(
            received_write.fd(),
            telemetry_cap.generation(),
            telemetry_cap.route_key(),
            &policy_slots,
        ) {
            RemoteFdWriteRoute::RemoteTelemetry(cap) => cap,
            RemoteFdWriteRoute::RemoteActuator(cap) => {
                panic!("remote telemetry fd selected actuator route: {cap:?}")
            }
            RemoteFdWriteRoute::RemoteManagement(cap) => {
                panic!("remote telemetry fd selected management route: {cap:?}")
            }
            RemoteFdWriteRoute::Rejected(error) => {
                panic!("remote telemetry fd should select telemetry route: {error:?}")
            }
        };
        assert_eq!(resolved_telemetry.target_node(), GATEWAY);
        assert_eq!(
            resolved_telemetry.target_role(),
            NodeRole::Gateway.bit() as u8
        );
        assert_eq!(resolved_telemetry.lane(), 20);
        assert_eq!(resolved_telemetry.route(), LABEL_SWARM_TELEMETRY);

        (kernel
            .flow::<RemoteTelemetryRouteControl>()
            .expect("kernel flow<remote telemetry route>")
            .send(()))
        .await
        .expect("kernel selects remote telemetry route");
        let decoded_telemetry =
            SwarmTelemetry::decode_payload(Payload::new(received_write.as_bytes()))
                .expect("fd payload decodes as telemetry");
        assert_eq!(decoded_telemetry, telemetry);
        (kernel
            .flow::<SwarmTelemetryMsg>()
            .expect("kernel flow<swarm telemetry>")
            .send(&decoded_telemetry))
        .await
        .expect("kernel sends telemetry");
        let gateway_branch = (gateway.offer()).await.expect("gateway offers route");
        assert_eq!(
            (gateway_branch.decode::<SwarmTelemetryMsg>())
                .await
                .expect("gateway decodes telemetry"),
            telemetry
        );

        let write_done = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("kernel flow<telemetry fd_write ret>")
            .send(&write_done))
        .await
        .expect("kernel returns telemetry fd_write");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers telemetry route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("engine decodes telemetry fd_write ret"),
            write_done
        );
        assert!(
            kernel.flow::<RemoteRejectRouteControl>().is_err(),
            "telemetry fd must not leave reject route reachable"
        );
    });
}

#[test]
fn remote_sample_is_wired_through_hibana_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous");

        let (coordinator_program, sensor_program) = project_sample_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(204), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(204), &sensor_program, NoBinding)
            .expect("attach sensor");

        let request = RemoteSampleRequest::new(1, 2, 8);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator swarm flow<sample req>")
            .send(&request))
        .await
        .expect("send sample request over swarm transport");
        let label_hint = medium
            .peek_label(SENSOR)
            .expect("swarm transport exposes RX-ready label hint");
        let mut resolver: PicoInterruptResolver<1, 2, 1> = PicoInterruptResolver::new();
        resolver
            .push_irq(InterruptEvent::TransportRxReady {
                role: 1,
                lane: 0,
                label_hint,
            })
            .expect("queue transport RX-ready IRQ");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::TransportRxReady {
                role: 1,
                lane: 0,
                label_hint,
            }))
        );
        assert_eq!(
            (sensor.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor recv sample over swarm"),
            request
        );

        let sample = RemoteSample::new(8, 0, 1234, 5678);
        (sensor
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor swarm flow<sample ret>")
            .send(&sample))
        .await
        .expect("send sample over swarm transport");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample over swarm"),
            sample
        );
    });
}

#[test]
fn two_node_wifi_ping_pong_is_wired_through_hibana_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmPingPongKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_universe(SwarmPingPongLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmPingPongKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_universe(SwarmPingPongLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous");

        let (coordinator_program, sensor_program) = project_swarm_ping_pong_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(202), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(202), &sensor_program, NoBinding)
            .expect("attach sensor");

        (sensor
            .flow::<Msg<LABEL_SWARM_PING, u8>>()
            .expect("sensor flow<ping>")
            .send(&SWARM_PING_VALUE))
        .await
        .expect("sensor sends ping over swarm");
        let _ping_frame_label = medium
            .peek_label(COORDINATOR)
            .expect("swarm transport exposes ping frame label hint");
        assert_eq!(
            (coordinator.recv::<Msg<LABEL_SWARM_PING, u8>>())
                .await
                .expect("coordinator receives ping"),
            SWARM_PING_VALUE
        );

        (coordinator
            .flow::<Msg<LABEL_SWARM_PONG, u8>>()
            .expect("coordinator flow<pong>")
            .send(&SWARM_PONG_VALUE))
        .await
        .expect("coordinator sends pong over swarm");
        let _pong_frame_label = medium
            .peek_label(SENSOR)
            .expect("swarm transport exposes pong frame label hint");
        assert_eq!(
            (sensor.recv::<Msg<LABEL_SWARM_PONG, u8>>())
                .await
                .expect("sensor receives pong"),
            SWARM_PONG_VALUE
        );
    });
}

#[test]
fn wifi_packet_loss_does_not_create_semantic_fallback_and_requires_explicit_redelivery() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous");

        let (coordinator_program, sensor_program) = project_sample_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(212), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(212), &sensor_program, NoBinding)
            .expect("attach sensor");

        let request = RemoteSampleRequest::new(1, 2, 12);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sample req>")
            .send(&request))
        .await
        .expect("send sample request over swarm");
        let request_frame_label = medium
            .peek_label(SENSOR)
            .expect("swarm transport exposes sample request frame label hint");

        let dropped = medium
            .drop_for(SENSOR)
            .expect("drop the Wi-Fi frame before sensor receives it");
        assert_eq!(dropped.label_hint(), request_frame_label);
        assert_eq!(medium.peek_label(SENSOR), None);

        let mut pending_recv = sensor.recv::<RemoteSampleReqMsg>();
        assert!(
            matches!(poll_once(&mut pending_recv), Poll::Pending),
            "packet loss must leave the typed receive pending instead of inventing a semantic fallback"
        );

        let mut payload = [0u8; SWARM_FRAME_PAYLOAD_CAPACITY];
        let payload_len = request
            .encode_into(&mut payload)
            .expect("encode redelivered sample request");
        let redelivery = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            212,
            SESSION_GENERATION,
            0,
            request_frame_label,
            dropped.seq().wrapping_add(1),
            0,
            &payload[..payload_len],
            SECURE,
        )
        .expect("build transport redelivery frame");
        medium.send(redelivery).expect("deliver redelivery frame");
        assert_eq!(
            (pending_recv)
                .await
                .expect("redelivery lets typed receive progress"),
            request
        );

        let sample = RemoteSample::new(12, 0, 31_000, 100);
        (sensor
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor flow<sample ret after redelivery>")
            .send(&sample))
        .await
        .expect("send sample after redelivery");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample after redelivery"),
            sample
        );
    });
}

#[test]
fn wasip1_fd_write_guest_is_wired_through_hibana_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse stdout guest");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.lease_id(), MEM_LEASE_NONE);
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);

        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous");

        let (coordinator_program, sensor_program) = project_swarm_wasip1_fd_write_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(205), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(205), &sensor_program, NoBinding)
            .expect("attach sensor");

        let mut leases: MemoryLeaseTable<2> =
            MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
        let borrow = MemBorrow::new(TEST_STDOUT_PTR, chunk.len() as u8, TEST_MEMORY_EPOCH);
        (sensor
            .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
            .expect("sensor flow<mem borrow read>")
            .send(&borrow))
        .await
        .expect("sensor send memory borrow over swarm");
        assert_eq!(
            (coordinator.recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>())
                .await
                .expect("coordinator recv memory borrow"),
            borrow
        );
        let grant = leases.grant_read(borrow).expect("grant read lease");
        assert_eq!(grant.rights(), MemRights::Read);
        (coordinator
            .flow::<MemReadGrantControl>()
            .expect("coordinator flow<read grant>")
            .send(()))
        .await
        .expect("coordinator send read grant over swarm");
        let received_grant = (sensor.recv::<MemReadGrantControl>())
            .await
            .expect("sensor recv read grant");
        let (rights, lease_id) = received_grant
            .decode_handle()
            .expect("decode read lease token");
        assert_eq!(rights, MemRights::Read.tag());
        assert_eq!(lease_id as u8, grant.lease_id());

        let write = FdWrite::new_with_lease(TEST_STDOUT_FD, lease_id as u8, chunk.as_bytes())
            .expect("fd_write request");
        let request = EngineReq::FdWrite(write);
        (sensor
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("sensor flow<fd_write>")
            .send(&request))
        .await
        .expect("sensor send fd_write over swarm");
        let received = (coordinator.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("coordinator recv fd_write");
        assert_eq!(received, request);
        let EngineReq::FdWrite(received_write) = received else {
            panic!("expected fd_write request");
        };
        assert_eq!(received_write.fd(), TEST_STDOUT_FD);
        assert_eq!(received_write.lease_id(), grant.lease_id());
        assert_eq!(received_write.as_bytes(), TEST_STDOUT_TEXT);

        let reply = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (coordinator
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("coordinator flow<fd_write ret>")
            .send(&reply))
        .await
        .expect("coordinator send fd_write ret over swarm");
        assert_eq!(
            (sensor.recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("sensor recv fd_write ret"),
            reply
        );

        let release = MemRelease::new(grant.lease_id());
        (sensor
            .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
            .expect("sensor flow<mem release>")
            .send(&release))
        .await
        .expect("sensor send memory release over swarm");
        assert_eq!(
            (coordinator.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>())
                .await
                .expect("coordinator recv memory release"),
            release
        );
        leases.release(release).expect("release read lease");
    });
}

async fn exchange_swarm_sample<const ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    sensor: &mut Endpoint<'_, ROLE>,
    sensor_node: NodeId,
    value: u32,
    timestamp: u64,
) -> RemoteSample {
    let request = RemoteSampleRequest::new(1, 1, sensor_node.raw() as u8);
    (coordinator
        .flow::<RemoteSampleReqMsg>()
        .expect("coordinator flow<sample req>")
        .send(&request))
    .await
    .expect("send sample request");
    assert_eq!(
        (sensor.recv::<RemoteSampleReqMsg>())
            .await
            .expect("sensor recv sample req"),
        request
    );
    let sample = RemoteSample::new(sensor_node.raw() as u8, 0, value, timestamp);
    (sensor
        .flow::<RemoteSampleRetMsg>()
        .expect("sensor flow<sample ret>")
        .send(&sample))
    .await
    .expect("send sample");
    assert_eq!(
        (coordinator.recv::<RemoteSampleRetMsg>())
            .await
            .expect("coordinator recv sample"),
        sample
    );
    sample
}

async fn exchange_swarm_aggregate<const ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    sensor: &mut Endpoint<'_, ROLE>,
    sensor_node: NodeId,
    aggregate: u32,
) {
    let command = RemoteActuateRequest::new(2, 1, sensor_node.raw() as u8, aggregate);
    (coordinator
        .flow::<RemoteActuateReqMsg>()
        .expect("coordinator flow<aggregate>")
        .send(&command))
    .await
    .expect("send aggregate");
    assert_eq!(
        (sensor.recv::<RemoteActuateReqMsg>())
            .await
            .expect("sensor recv aggregate"),
        command
    );
    let ack = RemoteActuateAck::new(sensor_node.raw() as u8, 0);
    (sensor
        .flow::<RemoteActuateRetMsg>()
        .expect("sensor flow<aggregate ack>")
        .send(&ack))
    .await
    .expect("send aggregate ack");
    assert_eq!(
        (coordinator.recv::<RemoteActuateRetMsg>())
            .await
            .expect("coordinator recv aggregate ack"),
        ack
    );
}

#[cfg(feature = "profile-host-qemu-swarm")]
async fn exchange_swarm_remote_actuator<const ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    actuator: &mut Endpoint<'_, ROLE>,
    actuator_node: NodeId,
    value: u32,
) {
    let command = RemoteActuateRequest::new(
        QEMU_REMOTE_ACTUATOR_FD,
        SESSION_GENERATION,
        actuator_node.raw() as u8,
        value,
    );
    (coordinator
        .flow::<RemoteActuateReqMsg>()
        .expect("coordinator flow<remote actuator>")
        .send(&command))
    .await
    .expect("send remote actuator command over swarm");
    assert_eq!(
        (actuator.recv::<RemoteActuateReqMsg>())
            .await
            .expect("actuator recv remote command"),
        command
    );
    let ack = RemoteActuateAck::new(actuator_node.raw() as u8, 0);
    (actuator
        .flow::<RemoteActuateRetMsg>()
        .expect("actuator flow<remote ack>")
        .send(&ack))
    .await
    .expect("send remote actuator ack");
    assert_eq!(
        (coordinator.recv::<RemoteActuateRetMsg>())
            .await
            .expect("coordinator recv remote actuator ack"),
        ack
    );
}

#[cfg(feature = "profile-host-qemu-swarm")]
async fn exchange_swarm_gateway_telemetry<const SOURCE_ROLE: u8, const GATEWAY_ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    source: &mut Endpoint<'_, SOURCE_ROLE>,
    gateway: &mut Endpoint<'_, GATEWAY_ROLE>,
    source_node: NodeId,
) {
    let telemetry = SwarmTelemetry::new(
        source_node,
        RoleMask::single(NodeRole::Actuator),
        1,
        0,
        512,
        23_500,
        SESSION_GENERATION,
    );
    (source
        .flow::<SwarmTelemetryMsg>()
        .expect("source flow<gateway telemetry>")
        .send(&telemetry))
    .await
    .expect("source sends gateway telemetry");
    assert_eq!(
        (gateway.recv::<SwarmTelemetryMsg>())
            .await
            .expect("gateway receives telemetry"),
        telemetry
    );
    (gateway
        .flow::<SwarmTelemetryMsg>()
        .expect("gateway flow<telemetry acceptance>")
        .send(&telemetry))
    .await
    .expect("gateway forwards telemetry acceptance");
    assert_eq!(
        (coordinator.recv::<SwarmTelemetryMsg>())
            .await
            .expect("coordinator receives telemetry acceptance"),
        telemetry
    );
}

#[cfg(feature = "profile-host-qemu-swarm")]
async fn exchange_qemu_network_object_route<const GATEWAY_ROLE: u8>(
    coordinator: &mut Endpoint<'_, 0>,
    gateway: &mut Endpoint<'_, GATEWAY_ROLE>,
    gateway_node: NodeId,
) {
    let mut fds: NetworkObjectTable<2> = NetworkObjectTable::new();
    let datagram_fd = fds
        .apply_cap_grant_datagram(
            COORDINATOR,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            gateway_node,
            22,
            LABEL_NET_DATAGRAM_SEND,
            NetworkRights::Send,
        )
        .expect("grant qemu datagram NetworkObject");
    let stream_fd = fds
        .apply_cap_grant_stream(
            COORDINATOR,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            gateway_node,
            23,
            LABEL_NET_STREAM_WRITE,
            NetworkRights::Send,
        )
        .expect("grant qemu stream NetworkObject");
    let mut gateway_fds: NetworkObjectTable<3> = NetworkObjectTable::new();
    let _gateway_dummy_fd = gateway_fds
        .apply_cap_grant_datagram(
            gateway_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            COORDINATOR,
            21,
            LABEL_NET_DATAGRAM_RECV,
            NetworkRights::Receive,
        )
        .expect("pre-seed gateway table so inbound routing is not fd-order dependent");
    let gateway_datagram_fd = gateway_fds
        .apply_cap_grant_datagram(
            gateway_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            COORDINATOR,
            22,
            LABEL_NET_DATAGRAM_SEND,
            NetworkRights::Receive,
        )
        .expect("grant gateway datagram receive NetworkObject");
    let gateway_stream_fd = gateway_fds
        .apply_cap_grant_stream(
            gateway_node,
            SWARM_CREDENTIAL,
            SESSION_GENERATION,
            COORDINATOR,
            23,
            LABEL_NET_STREAM_WRITE,
            NetworkRights::Receive,
        )
        .expect("grant gateway stream receive NetworkObject");

    assert_ne!(
        datagram_fd.generation(),
        SESSION_GENERATION,
        "NetworkObject generation is distinct from session generation"
    );
    let resolved_datagram = match fds.route_fd_write_routed(
        datagram_fd.fd(),
        datagram_fd.generation(),
        datagram_fd.route_key(),
    ) {
        NetworkObjectWriteRoute::Datagram(fd) => fd,
        other => panic!("datagram fd should choose datagram route: {other:?}"),
    };
    assert_eq!(resolved_datagram.target_node(), gateway_node);
    assert_eq!(resolved_datagram.lane(), 22);
    assert_eq!(resolved_datagram.route(), LABEL_NET_DATAGRAM_SEND);
    assert_ne!(resolved_datagram.route(), gateway_node.raw() as u8);

    (coordinator
        .flow::<NetworkDatagramSendRouteControl>()
        .expect("coordinator flow<qemu datagram route control>")
        .send(()))
    .await
    .expect("coordinator selects qemu datagram route");
    let datagram = DatagramSend::new(
        resolved_datagram.fd(),
        resolved_datagram.generation(),
        resolved_datagram.route(),
        fds.allocate_operation_id(),
        b"qemu datagram fd",
    )
    .expect("qemu datagram send");
    (coordinator
        .flow::<DatagramSendMsg>()
        .expect("coordinator flow<qemu datagram send>")
        .send(&datagram))
    .await
    .expect("coordinator sends qemu datagram over swarm");
    let gateway_branch = (gateway.offer())
        .await
        .expect("gateway offers qemu datagram route");
    assert_eq!(
        (gateway_branch.decode::<DatagramSendMsg>())
            .await
            .expect("gateway receives qemu datagram"),
        datagram
    );
    let gateway_datagram = match gateway_fds.route_receive_routed(NetworkRoute::new(
        COORDINATOR,
        22,
        LABEL_NET_DATAGRAM_SEND,
        SESSION_GENERATION,
    )) {
        NetworkObjectReadRoute::Datagram(fd) => fd,
        other => panic!("gateway datagram receive should choose datagram route: {other:?}"),
    };
    assert_eq!(gateway_datagram, gateway_datagram_fd);
    assert_ne!(gateway_datagram.fd(), resolved_datagram.fd());
    assert_ne!(
        gateway_datagram.generation(),
        resolved_datagram.generation()
    );
    assert_eq!(gateway_datagram.target_node(), COORDINATOR);
    assert_eq!(gateway_datagram.route(), datagram.route());
    assert_eq!(
        gateway_fds.route_receive_routed(NetworkRoute::new(
            COORDINATOR,
            22,
            LABEL_NET_STREAM_WRITE,
            SESSION_GENERATION,
        )),
        NetworkObjectReadRoute::Rejected(NetworkError::BadRoute)
    );
    let datagram_ack = DatagramAck::new(
        datagram.fd(),
        datagram.generation(),
        datagram.operation_id(),
        true,
    );
    assert!(fds.datagram_ack_accepted_for_route(
        resolved_datagram,
        datagram_ack,
        datagram.operation_id(),
    ));
    assert!(
        !DatagramAck::new(
            resolved_datagram.fd() ^ 1,
            resolved_datagram.generation(),
            datagram.operation_id(),
            true
        )
        .accepted_for(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id()
        )
    );
    assert!(
        !DatagramAck::new(
            resolved_datagram.fd(),
            resolved_datagram.generation().wrapping_add(1),
            datagram.operation_id(),
            true,
        )
        .accepted_for(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id()
        )
    );
    assert!(
        !DatagramAck::new(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id() ^ 1,
            true,
        )
        .accepted_for(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id()
        )
    );
    assert!(
        !DatagramAck::new(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id(),
            false
        )
        .accepted_for(
            resolved_datagram.fd(),
            resolved_datagram.generation(),
            datagram.operation_id()
        )
    );
    (gateway
        .flow::<DatagramAckMsg>()
        .expect("gateway flow<qemu datagram ack>")
        .send(&datagram_ack))
    .await
    .expect("gateway sends qemu datagram ack");
    assert_eq!(
        (coordinator.recv::<DatagramAckMsg>())
            .await
            .expect("coordinator receives qemu datagram ack"),
        datagram_ack
    );

    let resolved_stream = match fds.route_fd_write_routed(
        stream_fd.fd(),
        stream_fd.generation(),
        stream_fd.route_key(),
    ) {
        NetworkObjectWriteRoute::Stream(fd) => fd,
        other => panic!("stream fd should choose stream route: {other:?}"),
    };
    assert_eq!(resolved_stream.target_node(), gateway_node);
    assert_eq!(resolved_stream.lane(), 23);
    assert_eq!(resolved_stream.route(), LABEL_NET_STREAM_WRITE);
    assert_ne!(resolved_stream.route(), gateway_node.raw() as u8);

    (coordinator
        .flow::<NetworkStreamWriteRouteControl>()
        .expect("coordinator flow<qemu stream route control>")
        .send(()))
    .await
    .expect("coordinator selects qemu stream route");
    let stream = StreamWrite::new(
        resolved_stream.fd(),
        resolved_stream.generation(),
        resolved_stream.route(),
        fds.allocate_operation_id(),
        0,
        NET_STREAM_FLAG_FIN,
        b"qemu stream fd",
    )
    .expect("qemu stream write");
    (coordinator
        .flow::<StreamWriteMsg>()
        .expect("coordinator flow<qemu stream write>")
        .send(&stream))
    .await
    .expect("coordinator sends qemu stream over swarm");
    assert_eq!(
        (gateway.recv::<StreamWriteMsg>())
            .await
            .expect("gateway receives qemu stream"),
        stream
    );
    let gateway_stream = match gateway_fds.route_receive_routed(NetworkRoute::new(
        COORDINATOR,
        23,
        LABEL_NET_STREAM_WRITE,
        SESSION_GENERATION,
    )) {
        NetworkObjectReadRoute::Stream(fd) => fd,
        other => panic!("gateway stream receive should choose stream route: {other:?}"),
    };
    assert_eq!(gateway_stream, gateway_stream_fd);
    assert_ne!(gateway_stream.fd(), resolved_stream.fd());
    assert_ne!(gateway_stream.generation(), resolved_stream.generation());
    assert_eq!(gateway_stream.target_node(), COORDINATOR);
    assert_eq!(gateway_stream.route(), stream.route());
    assert_eq!(
        gateway_fds.route_receive_routed(NetworkRoute::new(
            COORDINATOR,
            23,
            LABEL_NET_DATAGRAM_SEND,
            SESSION_GENERATION,
        )),
        NetworkObjectReadRoute::Rejected(NetworkError::BadRoute)
    );
    let stream_ack = StreamAck::new(
        stream.fd(),
        stream.generation(),
        stream.operation_id(),
        stream.sequence(),
        true,
    );
    assert!(fds.stream_ack_accepted_for_route(
        resolved_stream,
        stream_ack,
        stream.operation_id(),
        stream.sequence(),
    ));
    assert!(
        !StreamAck::new(
            resolved_stream.fd() ^ 1,
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence(),
            true,
        )
        .accepted_for(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence()
        )
    );
    assert!(
        !StreamAck::new(
            resolved_stream.fd(),
            resolved_stream.generation().wrapping_add(1),
            stream.operation_id(),
            stream.sequence(),
            true,
        )
        .accepted_for(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence()
        )
    );
    assert!(
        !StreamAck::new(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id() ^ 1,
            stream.sequence(),
            true,
        )
        .accepted_for(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence()
        )
    );
    assert!(
        !StreamAck::new(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence().wrapping_add(1),
            true,
        )
        .accepted_for(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence()
        )
    );
    assert!(
        !StreamAck::new(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence(),
            false,
        )
        .accepted_for(
            resolved_stream.fd(),
            resolved_stream.generation(),
            stream.operation_id(),
            stream.sequence()
        )
    );
    (gateway
        .flow::<StreamAckMsg>()
        .expect("gateway flow<qemu stream ack>")
        .send(&stream_ack))
    .await
    .expect("gateway sends qemu stream ack");
    assert_eq!(
        (coordinator.recv::<StreamAckMsg>())
            .await
            .expect("coordinator receives qemu stream ack"),
        stream_ack
    );
}

#[test]
fn one_choreography_connects_all_swarm_nodes_with_sample_wasi_and_aggregate() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse stdout guest");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);

        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, SENSOR, ACTUATOR, NodeId::new(4)];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register sensor rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    ACTUATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register actuator rendezvous");

        let node4 = NodeId::new(4);
        let clock3 = CounterClock::new();
        let mut tap3 = [TapEvent::zero(); 128];
        let mut slab3 = vec![0u8; 262_144];
        let cluster3 = SwarmRoleTestKit::new(&clock3);
        let rv3 = cluster3
            .add_rendezvous_from_config(
                Config::new(&mut tap3, slab3.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node4,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node4 rendezvous");

        let (program0, program1, program2, program3) = project_global_swarm_roles_4();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(2350), &program0, NoBinding)
            .expect("attach coordinator");
        let mut sensor1 = cluster1
            .enter(rv1, SessionId::new(2350), &program1, NoBinding)
            .expect("attach sensor1");
        let mut sensor2 = cluster2
            .enter(rv2, SessionId::new(2350), &program2, NoBinding)
            .expect("attach sensor2");
        let mut sensor3 = cluster3
            .enter(rv3, SessionId::new(2350), &program3, NoBinding)
            .expect("attach sensor3");

        let samples = [
            (SENSOR, pico2w_swarm_sample_value(SENSOR.raw())),
            (ACTUATOR, pico2w_swarm_sample_value(ACTUATOR.raw())),
            (node4, pico2w_swarm_sample_value(node4.raw())),
        ];

        let request = RemoteSampleRequest::new(1, 1, SENSOR.raw() as u8);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sample req 1>")
            .send(&request))
        .await
        .expect("send sample request 1");
        assert_eq!(
            (sensor1.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor1 recv sample req"),
            request
        );
        let sample = RemoteSample::new(SENSOR.raw() as u8, 0, samples[0].1, 2350);
        (sensor1
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor1 flow<sample ret>")
            .send(&sample))
        .await
        .expect("send sample 1");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample 1"),
            sample
        );

        let request = RemoteSampleRequest::new(1, 1, ACTUATOR.raw() as u8);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sample req 2>")
            .send(&request))
        .await
        .expect("send sample request 2");
        assert_eq!(
            (sensor2.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor2 recv sample req"),
            request
        );
        let sample = RemoteSample::new(ACTUATOR.raw() as u8, 0, samples[1].1, 2351);
        (sensor2
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor2 flow<sample ret>")
            .send(&sample))
        .await
        .expect("send sample 2");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample 2"),
            sample
        );

        let request = RemoteSampleRequest::new(1, 1, node4.raw() as u8);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sample req 3>")
            .send(&request))
        .await
        .expect("send sample request 3");
        assert_eq!(
            (sensor3.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor3 recv sample req"),
            request
        );
        let sample = RemoteSample::new(node4.raw() as u8, 0, samples[2].1, 2352);
        (sensor3
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor3 flow<sample ret>")
            .send(&sample))
        .await
        .expect("send sample 3");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator recv sample 3"),
            sample
        );

        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor1, SENSOR, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor2, ACTUATOR, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor3, node4, chunk.as_bytes())
            .await;

        let aggregate = samples
            .iter()
            .fold(0u32, |sum, (_node, value)| sum.wrapping_add(*value));
        let command = RemoteActuateRequest::new(2, 1, SENSOR.raw() as u8, aggregate);
        (coordinator
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<aggregate 1>")
            .send(&command))
        .await
        .expect("send aggregate 1");
        assert_eq!(
            (sensor1.recv::<RemoteActuateReqMsg>())
                .await
                .expect("sensor1 recv aggregate"),
            command
        );
        let ack = RemoteActuateAck::new(SENSOR.raw() as u8, 0);
        (sensor1
            .flow::<RemoteActuateRetMsg>()
            .expect("sensor1 flow<aggregate ack>")
            .send(&ack))
        .await
        .expect("send aggregate ack 1");
        assert_eq!(
            (coordinator.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator recv aggregate ack 1"),
            ack
        );

        let command = RemoteActuateRequest::new(2, 1, ACTUATOR.raw() as u8, aggregate);
        (coordinator
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<aggregate 2>")
            .send(&command))
        .await
        .expect("send aggregate 2");
        assert_eq!(
            (sensor2.recv::<RemoteActuateReqMsg>())
                .await
                .expect("sensor2 recv aggregate"),
            command
        );
        let ack = RemoteActuateAck::new(ACTUATOR.raw() as u8, 0);
        (sensor2
            .flow::<RemoteActuateRetMsg>()
            .expect("sensor2 flow<aggregate ack>")
            .send(&ack))
        .await
        .expect("send aggregate ack 2");
        assert_eq!(
            (coordinator.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator recv aggregate ack 2"),
            ack
        );

        let command = RemoteActuateRequest::new(2, 1, node4.raw() as u8, aggregate);
        (coordinator
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<aggregate 3>")
            .send(&command))
        .await
        .expect("send aggregate 3");
        assert_eq!(
            (sensor3.recv::<RemoteActuateReqMsg>())
                .await
                .expect("sensor3 recv aggregate"),
            command
        );
        let ack = RemoteActuateAck::new(node4.raw() as u8, 0);
        (sensor3
            .flow::<RemoteActuateRetMsg>()
            .expect("sensor3 flow<aggregate ack>")
            .send(&ack))
        .await
        .expect("send aggregate ack 3");
        assert_eq!(
            (coordinator.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator recv aggregate ack 3"),
            ack
        );
    });
}

#[test]
fn six_process_swarm_choreography_connects_coordinator_and_five_sensors() {
    hibana_pico::port::exec::run_current_task(async {
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse stdout guest");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);

        let medium: HostSwarmMedium<192> = HostSwarmMedium::new();
        let node2 = NodeId::new(2);
        let node3 = NodeId::new(3);
        let node4 = NodeId::new(4);
        let node5 = NodeId::new(5);
        let node6 = NodeId::new(6);
        let role_nodes = [COORDINATOR, node2, node3, node4, node5, node6];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmSixRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmSixRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node2,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node2 rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmSixRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node3,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node3 rendezvous");

        let clock3 = CounterClock::new();
        let mut tap3 = [TapEvent::zero(); 128];
        let mut slab3 = vec![0u8; 262_144];
        let cluster3 = SwarmSixRoleTestKit::new(&clock3);
        let rv3 = cluster3
            .add_rendezvous_from_config(
                Config::new(&mut tap3, slab3.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node4,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node4 rendezvous");

        let clock4 = CounterClock::new();
        let mut tap4 = [TapEvent::zero(); 128];
        let mut slab4 = vec![0u8; 262_144];
        let cluster4 = SwarmSixRoleTestKit::new(&clock4);
        let rv4 = cluster4
            .add_rendezvous_from_config(
                Config::new(&mut tap4, slab4.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node5,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node5 rendezvous");

        let clock5 = CounterClock::new();
        let mut tap5 = [TapEvent::zero(); 128];
        let mut slab5 = vec![0u8; 262_144];
        let cluster5 = SwarmSixRoleTestKit::new(&clock5);
        let rv5 = cluster5
            .add_rendezvous_from_config(
                Config::new(&mut tap5, slab5.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node6,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register node6 rendezvous");

        let (program0, program1, program2, program3, program4, program5) =
            project_global_swarm_roles_6();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(2360), &program0, NoBinding)
            .expect("attach coordinator");
        let mut sensor1 = cluster1
            .enter(rv1, SessionId::new(2360), &program1, NoBinding)
            .expect("attach sensor1");
        let mut sensor2 = cluster2
            .enter(rv2, SessionId::new(2360), &program2, NoBinding)
            .expect("attach sensor2");
        let mut sensor3 = cluster3
            .enter(rv3, SessionId::new(2360), &program3, NoBinding)
            .expect("attach sensor3");
        let mut sensor4 = cluster4
            .enter(rv4, SessionId::new(2360), &program4, NoBinding)
            .expect("attach sensor4");
        let mut sensor5 = cluster5
            .enter(rv5, SessionId::new(2360), &program5, NoBinding)
            .expect("attach sensor5");

        let samples = [
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor1,
                node2,
                pico2w_swarm_sample_value(node2.raw()),
                2360,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor2,
                node3,
                pico2w_swarm_sample_value(node3.raw()),
                2361,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor3,
                node4,
                pico2w_swarm_sample_value(node4.raw()),
                2362,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor4,
                node5,
                pico2w_swarm_sample_value(node5.raw()),
                2363,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor5,
                node6,
                pico2w_swarm_sample_value(node6.raw()),
                2364,
            )
            .await,
        ];

        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor1, node2, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor2, node3, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor3, node4, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor4, node5, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor5, node6, chunk.as_bytes())
            .await;

        let aggregate = samples
            .iter()
            .fold(0u32, |sum, sample| sum.wrapping_add(sample.value()));
        assert_eq!(aggregate, PICO2W_SWARM_DEFAULT_AGGREGATE);

        exchange_swarm_aggregate(&mut coordinator, &mut sensor1, node2, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor2, node3, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor3, node4, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor4, node5, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor5, node6, aggregate).await;
    });
}

#[test]
#[cfg(feature = "profile-host-qemu-swarm")]
fn production_qemu_swarm_routes_network_objects_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        // This host proof exercises the production swarm choreography path. The
        // patched CYW43439 UDP overlay is covered by scripts/run_pico2w_swarm_qemu.sh.
        let module = Wasip1StdoutModule::parse(WASIP1_STDOUT_GUEST).expect("parse stdout guest");
        let chunk = module
            .stdout_chunk_for(TEST_STDOUT_TEXT)
            .expect("stdout chunk");
        assert_eq!(chunk.as_bytes(), TEST_STDOUT_TEXT);

        let medium: HostSwarmMedium<192> = HostSwarmMedium::new();
        let node2 = NodeId::new(2);
        let node3 = NodeId::new(3);
        let node4 = NodeId::new(4);
        let node5 = NodeId::new(5);
        let node6 = NodeId::new(6);
        let role_nodes = [COORDINATOR, node2, node3, node4, node5, node6];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmSixRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmSixRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node2,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu node2 rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmSixRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node3,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu node3 rendezvous");

        let clock3 = CounterClock::new();
        let mut tap3 = [TapEvent::zero(); 128];
        let mut slab3 = vec![0u8; 262_144];
        let cluster3 = SwarmSixRoleTestKit::new(&clock3);
        let rv3 = cluster3
            .add_rendezvous_from_config(
                Config::new(&mut tap3, slab3.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node4,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu node4 rendezvous");

        let clock4 = CounterClock::new();
        let mut tap4 = [TapEvent::zero(); 128];
        let mut slab4 = vec![0u8; 262_144];
        let cluster4 = SwarmSixRoleTestKit::new(&clock4);
        let rv4 = cluster4
            .add_rendezvous_from_config(
                Config::new(&mut tap4, slab4.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node5,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu node5 rendezvous");

        let clock5 = CounterClock::new();
        let mut tap5 = [TapEvent::zero(); 128];
        let mut slab5 = vec![0u8; 262_144];
        let cluster5 = SwarmSixRoleTestKit::new(&clock5);
        let rv5 = cluster5
            .add_rendezvous_from_config(
                Config::new(&mut tap5, slab5.as_mut_slice())
                    .with_lane_range(0..25)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    node6,
                    role_nodes,
                    6,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register qemu node6 rendezvous");

        let mut coordinator = cluster0
            .enter(
                rv0,
                SessionId::new(2361),
                coordinator_program_6(),
                NoBinding,
            )
            .expect("attach production coordinator");
        let mut sensor1 = cluster1
            .enter(rv1, SessionId::new(2361), role1_program_6(), NoBinding)
            .expect("attach production node2");
        let mut sensor2 = cluster2
            .enter(rv2, SessionId::new(2361), role2_program_6(), NoBinding)
            .expect("attach production node3");
        let mut sensor3 = cluster3
            .enter(rv3, SessionId::new(2361), role3_program_6(), NoBinding)
            .expect("attach production node4");
        let mut sensor4 = cluster4
            .enter(rv4, SessionId::new(2361), role4_program_6(), NoBinding)
            .expect("attach production node5");
        let mut sensor5 = cluster5
            .enter(rv5, SessionId::new(2361), role5_program_6(), NoBinding)
            .expect("attach production node6");

        let samples = [
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor1,
                node2,
                pico2w_swarm_sample_value(node2.raw()),
                2360,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor2,
                node3,
                pico2w_swarm_sample_value(node3.raw()),
                2361,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor3,
                node4,
                pico2w_swarm_sample_value(node4.raw()),
                2362,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor4,
                node5,
                pico2w_swarm_sample_value(node5.raw()),
                2363,
            )
            .await,
            exchange_swarm_sample(
                &mut coordinator,
                &mut sensor5,
                node6,
                pico2w_swarm_sample_value(node6.raw()),
                2364,
            )
            .await,
        ];

        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor1, node2, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor2, node3, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor3, node4, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor4, node5, chunk.as_bytes())
            .await;
        exchange_swarm_wasip1_fd_write(&mut coordinator, &mut sensor5, node6, chunk.as_bytes())
            .await;

        let aggregate = samples
            .iter()
            .fold(0u32, |sum, sample| sum.wrapping_add(sample.value()));
        assert_eq!(aggregate, PICO2W_SWARM_DEFAULT_AGGREGATE);

        exchange_swarm_aggregate(&mut coordinator, &mut sensor1, node2, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor2, node3, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor3, node4, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor4, node5, aggregate).await;
        exchange_swarm_aggregate(&mut coordinator, &mut sensor5, node6, aggregate).await;

        exchange_swarm_remote_actuator(
            &mut coordinator,
            &mut sensor2,
            node3,
            aggregate ^ 0x0000_a5a5,
        )
        .await;
        exchange_swarm_gateway_telemetry(&mut coordinator, &mut sensor2, &mut sensor3, node3).await;
        exchange_qemu_network_object_route(&mut coordinator, &mut sensor3, node4).await;
    });
}

#[test]
#[cfg(feature = "profile-host-qemu-swarm")]
fn qemu_mesh_udp_source_ports_are_exclusive_node_bindings() {
    let source = std::fs::read_to_string("qemu/overlay/hw/misc/cyw43439_wifi.c")
        .expect("read QEMU CYW43439 overlay");

    assert!(
        source.contains("if (!cyw43439_radio_mesh_enabled(s)) {\n        socket_set_fast_reuse(s->radio_fd);\n    }"),
        "mesh sockets must not enable fast reuse because source ports bind node identity"
    );
    assert!(
        source.contains("s->radio_port_base > UINT16_MAX - s->node_count"),
        "mesh port allocation must reject radio-port-base overflow"
    );
    assert!(
        source.contains("source_addr.sin_addr.s_addr != htonl(INADDR_LOOPBACK)"),
        "mesh receive must reject non-127.0.0.1 loopback aliases before trusting source ports"
    );
    assert!(
        source.contains("source_node = source_port - s->radio_port_base;")
            && source.contains("frame_src_node != source_node || frame_dst_node != s->node_id"),
        "mesh receive must bind UDP source port, frame src, and frame dst before queueing"
    );
}

#[test]
#[cfg(feature = "profile-host-qemu-swarm")]
fn qemu_swarm_runtime_checks_transport_rx_metadata_for_network_objects() {
    let source = std::fs::read_to_string("src/projects/pico2w_swarm/runtime/mod.rs")
        .expect("read Pico 2 W swarm runtime");

    assert!(
        source.contains("if !meta.matches(source_node, local_node, lane)"),
        "QEMU RX metadata checks must bind source node, destination node, and lane"
    );
    assert!(
        source.contains(
            "expect_qemu_rx_meta(\n        ROLE,\n        local_node,\n        datagram_route.target_node(),\n        datagram_route.lane(),"
        ),
        "datagram NetworkObject receive must verify the transport source and lane"
    );
    assert!(
        source.contains(
            "expect_qemu_rx_meta(\n        ROLE,\n        local_node,\n        stream_route.target_node(),\n        stream_route.lane(),"
        ),
        "stream NetworkObject receive must verify the transport source and lane"
    );
}

#[test]
fn one_choreography_connects_sensor_actuator_and_gateway_telemetry() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, SENSOR, ACTUATOR, GATEWAY];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register sensor rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    ACTUATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register actuator rendezvous");

        let clock3 = CounterClock::new();
        let mut tap3 = [TapEvent::zero(); 128];
        let mut slab3 = vec![0u8; 262_144];
        let cluster3 = SwarmRoleTestKit::new(&clock3);
        let rv3 = cluster3
            .add_rendezvous_from_config(
                Config::new(&mut tap3, slab3.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register gateway rendezvous");

        let (program0, program1, program2, program3) = project_sensor_actuator_gateway_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(213), &program0, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(213), &program1, NoBinding)
            .expect("attach sensor");
        let mut actuator = cluster2
            .enter(rv2, SessionId::new(213), &program2, NoBinding)
            .expect("attach actuator");
        let mut gateway = cluster3
            .enter(rv3, SessionId::new(213), &program3, NoBinding)
            .expect("attach gateway");

        let request = RemoteSampleRequest::new(1, 1, SENSOR.raw() as u8);
        (coordinator
            .flow::<RemoteSampleReqMsg>()
            .expect("coordinator flow<sensor sample req>")
            .send(&request))
        .await
        .expect("coordinator sends sensor sample req");
        assert_eq!(
            (sensor.recv::<RemoteSampleReqMsg>())
                .await
                .expect("sensor receives sample req"),
            request
        );

        let sample = RemoteSample::new(
            SENSOR.raw() as u8,
            0,
            pico2w_swarm_sample_value(SENSOR.raw()),
            2130,
        );
        (sensor
            .flow::<RemoteSampleRetMsg>()
            .expect("sensor flow<sample ret>")
            .send(&sample))
        .await
        .expect("sensor sends sample");
        assert_eq!(
            (coordinator.recv::<RemoteSampleRetMsg>())
                .await
                .expect("coordinator receives sample"),
            sample
        );

        let actuate = RemoteActuateRequest::new(2, 1, ACTUATOR.raw() as u8, sample.value());
        (coordinator
            .flow::<RemoteActuateReqMsg>()
            .expect("coordinator flow<actuator command>")
            .send(&actuate))
        .await
        .expect("coordinator sends actuator command");
        assert_eq!(
            (actuator.recv::<RemoteActuateReqMsg>())
                .await
                .expect("actuator receives command"),
            actuate
        );

        let ack = RemoteActuateAck::new(ACTUATOR.raw() as u8, 0);
        (actuator
            .flow::<RemoteActuateRetMsg>()
            .expect("actuator flow<ack>")
            .send(&ack))
        .await
        .expect("actuator sends ack");
        assert_eq!(
            (coordinator.recv::<RemoteActuateRetMsg>())
                .await
                .expect("coordinator receives ack"),
            ack
        );

        let telemetry = SwarmTelemetry::new(
            COORDINATOR,
            RoleMask::single(NodeRole::Coordinator),
            1,
            0,
            64,
            2_600,
            SESSION_GENERATION,
        );
        assert!(!telemetry.blocks_runtime_authority());
        (coordinator
            .flow::<SwarmTelemetryMsg>()
            .expect("coordinator flow<gateway telemetry>")
            .send(&telemetry))
        .await
        .expect("coordinator sends telemetry to gateway");
        let _telemetry_frame_label = medium
            .peek_label(GATEWAY)
            .expect("swarm transport exposes telemetry frame label hint");
        assert_eq!(
            (gateway.recv::<SwarmTelemetryMsg>())
                .await
                .expect("gateway receives telemetry"),
            telemetry
        );
    });
}

#[test]
fn swarm_wrong_payload_and_wrong_localside_label_reject() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous");

        let (coordinator_program, sensor_program) = project_sample_roles();
        let _coordinator = cluster0
            .enter(rv0, SessionId::new(214), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(214), &sensor_program, NoBinding)
            .expect("attach sensor");

        let wrong_payload = RemoteActuateRequest::new(2, 1, ACTUATOR.raw() as u8, 0xdead_beef);
        let mut payload = [0u8; SWARM_FRAME_PAYLOAD_CAPACITY];
        let payload_len = wrong_payload
            .encode_into(&mut payload)
            .expect("encode wrong payload");
        let frame = SwarmFrame::new(
            COORDINATOR,
            SENSOR,
            214,
            SESSION_GENERATION,
            0,
            LABEL_REMOTE_SAMPLE_REQ,
            1,
            0,
            &payload[..payload_len],
            SECURE,
        )
        .expect("build correct-label wrong-payload frame");
        medium.send(frame).expect("queue wrong-payload frame");
        assert!(
            matches!(
                (sensor.recv::<RemoteSampleReqMsg>()).await,
                Err(RecvError::Codec(_))
            ),
            "a matching label hint must not authorize an incompatible payload"
        );

        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, SENSOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator rendezvous for wrong-label case");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, SENSOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register sensor rendezvous for wrong-label case");

        let (coordinator_program, sensor_program) = project_sample_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(215), &coordinator_program, NoBinding)
            .expect("attach coordinator for wrong-label case");
        let mut sensor = cluster1
            .enter(rv1, SessionId::new(215), &sensor_program, NoBinding)
            .expect("attach sensor for wrong-label case");

        assert!(
            coordinator.flow::<RemoteActuateReqMsg>().is_err(),
            "coordinator cannot choose an actuator label while the choreography expects a sensor sample request"
        );
        assert!(
            sensor.flow::<RemoteSampleRetMsg>().is_err(),
            "sensor cannot send a sample before receiving the sample request"
        );
    });
}

#[test]
fn datagram_fd_protocol_is_bounded_and_wired_through_hibana_messages() {
    hibana_pico::port::exec::run_current_task(async {
        let mut fds: NetworkObjectTable<2> = NetworkObjectTable::new();
        let fd = fds
            .apply_cap_grant_datagram(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                NodeId::new(4),
                0,
                13,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated datagram fd");
        let recv_only = fds
            .apply_cap_grant_datagram(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                NodeId::new(5),
                0,
                14,
                NetworkRights::Receive,
            )
            .expect("install authenticated recv-only fd");
        assert_eq!(
            fds.resolve(
                recv_only.fd(),
                recv_only.generation(),
                NetworkRights::Send,
                SESSION_GENERATION
            ),
            Err(NetworkError::PermissionDenied)
        );
        assert_eq!(
            fds.resolve(
                fd.fd(),
                fd.generation().wrapping_add(1),
                NetworkRights::Send,
                SESSION_GENERATION
            ),
            Err(NetworkError::BadGeneration)
        );
        assert_eq!(
            DatagramSend::new(
                fd.fd(),
                fd.generation(),
                fd.route(),
                0x5150_4c44,
                &[0u8; NET_DATAGRAM_PAYLOAD_CAPACITY + 1],
            ),
            Err(NetworkError::PayloadTooLarge)
        );

        let backend = HostQueueBackend::new();
        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register app rendezvous");
        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register network rendezvous");
        let (app_program, network_program) = project_datagram_roles();
        let mut app = cluster0
            .enter(rv0, SessionId::new(202), &app_program, NoBinding)
            .expect("attach app");
        let mut network = cluster1
            .enter(rv1, SessionId::new(202), &network_program, NoBinding)
            .expect("attach network");

        let outbound = DatagramSend::new(
            fd.fd(),
            fd.generation(),
            fd.route(),
            fds.allocate_operation_id(),
            b"hello",
        )
        .expect("datagram send");
        let resolved = fds
            .resolve(
                outbound.fd(),
                outbound.generation(),
                NetworkRights::Send,
                SESSION_GENERATION,
            )
            .expect("resolve send fd");
        assert_eq!(resolved.protocol(), NetworkRoleProtocol::Datagram);
        (app.flow::<DatagramSendMsg>()
            .expect("app flow<datagram send>")
            .send(&outbound))
        .await
        .expect("send datagram");
        assert_eq!(
            (network.recv::<DatagramSendMsg>())
                .await
                .expect("network recv datagram"),
            outbound
        );
        let ack = DatagramAck::new(fd.fd(), fd.generation(), outbound.operation_id(), true);
        (network
            .flow::<DatagramAckMsg>()
            .expect("network flow<ack>")
            .send(&ack))
        .await
        .expect("send datagram ack");
        assert_eq!(
            (app.recv::<DatagramAckMsg>())
                .await
                .expect("app recv datagram ack"),
            ack
        );

        let recv_req = DatagramRecv::new(fd.fd(), fd.generation(), 8);
        (app.flow::<DatagramRecvMsg>()
            .expect("app flow<datagram recv>")
            .send(&recv_req))
        .await
        .expect("send datagram recv request");
        assert_eq!(
            (network.recv::<DatagramRecvMsg>())
                .await
                .expect("network recv request"),
            recv_req
        );
        let recv_ret = DatagramRecvRet::new(fd.fd(), fd.generation(), b"pong").expect("recv ret");
        (network
            .flow::<DatagramRecvRetMsg>()
            .expect("network flow<recv ret>")
            .send(&recv_ret))
        .await
        .expect("send datagram recv ret");
        assert_eq!(
            (app.recv::<DatagramRecvRetMsg>())
                .await
                .expect("app recv datagram"),
            recv_ret
        );
    });
}

#[test]
fn wasi_fd_selects_network_datagram_route_without_p2_or_bridge() {
    hibana_pico::port::exec::run_current_task(async {
        let mut fds: NetworkObjectTable<4> = NetworkObjectTable::new();
        let datagram_fd = fds
            .apply_cap_grant_datagram_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                22,
                LABEL_NET_DATAGRAM_SEND,
                4,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated datagram fd");
        assert_eq!(datagram_fd.policy_slot(), 4);
        let recv_only_fd = fds
            .apply_cap_grant_datagram(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                22,
                LABEL_NET_DATAGRAM_RECV,
                NetworkRights::Receive,
            )
            .expect("install authenticated recv-only datagram fd");
        let stream_fd = fds
            .apply_cap_grant_stream(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                23,
                LABEL_NET_STREAM_WRITE,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated stream fd");
        let mut policy_slots: PolicySlotTable<4> = PolicySlotTable::new();
        policy_slots
            .allow(datagram_fd.policy_slot())
            .expect("allow datagram policy slot");
        policy_slots
            .allow(stream_fd.policy_slot())
            .expect("allow stream policy slot");
        assert_eq!(
            fds.route_fd_write(
                recv_only_fd.fd(),
                recv_only_fd.generation(),
                SESSION_GENERATION
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::PermissionDenied)
        );
        assert_eq!(
            fds.route_fd_read(stream_fd.fd(), stream_fd.generation(), SESSION_GENERATION),
            NetworkObjectReadRoute::Stream(stream_fd)
        );
        assert_eq!(
            fds.route_fd_write(stream_fd.fd(), stream_fd.generation(), SESSION_GENERATION),
            NetworkObjectWriteRoute::Stream(stream_fd)
        );
        assert_eq!(
            fds.route_fd_write_authorized(
                datagram_fd.fd(),
                datagram_fd.generation(),
                datagram_fd.route_key(),
                &policy_slots,
            ),
            NetworkObjectWriteRoute::Datagram(datagram_fd)
        );
        policy_slots
            .deny(datagram_fd.policy_slot())
            .expect("deny datagram policy slot");
        assert_eq!(
            fds.route_fd_write_authorized(
                datagram_fd.fd(),
                datagram_fd.generation(),
                datagram_fd.route_key(),
                &policy_slots,
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::PolicyDenied)
        );
        policy_slots
            .allow(datagram_fd.policy_slot())
            .expect("re-allow datagram policy slot");
        assert_eq!(
            fds.route_fd_write_routed(
                datagram_fd.fd(),
                datagram_fd.generation(),
                NetworkRoute::new(
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    SESSION_GENERATION.wrapping_add(1),
                )
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadSessionGeneration)
        );
        assert_eq!(
            fds.route_fd_write_routed(
                datagram_fd.fd(),
                datagram_fd.generation(),
                NetworkRoute::new(SENSOR, 23, LABEL_NET_DATAGRAM_SEND, SESSION_GENERATION)
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadRoute)
        );
        assert_eq!(
            fds.route_fd_write_routed(
                datagram_fd.fd(),
                datagram_fd.generation(),
                NetworkRoute::with_policy(
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    SESSION_GENERATION,
                    datagram_fd.policy_slot().wrapping_add(1),
                )
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadRoute)
        );

        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, GATEWAY, SENSOR, ACTUATOR];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register engine rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register kernel rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register network rendezvous");

        let (engine_program, kernel_program, network_program) =
            project_network_object_write_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(220), &engine_program, NoBinding)
            .expect("attach engine network object_write projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(220), &kernel_program, NoBinding)
            .expect("attach kernel network object_write projection");
        let mut network = cluster2
            .enter(rv2, SessionId::new(220), &network_program, NoBinding)
            .expect("attach network object_write projection");

        let write =
            FdWrite::new_with_lease(datagram_fd.fd(), 8, b"udp").expect("network object_write");
        let write_request = EngineReq::FdWrite(write);
        (engine
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine flow<network object_write>")
            .send(&write_request))
        .await
        .expect("engine sends network object_write to kernel");
        let received_write = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("kernel receives network object_write");
        assert_eq!(received_write, write_request);
        let EngineReq::FdWrite(received_write) = received_write else {
            panic!("expected fd_write request");
        };
        let resolved_write = match fds.route_fd_write_authorized(
            received_write.fd(),
            datagram_fd.generation(),
            datagram_fd.route_key(),
            &policy_slots,
        ) {
            NetworkObjectWriteRoute::Datagram(fd) => fd,
            NetworkObjectWriteRoute::Stream(_) => {
                panic!("datagram fd_write must not select stream route")
            }
            NetworkObjectWriteRoute::Rejected(error) => {
                panic!("datagram fd_write should select network route: {error:?}")
            }
        };
        assert_eq!(resolved_write.target_node(), SENSOR);
        assert_eq!(resolved_write.lane(), 22);
        assert_eq!(resolved_write.protocol(), NetworkRoleProtocol::Datagram);

        (kernel
            .flow::<NetworkDatagramSendRouteControl>()
            .expect("kernel flow<network datagram send route>")
            .send(()))
        .await
        .expect("kernel selects datagram send route");
        let datagram_send = DatagramSend::new(
            resolved_write.fd(),
            resolved_write.generation(),
            resolved_write.route(),
            fds.allocate_operation_id(),
            received_write.as_bytes(),
        )
        .expect("datagram send request");
        (kernel
            .flow::<DatagramSendMsg>()
            .expect("kernel flow<datagram send>")
            .send(&datagram_send))
        .await
        .expect("kernel sends datagram request");
        let network_branch = (network.offer()).await.expect("network offers send route");
        assert_eq!(
            (network_branch.decode::<DatagramSendMsg>())
                .await
                .expect("network decodes datagram send"),
            datagram_send
        );
        let ack = DatagramAck::new(
            resolved_write.fd(),
            resolved_write.generation(),
            datagram_send.operation_id(),
            true,
        );
        (network
            .flow::<DatagramAckMsg>()
            .expect("network flow<datagram ack>")
            .send(&ack))
        .await
        .expect("network sends datagram ack");
        assert_eq!(
            (kernel.recv::<DatagramAckMsg>())
                .await
                .expect("kernel receives datagram ack"),
            ack
        );
        let write_done = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("kernel flow<network object_write ret>")
            .send(&write_done))
        .await
        .expect("kernel returns network object_write");
        let engine_branch = (engine.offer()).await.expect("engine offers send route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("engine decodes network object_write ret"),
            write_done
        );
        assert!(
            kernel.flow::<NetworkRejectRouteControl>().is_err(),
            "unselected network reject arm must not be reachable after send route"
        );
        drop(engine);
        drop(kernel);
        drop(network);

        let (engine_program, kernel_program, network_program) =
            project_network_object_read_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(221), &engine_program, NoBinding)
            .expect("attach engine network object_read projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(221), &kernel_program, NoBinding)
            .expect("attach kernel network object_read projection");
        let mut network = cluster2
            .enter(rv2, SessionId::new(221), &network_program, NoBinding)
            .expect("attach network object_read projection");

        let read = FdRead::new_with_lease(datagram_fd.fd(), 9, 8).expect("network object_read");
        let read_request = EngineReq::FdRead(read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<network object_read>")
            .send(&read_request))
        .await
        .expect("engine sends network object_read to kernel");
        let received_read = (kernel.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("kernel receives network object_read");
        assert_eq!(received_read, read_request);
        let EngineReq::FdRead(received_read) = received_read else {
            panic!("expected fd_read request");
        };
        let resolved_read = match fds.route_fd_read_authorized(
            received_read.fd(),
            datagram_fd.generation(),
            datagram_fd.route_key(),
            &policy_slots,
        ) {
            NetworkObjectReadRoute::Datagram(fd) => fd,
            NetworkObjectReadRoute::Stream(_) => {
                panic!("datagram fd_read must not select stream route")
            }
            NetworkObjectReadRoute::Rejected(error) => {
                panic!("datagram fd_read should select network route: {error:?}")
            }
        };
        assert_eq!(resolved_read.target_node(), SENSOR);
        assert_eq!(resolved_read.lane(), 22);

        (kernel
            .flow::<NetworkDatagramRecvRouteControl>()
            .expect("kernel flow<network datagram recv route>")
            .send(()))
        .await
        .expect("kernel selects datagram recv route");
        let datagram_recv = DatagramRecv::new(
            resolved_read.fd(),
            resolved_read.generation(),
            received_read.max_len(),
        );
        (kernel
            .flow::<DatagramRecvMsg>()
            .expect("kernel flow<datagram recv>")
            .send(&datagram_recv))
        .await
        .expect("kernel sends datagram recv request");
        let network_branch = (network.offer()).await.expect("network offers recv route");
        assert_eq!(
            (network_branch.decode::<DatagramRecvMsg>())
                .await
                .expect("network decodes datagram recv"),
            datagram_recv
        );
        let recv_ret =
            DatagramRecvRet::new(resolved_read.fd(), resolved_read.generation(), b"pong")
                .expect("datagram recv ret");
        (network
            .flow::<DatagramRecvRetMsg>()
            .expect("network flow<datagram recv ret>")
            .send(&recv_ret))
        .await
        .expect("network sends datagram recv ret");
        assert_eq!(
            (kernel.recv::<DatagramRecvRetMsg>())
                .await
                .expect("kernel receives datagram recv ret"),
            recv_ret
        );
        let read_done = EngineRet::FdReadDone(
            FdReadDone::new_with_lease(
                received_read.fd(),
                received_read.lease_id(),
                recv_ret.payload(),
            )
            .expect("network object_read done"),
        );
        (kernel
            .flow::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>()
            .expect("kernel flow<network object_read ret>")
            .send(&read_done))
        .await
        .expect("kernel returns network object_read");
        let engine_branch = (engine.offer()).await.expect("engine offers recv route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>())
                .await
                .expect("engine decodes network object_read ret"),
            read_done
        );
        assert!(
            kernel.flow::<NetworkRejectRouteControl>().is_err(),
            "unselected network reject arm must not be reachable after recv route"
        );
        drop(engine);
        drop(kernel);
        drop(network);

        let (engine_program, kernel_program) = project_network_object_reject_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(222), &engine_program, NoBinding)
            .expect("attach engine network reject projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(222), &kernel_program, NoBinding)
            .expect("attach kernel network reject projection");

        let stale_read = FdRead::new_with_lease(datagram_fd.fd(), 10, 4).expect("stale fd_read");
        let stale_request = EngineReq::FdRead(stale_read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<stale network object_read>")
            .send(&stale_request))
        .await
        .expect("engine sends stale network object_read");
        let received_stale = (kernel.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("kernel receives stale network object_read");
        assert_eq!(received_stale, stale_request);
        let EngineReq::FdRead(received_stale) = received_stale else {
            panic!("expected stale fd_read request");
        };
        let rejected = match fds.route_fd_read(
            received_stale.fd(),
            datagram_fd.generation().wrapping_add(1),
            SESSION_GENERATION,
        ) {
            NetworkObjectReadRoute::Rejected(error) => error,
            NetworkObjectReadRoute::Datagram(_) => panic!("stale generation must reject"),
            NetworkObjectReadRoute::Stream(_) => panic!("stale generation must reject"),
        };
        assert_eq!(rejected, NetworkError::BadGeneration);

        (kernel
            .flow::<NetworkRejectRouteControl>()
            .expect("kernel flow<network reject route>")
            .send(()))
        .await
        .expect("kernel selects network reject route");
        let fd_error = FdError::new(received_stale.fd(), 70);
        (kernel
            .flow::<FdErrorMsg>()
            .expect("kernel flow<network object error>")
            .send(&fd_error))
        .await
        .expect("kernel sends network object error");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers network reject");
        assert_eq!(
            (engine_branch.decode::<FdErrorMsg>())
                .await
                .expect("engine decodes network object error"),
            fd_error
        );
        assert!(
            kernel.flow::<NetworkDatagramRecvRouteControl>().is_err(),
            "accepted network route must not be reachable after reject"
        );
    });
}

#[test]
fn wasi_fd_selects_network_stream_route_without_p2_or_bridge() {
    hibana_pico::port::exec::run_current_task(async {
        let mut fds: NetworkObjectTable<2> = NetworkObjectTable::new();
        let stream_fd = fds
            .apply_cap_grant_stream_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                23,
                LABEL_NET_STREAM_WRITE,
                5,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated stream fd");
        assert_eq!(stream_fd.policy_slot(), 5);
        let mut policy_slots: PolicySlotTable<2> = PolicySlotTable::new();
        policy_slots
            .allow(stream_fd.policy_slot())
            .expect("allow stream policy slot");
        assert_eq!(
            fds.route_fd_write_authorized(
                stream_fd.fd(),
                stream_fd.generation(),
                stream_fd.route_key(),
                &policy_slots,
            ),
            NetworkObjectWriteRoute::Stream(stream_fd)
        );
        assert_eq!(
            fds.route_fd_read_authorized(
                stream_fd.fd(),
                stream_fd.generation(),
                stream_fd.route_key(),
                &policy_slots,
            ),
            NetworkObjectReadRoute::Stream(stream_fd)
        );

        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let role_nodes = [COORDINATOR, GATEWAY, SENSOR, ACTUATOR];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register stream engine rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    GATEWAY,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register stream kernel rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..24)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    SENSOR,
                    role_nodes,
                    3,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register stream network rendezvous");

        let (engine_program, kernel_program, network_program) =
            project_network_stream_fd_write_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(223), &engine_program, NoBinding)
            .expect("attach stream write engine projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(223), &kernel_program, NoBinding)
            .expect("attach stream write kernel projection");
        let mut network = cluster2
            .enter(rv2, SessionId::new(223), &network_program, NoBinding)
            .expect("attach stream write network projection");

        let write = FdWrite::new_with_lease(stream_fd.fd(), 8, b"stream").expect("stream fd_write");
        let write_request = EngineReq::FdWrite(write);
        (engine
            .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
            .expect("engine flow<stream fd_write>")
            .send(&write_request))
        .await
        .expect("engine sends stream fd_write");
        let received_write = (kernel.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>())
            .await
            .expect("kernel receives stream fd_write");
        let EngineReq::FdWrite(received_write) = received_write else {
            panic!("expected stream fd_write request");
        };
        let resolved_write = match fds.route_fd_write_authorized(
            received_write.fd(),
            stream_fd.generation(),
            stream_fd.route_key(),
            &policy_slots,
        ) {
            NetworkObjectWriteRoute::Stream(fd) => fd,
            other => panic!("stream fd_write should select stream route: {other:?}"),
        };
        assert_eq!(resolved_write.protocol(), NetworkRoleProtocol::Stream);
        assert_eq!(resolved_write.lane(), 23);

        (kernel
            .flow::<NetworkStreamWriteRouteControl>()
            .expect("kernel flow<network stream write route>")
            .send(()))
        .await
        .expect("kernel selects stream write route");
        let stream_write = StreamWrite::new(
            resolved_write.fd(),
            resolved_write.generation(),
            resolved_write.route(),
            fds.allocate_operation_id(),
            0,
            NET_STREAM_FLAG_FIN,
            received_write.as_bytes(),
        )
        .expect("stream write request");
        assert_eq!(stream_write.sequence(), 0);
        assert!(stream_write.is_fin());
        (kernel
            .flow::<StreamWriteMsg>()
            .expect("kernel flow<stream write>")
            .send(&stream_write))
        .await
        .expect("kernel sends stream write");
        let network_branch = (network.offer())
            .await
            .expect("network offers stream write route");
        assert_eq!(
            (network_branch.decode::<StreamWriteMsg>())
                .await
                .expect("network decodes stream write"),
            stream_write
        );
        let ack = StreamAck::new(
            resolved_write.fd(),
            resolved_write.generation(),
            stream_write.operation_id(),
            0,
            true,
        );
        (network
            .flow::<StreamAckMsg>()
            .expect("network flow<stream ack>")
            .send(&ack))
        .await
        .expect("network sends stream ack");
        assert_eq!(
            (kernel.recv::<StreamAckMsg>())
                .await
                .expect("kernel receives stream ack"),
            ack
        );
        let write_done = EngineRet::FdWriteDone(FdWriteDone::new(
            received_write.fd(),
            received_write.len() as u8,
        ));
        (kernel
            .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
            .expect("kernel flow<stream fd_write ret>")
            .send(&write_done))
        .await
        .expect("kernel returns stream fd_write");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers stream write route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>())
                .await
                .expect("engine decodes stream fd_write ret"),
            write_done
        );
        drop(engine);
        drop(kernel);
        drop(network);

        let (engine_program, kernel_program, network_program) =
            project_network_stream_fd_read_route_roles();
        let mut engine = cluster0
            .enter(rv0, SessionId::new(224), &engine_program, NoBinding)
            .expect("attach stream read engine projection");
        let mut kernel = cluster1
            .enter(rv1, SessionId::new(224), &kernel_program, NoBinding)
            .expect("attach stream read kernel projection");
        let mut network = cluster2
            .enter(rv2, SessionId::new(224), &network_program, NoBinding)
            .expect("attach stream read network projection");

        let read = FdRead::new_with_lease(stream_fd.fd(), 9, 8).expect("stream fd_read");
        let read_request = EngineReq::FdRead(read);
        (engine
            .flow::<Msg<LABEL_WASI_FD_READ, EngineReq>>()
            .expect("engine flow<stream fd_read>")
            .send(&read_request))
        .await
        .expect("engine sends stream fd_read");
        let received_read = (kernel.recv::<Msg<LABEL_WASI_FD_READ, EngineReq>>())
            .await
            .expect("kernel receives stream fd_read");
        let EngineReq::FdRead(received_read) = received_read else {
            panic!("expected stream fd_read request");
        };
        let resolved_read = match fds.route_fd_read_authorized(
            received_read.fd(),
            stream_fd.generation(),
            stream_fd.route_key(),
            &policy_slots,
        ) {
            NetworkObjectReadRoute::Stream(fd) => fd,
            other => panic!("stream fd_read should select stream route: {other:?}"),
        };

        (kernel
            .flow::<NetworkStreamReadRouteControl>()
            .expect("kernel flow<network stream read route>")
            .send(()))
        .await
        .expect("kernel selects stream read route");
        let stream_read = StreamRead::new(
            resolved_read.fd(),
            resolved_read.generation(),
            0,
            received_read.max_len(),
        );
        (kernel
            .flow::<StreamReadMsg>()
            .expect("kernel flow<stream read>")
            .send(&stream_read))
        .await
        .expect("kernel sends stream read");
        let network_branch = (network.offer())
            .await
            .expect("network offers stream read route");
        assert_eq!(
            (network_branch.decode::<StreamReadMsg>())
                .await
                .expect("network decodes stream read"),
            stream_read
        );
        let stream_ret = StreamReadRet::new(
            resolved_read.fd(),
            resolved_read.generation(),
            0,
            NET_STREAM_FLAG_FIN,
            b"pipe",
        )
        .expect("stream read ret");
        assert_eq!(stream_ret.sequence(), 0);
        assert!(stream_ret.is_fin());
        (network
            .flow::<StreamReadRetMsg>()
            .expect("network flow<stream read ret>")
            .send(&stream_ret))
        .await
        .expect("network sends stream read ret");
        assert_eq!(
            (kernel.recv::<StreamReadRetMsg>())
                .await
                .expect("kernel receives stream read ret"),
            stream_ret
        );
        let read_done = EngineRet::FdReadDone(
            FdReadDone::new_with_lease(
                received_read.fd(),
                received_read.lease_id(),
                stream_ret.payload(),
            )
            .expect("stream fd_read done"),
        );
        (kernel
            .flow::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>()
            .expect("kernel flow<stream fd_read ret>")
            .send(&read_done))
        .await
        .expect("kernel returns stream fd_read");
        let engine_branch = (engine.offer())
            .await
            .expect("engine offers stream read route");
        assert_eq!(
            (engine_branch.decode::<Msg<LABEL_WASI_FD_READ_RET, EngineRet>>())
                .await
                .expect("engine decodes stream fd_read ret"),
            read_done
        );
    });
}

#[test]
fn three_node_policy_and_remote_management_are_wired_and_fenced() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();
        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register coordinator rendezvous");
        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register gateway rendezvous");
        let (coordinator_program, gateway_program) = project_policy_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(203), &coordinator_program, NoBinding)
            .expect("attach coordinator");
        let mut gateway = cluster1
            .enter(rv1, SessionId::new(203), &gateway_program, NoBinding)
            .expect("attach gateway");

        let telemetry = SwarmTelemetry::new(
            ACTUATOR,
            RoleMask::single(NodeRole::Gateway).with(NodeRole::Actuator),
            2,
            0,
            128,
            36_50,
            SESSION_GENERATION,
        );
        (gateway
            .flow::<SwarmTelemetryMsg>()
            .expect("gateway flow<telemetry>")
            .send(&telemetry))
        .await
        .expect("send telemetry");
        assert_eq!(
            (coordinator.recv::<SwarmTelemetryMsg>())
                .await
                .expect("coordinator recv telemetry"),
            telemetry
        );

        let policy = MultiAppPolicyState::new(
            AppInstance::new(AppChoice::App0, 10, 20, 30),
            AppInstance::new(AppChoice::App1, 11, 21, 31),
        );
        let signal = policy
            .choose_explicit(
                COORDINATOR,
                AppChoice::App1,
                AppChoice::App1.label(),
                telemetry,
            )
            .expect("explicit app1 policy");
        assert_eq!(signal.choice(), AppChoice::App1);
        (coordinator
            .flow::<PolicyApp1Msg>()
            .expect("coordinator flow<policy app1>")
            .send(&signal))
        .await
        .expect("send policy signal");
        assert_eq!(
            (gateway.recv::<PolicyApp1Msg>())
                .await
                .expect("gateway recv policy"),
            signal
        );

        let blocked = SwarmTelemetry::new(
            ACTUATOR,
            RoleMask::single(NodeRole::Actuator),
            17,
            0,
            128,
            36_50,
            SESSION_GENERATION,
        );
        assert_eq!(
            policy.choose_explicit(
                COORDINATOR,
                AppChoice::App1,
                AppChoice::App1.label(),
                blocked
            ),
            Err(PolicyError::TelemetryBlocked)
        );

        let mut remote_objects: RemoteObjectTable<2> = RemoteObjectTable::new();
        let _mgmt_cap = remote_objects
            .apply_cap_grant_management(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                ACTUATOR,
                NodeRole::Actuator.bit() as u8,
                0,
                AppChoice::App1.label(),
                RemoteRights::Write,
            )
            .expect("install authenticated management cap");
        assert!(remote_objects.has_active());
        let mut network_objects: NetworkObjectTable<2> = NetworkObjectTable::new();
        let _datagram_fd = network_objects
            .apply_cap_grant_datagram(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                GATEWAY,
                22,
                LABEL_NET_DATAGRAM_SEND,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated active datagram fd");
        assert!(network_objects.has_active());

        static IMAGE: &[u8] =
            b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write hibana wasip1 stdout\n";
        let mut images: ImageSlotTable<2, 128> = ImageSlotTable::new();
        images
            .begin(MgmtImageBegin::new(0, IMAGE.len() as u32, 55))
            .expect("begin image");
        let mut offset = 0usize;
        while offset < IMAGE.len() {
            let end = core::cmp::min(offset + MGMT_IMAGE_CHUNK_CAPACITY, IMAGE.len());
            images
                .chunk(
                    MgmtImageChunk::new(0, offset as u32, &IMAGE[offset..end])
                        .expect("image chunk"),
                )
                .expect("push image chunk");
            offset = end;
        }
        images
            .end(MgmtImageEnd::new(0, IMAGE.len() as u32))
            .expect("finish image");

        assert_eq!(
            images.activate_at_boundary(
                MgmtImageActivate::new(0, 1),
                ActivationBoundary::new(
                    true,
                    true,
                    !remote_objects.has_active() && !network_objects.has_active(),
                    1,
                ),
            ),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(remote_objects.quiesce_all(), 1);
        assert_eq!(
            images.activate_at_boundary(
                MgmtImageActivate::new(0, 1),
                ActivationBoundary::new(
                    true,
                    true,
                    !remote_objects.has_active() && !network_objects.has_active(),
                    1,
                ),
            ),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(network_objects.quiesce_all(), 1);
        images
            .activate_at_boundary(
                MgmtImageActivate::new(0, 1),
                ActivationBoundary::new(
                    true,
                    true,
                    !remote_objects.has_active() && !network_objects.has_active(),
                    1,
                ),
            )
            .expect("activate after remote and network objects are quiesced");
    });
}

#[test]
fn swarm_policy_route_selects_app_scope_from_budget_telemetry() {
    hibana_pico::port::exec::run_current_task(async {
        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, ACTUATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register coordinator policy rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, ACTUATOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register gateway policy rendezvous");

        let (coordinator_program, gateway_program) = project_swarm_policy_route_roles();
        let mut coordinator = cluster0
            .enter(rv0, SessionId::new(210), &coordinator_program, NoBinding)
            .expect("attach coordinator policy route");
        let mut gateway = cluster1
            .enter(rv1, SessionId::new(210), &gateway_program, NoBinding)
            .expect("attach gateway policy route");

        let telemetry = SwarmTelemetry::new(
            ACTUATOR,
            RoleMask::single(NodeRole::Gateway).with(NodeRole::Actuator),
            1,
            0,
            64,
            35_00,
            SESSION_GENERATION,
        );
        (gateway
            .flow::<SwarmTelemetryMsg>()
            .expect("gateway flow<swarm telemetry>")
            .send(&telemetry))
        .await
        .expect("gateway sends telemetry over swarm");
        let _telemetry_frame_label = medium
            .peek_label(COORDINATOR)
            .expect("swarm transport exposes telemetry frame label hint");
        assert_eq!(
            (coordinator.recv::<SwarmTelemetryMsg>())
                .await
                .expect("coordinator receives policy telemetry"),
            telemetry
        );

        let policy = MultiAppPolicyState::new(
            AppInstance::new(AppChoice::App0, 100, 200, 300),
            AppInstance::new(AppChoice::App1, 101, 201, 301),
        );
        let exhausted = SwarmTelemetry::new(
            ACTUATOR,
            RoleMask::single(NodeRole::Gateway),
            1,
            0,
            0,
            35_00,
            SESSION_GENERATION,
        );
        assert_eq!(
            policy.choose_explicit(
                COORDINATOR,
                AppChoice::App1,
                AppChoice::App1.label(),
                exhausted
            ),
            Err(PolicyError::TelemetryBlocked)
        );

        let app0 = AppId::new(AppChoice::App0.index() as u8);
        let app1 = AppId::new(AppChoice::App1.index() as u8);
        let mut streams: AppStreamTable<2> = AppStreamTable::new();
        let app0_stream = streams
            .open(app0, MemRights::Read)
            .expect("open app0 stream");
        let app1_stream = streams
            .open(app1, MemRights::Read)
            .expect("open app1 stream");
        let mut leases: AppLeaseTable<4> = AppLeaseTable::new(TEST_MEMORY_LEN);
        let app0_lease = leases
            .grant_read(app0, MemBorrow::new(128, 8, TEST_MEMORY_EPOCH))
            .expect("grant app0 lease");
        let app1_lease = leases
            .grant_read(app1, MemBorrow::new(256, 8, TEST_MEMORY_EPOCH))
            .expect("grant app1 lease");
        assert_eq!(app0_lease.lease_id(), app1_lease.lease_id());
        leases
            .release(app0, MemRelease::new(app0_lease.lease_id()))
            .expect("release app0 lease before selected app1 check");

        let signal = policy
            .choose_explicit(
                COORDINATOR,
                AppChoice::App1,
                AppChoice::App1.label(),
                telemetry,
            )
            .expect("choose app1 from budget telemetry");
        assert_eq!(signal.node_id(), COORDINATOR);
        assert_eq!(signal.choice(), AppChoice::App1);
        assert_eq!(signal.route_label(), AppChoice::App1.label());
        assert_eq!(signal.memory_generation(), 101);
        assert_eq!(signal.fd_generation(), 201);
        assert_eq!(signal.lease_generation(), 301);

        (coordinator
            .flow::<PublishAlertControl>()
            .expect("coordinator flow<policy app1 route control>")
            .send(()))
        .await
        .expect("coordinator selects app1 route");
        (coordinator
            .flow::<PolicyApp1Msg>()
            .expect("coordinator flow<policy app1 signal>")
            .send(&signal))
        .await
        .expect("coordinator sends selected app1 policy over swarm");

        let branch = (gateway.offer())
            .await
            .expect("gateway offers selected policy route");
        assert_eq!(branch.label(), AppChoice::App1.label());
        let selected = (branch.decode::<PolicyApp1Msg>())
            .await
            .expect("gateway decodes app1 policy signal");
        assert_eq!(selected, signal);

        let selected_app = AppId::new(selected.choice().index() as u8);
        streams
            .validate(selected_app, app1_stream, MemRights::Read)
            .expect("selected app1 stream remains valid");
        assert_eq!(
            streams.validate(selected_app, app0_stream, MemRights::Read),
            Err(AppScopeError::BadApp)
        );
        leases
            .validate_read(selected_app, app1_lease.lease_id(), 8)
            .expect("selected app1 lease remains valid");
        assert_eq!(
            leases.validate_read(app0, app1_lease.lease_id(), 8),
            Err(AppScopeError::UnknownHandle)
        );
        assert!(
            coordinator.flow::<PolicyApp0Msg>().is_err(),
            "unselected app0 policy branch must not be reachable after app1 route"
        );
    });
}

#[test]
fn topology_tx_and_state_controls_project_as_hibana_control_messages() {
    let program = seq_chain!(
        send::<Role<0>, Role<1>, TopologyBeginControl, 19>(),
        send::<Role<1>, Role<0>, TopologyAckControl, 19>(),
        send::<Role<0>, Role<1>, TopologyCommitControl, 19>(),
        send::<Role<0>, Role<1>, TxCommitControl, 19>(),
        send::<Role<1>, Role<0>, TxAbortControl, 19>(),
        send::<Role<1>, Role<0>, StateSnapshotControl, 19>(),
        send::<Role<0>, Role<1>, StateRestoreControl, 19>(),
    );
    let _coordinator_program: RoleProgram<0> = project(&program);
    let _node_program: RoleProgram<1> = project(&program);

    assert!(EngineLabelUniverse::MAX_LABEL >= 144);
}

#[test]
fn abort_normal_route_contains_inner_continue_break_loop_projection() {
    let abort = send::<Role<1>, Role<1>, EngineAbortRouteControl, 1>()
        .policy::<POLICY_BAKER_ENGINE_ABORT_ROUTE>();
    let normal = g::seq(
        send::<Role<1>, Role<1>, EngineNormalRouteControl, 1>()
            .policy::<POLICY_BAKER_ENGINE_ABORT_ROUTE>(),
        g::route(
            g::seq(
                send::<Role<1>, Role<1>, BakerTrafficLoopContinueControl, 1>()
                    .policy::<POLICY_BAKER_TRAFFIC_LOOP>(),
                send::<Role<1>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
            ),
            g::seq(
                send::<Role<1>, Role<1>, BakerTrafficLoopBreakControl, 1>()
                    .policy::<POLICY_BAKER_TRAFFIC_LOOP>(),
                send::<Role<1>, Role<0>, Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
            ),
        ),
    );
    let program = g::route(abort, normal);

    let _kernel_program: RoleProgram<0> = project(&program);
    let _engine_program: RoleProgram<1> = project(&program);
}

#[test]
fn remote_management_image_install_is_wired_through_hibana_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        static IMAGE: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1 fd_write stdout\n";
        let plan = ImageTransferPlan::new(IMAGE.len()).expect("plan remote image transfer");
        assert_eq!(plan.chunk_count(), 2);

        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, ACTUATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, ACTUATOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register management rendezvous");

        let (supervisor_program, management_program) = project_remote_management_roles();
        let mut supervisor = cluster0
            .enter(rv0, SessionId::new(206), &supervisor_program, NoBinding)
            .expect("attach supervisor");
        let mut management = cluster1
            .enter(rv1, SessionId::new(206), &management_program, NoBinding)
            .expect("attach management");

        let mut images: ImageSlotTable<2, 128> = ImageSlotTable::new();
        let mgmt_grant =
            MgmtControl::install_grant(COORDINATOR, SWARM_CREDENTIAL, SESSION_GENERATION, 0, 77);
        let mut leases: MemoryLeaseTable<2> =
            MemoryLeaseTable::new(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH);
        leases
            .grant_read(MemBorrow::new(TEST_STDOUT_PTR, 8, TEST_MEMORY_EPOCH))
            .expect("seed outstanding local memory lease");
        let mut resolver: PicoInterruptResolver<1, 2, 2> = PicoInterruptResolver::new();
        resolver
            .request_gpio_wait(GpioWait::new(60, 9, 4, SESSION_GENERATION))
            .expect("seed outstanding interrupt subscription");
        let mut remote_objects: RemoteObjectTable<2> = RemoteObjectTable::new();
        let management_cap = remote_objects
            .apply_cap_grant_management(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                ACTUATOR,
                1,
                1,
                LABEL_MGMT_IMAGE_BEGIN,
                RemoteRights::Write,
            )
            .expect("install authenticated management control");
        assert_eq!(
            remote_objects.resolve(
                management_cap.fd(),
                management_cap.generation(),
                RemoteRights::Write,
                SESSION_GENERATION,
            ),
            Ok(management_cap)
        );

        let begin = MgmtImageBegin::new(0, plan.total_len(), 77);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
            .expect("management flow<image begin>")
            .send(&begin))
        .await
        .expect("send image begin over swarm");
        let received_begin = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>())
            .await
            .expect("supervisor recv image begin over swarm");
        assert_eq!(received_begin, begin);
        let status = images
            .begin_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_begin,
            )
            .expect("begin image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<begin status>")
            .send(&status))
        .await
        .expect("send begin status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv begin status")
                .code(),
            MgmtStatusCode::Ok
        );

        let (first_offset, first_end) = plan.chunk_range(0).expect("first image chunk range");
        let first = MgmtImageChunk::new(0, first_offset as u32, &IMAGE[first_offset..first_end])
            .expect("first image chunk");
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
            .expect("management flow<first image chunk>")
            .send(&first))
        .await
        .expect("send first image chunk");
        let received_first = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>())
            .await
            .expect("supervisor recv first image chunk");
        assert_eq!(received_first, first);
        let status = images
            .chunk_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_first,
            )
            .expect("append first chunk");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<first chunk status>")
            .send(&status))
        .await
        .expect("send first chunk status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv first chunk status")
                .code(),
            MgmtStatusCode::Ok
        );

        let (second_offset, second_end) = plan.chunk_range(1).expect("second image chunk range");
        let second =
            MgmtImageChunk::new(0, second_offset as u32, &IMAGE[second_offset..second_end])
                .expect("second image chunk");
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
            .expect("management flow<second image chunk>")
            .send(&second))
        .await
        .expect("send second image chunk");
        let received_second = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>())
            .await
            .expect("supervisor recv second image chunk");
        assert_eq!(received_second, second);
        let status = images
            .chunk_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_second,
            )
            .expect("append second chunk");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<second chunk status>")
            .send(&status))
        .await
        .expect("send second chunk status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv second chunk status")
                .code(),
            MgmtStatusCode::Ok
        );

        let end = MgmtImageEnd::new(0, plan.total_len());
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
            .expect("management flow<image end>")
            .send(&end))
        .await
        .expect("send image end");
        let received_end = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>())
            .await
            .expect("recv end");
        assert_eq!(received_end, end);
        let status = images
            .end_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_end,
            )
            .expect("finish image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<end status>")
            .send(&status))
        .await
        .expect("send end status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv end status")
                .code(),
            MgmtStatusCode::Ok
        );
        assert_eq!(
            images.slot(0).expect("installed remote image").as_bytes(),
            IMAGE
        );

        let activate = MgmtImageActivate::new(0, 77);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
            .expect("management flow<activate before fence>")
            .send(&activate))
        .await
        .expect("send activate before fence");
        let received_activate = (supervisor
            .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>())
        .await
        .expect("recv activate before fence");
        assert_eq!(received_activate, activate);
        let activation_error = images
            .activate_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_activate,
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            )
            .expect_err("activation before fence must fail");
        assert_eq!(activation_error, ImageSlotError::NeedFence);
        let status = activation_error.status(received_activate.slot());
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<need fence status>")
            .send(&status))
        .await
        .expect("send need fence status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv need fence status")
                .code(),
            MgmtStatusCode::NeedFence
        );

        let fence = MemFence::new(MemFenceReason::HotSwap, 77);
        (management
            .flow::<Msg<LABEL_MEM_FENCE, MemFence>>()
            .expect("management flow<mem fence>")
            .send(&fence))
        .await
        .expect("send mem fence");
        let received_fence = (supervisor.recv::<Msg<LABEL_MEM_FENCE, MemFence>>())
            .await
            .expect("recv mem fence");
        assert_eq!(received_fence, fence);
        leases.fence(received_fence);
        assert_eq!(remote_objects.quiesce_all(), 1);
        assert!(!leases.has_outstanding_leases());
        assert!(!remote_objects.has_active());
        assert_eq!(
            images.activate_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                activate,
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            ),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(resolver.fence_gpio_waits(), 1);
        assert!(!resolver.has_active_gpio_waits());
        assert_eq!(
            images.activate_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                MgmtImageActivate::new(0, 1),
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            ),
            Err(ImageSlotError::BadFenceEpoch)
        );

        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
            .expect("management flow<activate after fence>")
            .send(&activate))
        .await
        .expect("send activate after fence");
        let received_activate = (supervisor
            .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>())
        .await
        .expect("recv activate after fence");
        assert_eq!(received_activate, activate);
        let status = images
            .activate_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_activate,
                ActivationBoundary::new(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    !remote_objects.has_active(),
                    leases.epoch(),
                ),
            )
            .expect("activate after safe boundary");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<activate ok status>")
            .send(&status))
        .await
        .expect("send activate ok status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv activate ok status")
                .code(),
            MgmtStatusCode::Ok
        );
        assert_eq!(images.active_slot(), Some(0));
    });
}

#[test]
fn remote_management_rejects_bad_image_over_swarm_transport() {
    hibana_pico::port::exec::run_current_task(async {
        static BAD_IMAGE: &[u8] = b"not-wasm";
        let plan = ImageTransferPlan::new(BAD_IMAGE.len()).expect("plan invalid image transfer");
        assert_eq!(plan.chunk_count(), 1);

        let medium: HostSwarmMedium<8> = HostSwarmMedium::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, COORDINATOR, ACTUATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register invalid-image supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmTransport::new(&medium, ACTUATOR, COORDINATOR, SESSION_GENERATION, SECURE),
            )
            .expect("register invalid-image management rendezvous");

        let (supervisor_program, management_program) =
            project_remote_management_invalid_image_roles();
        let mut supervisor = cluster0
            .enter(rv0, SessionId::new(225), &supervisor_program, NoBinding)
            .expect("attach invalid-image supervisor");
        let mut management = cluster1
            .enter(rv1, SessionId::new(225), &management_program, NoBinding)
            .expect("attach invalid-image management");

        let mut images: ImageSlotTable<2, 64> = ImageSlotTable::new();
        let mgmt_grant =
            MgmtControl::install_grant(COORDINATOR, SWARM_CREDENTIAL, SESSION_GENERATION, 0, 13);

        let begin = MgmtImageBegin::new(0, plan.total_len(), 13);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
            .expect("management flow<invalid image begin>")
            .send(&begin))
        .await
        .expect("send invalid image begin");
        let received_begin = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>())
            .await
            .expect("supervisor recv invalid image begin");
        assert_eq!(received_begin, begin);
        let status = images
            .begin_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_begin,
            )
            .expect("begin invalid image slot");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<invalid image begin status>")
            .send(&status))
        .await
        .expect("send invalid image begin status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv invalid image begin status")
                .code(),
            MgmtStatusCode::Ok
        );

        let (offset, end) = plan.chunk_range(0).expect("invalid image chunk range");
        let chunk = MgmtImageChunk::new(0, offset as u32, &BAD_IMAGE[offset..end])
            .expect("invalid image chunk");
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
            .expect("management flow<invalid image chunk>")
            .send(&chunk))
        .await
        .expect("send invalid image chunk");
        let received_chunk = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>())
            .await
            .expect("supervisor recv invalid image chunk");
        assert_eq!(received_chunk, chunk);
        let status = images
            .chunk_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_chunk,
            )
            .expect("append invalid image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<invalid image chunk status>")
            .send(&status))
        .await
        .expect("send invalid image chunk status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv invalid image chunk status")
                .code(),
            MgmtStatusCode::Ok
        );

        let image_end = MgmtImageEnd::new(0, plan.total_len());
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
            .expect("management flow<invalid image end>")
            .send(&image_end))
        .await
        .expect("send invalid image end");
        let received_end = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>())
            .await
            .expect("supervisor recv invalid image end");
        assert_eq!(received_end, image_end);
        let image_error = images
            .end_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_end,
            )
            .expect_err("bad image must be rejected");
        assert!(matches!(image_error, ImageSlotError::InvalidImage(_)));
        let status = image_error.status(received_end.slot());
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<invalid image rejected status>")
            .send(&status))
        .await
        .expect("send invalid image rejected status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv invalid image rejected status")
                .code(),
            MgmtStatusCode::InvalidImage
        );

        assert!(!images.slot(0).expect("invalid image slot").is_valid());
        assert_eq!(images.active_slot(), None);
        assert!(
            supervisor
                .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
                .is_err(),
            "bad image rejection must not open an activation phase"
        );
    });
}

#[test]
fn remote_management_activation_emits_node_image_update_to_swarm_observer() {
    hibana_pico::port::exec::run_current_task(async {
        static IMAGE: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1";
        let plan = ImageTransferPlan::new(IMAGE.len()).expect("plan observer image transfer");
        assert_eq!(plan.chunk_count(), 1);

        let medium: HostSwarmMedium<64> = HostSwarmMedium::new();
        let observer_node = SENSOR;
        let role_nodes = [COORDINATOR, ACTUATOR, observer_node, NodeId::new(4)];

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = SwarmRoleTestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    COORDINATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = SwarmRoleTestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    ACTUATOR,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register management rendezvous");

        let clock2 = CounterClock::new();
        let mut tap2 = [TapEvent::zero(); 128];
        let mut slab2 = vec![0u8; 262_144];
        let cluster2 = SwarmRoleTestKit::new(&clock2);
        let rv2 = cluster2
            .add_rendezvous_from_config(
                Config::new(&mut tap2, slab2.as_mut_slice())
                    .with_lane_range(0..21)
                    .with_universe(EngineLabelUniverse),
                HostSwarmRoleTransport::new(
                    &medium,
                    observer_node,
                    role_nodes,
                    4,
                    SESSION_GENERATION,
                    SECURE,
                ),
            )
            .expect("register observer rendezvous");

        let (supervisor_program, management_program, observer_program) =
            project_remote_management_observer_roles();
        let mut supervisor = cluster0
            .enter(rv0, SessionId::new(211), &supervisor_program, NoBinding)
            .expect("attach supervisor");
        let mut management = cluster1
            .enter(rv1, SessionId::new(211), &management_program, NoBinding)
            .expect("attach management");
        let mut observer = cluster2
            .enter(rv2, SessionId::new(211), &observer_program, NoBinding)
            .expect("attach observer");

        let mut images: ImageSlotTable<2, 128> = ImageSlotTable::new();
        let mgmt_grant =
            MgmtControl::install_grant(COORDINATOR, SWARM_CREDENTIAL, SESSION_GENERATION, 0, 88);

        let begin = MgmtImageBegin::new(0, plan.total_len(), 88);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
            .expect("management flow<image begin>")
            .send(&begin))
        .await
        .expect("send image begin");
        let received_begin = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>())
            .await
            .expect("supervisor recv image begin");
        assert_eq!(received_begin, begin);
        let status = images
            .begin_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_begin,
            )
            .expect("begin image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<begin status>")
            .send(&status))
        .await
        .expect("send begin status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv begin status")
                .code(),
            MgmtStatusCode::Ok
        );

        let (chunk_offset, chunk_end) = plan.chunk_range(0).expect("observer image chunk range");
        let chunk = MgmtImageChunk::new(0, chunk_offset as u32, &IMAGE[chunk_offset..chunk_end])
            .expect("image chunk");
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
            .expect("management flow<image chunk>")
            .send(&chunk))
        .await
        .expect("send image chunk");
        let received_chunk = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>())
            .await
            .expect("supervisor recv image chunk");
        assert_eq!(received_chunk, chunk);
        let status = images
            .chunk_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_chunk,
            )
            .expect("append image chunk");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<chunk status>")
            .send(&status))
        .await
        .expect("send chunk status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv chunk status")
                .code(),
            MgmtStatusCode::Ok
        );

        let end = MgmtImageEnd::new(0, plan.total_len());
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
            .expect("management flow<image end>")
            .send(&end))
        .await
        .expect("send image end");
        let received_end = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>())
            .await
            .expect("recv end");
        assert_eq!(received_end, end);
        let status = images
            .end_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_end,
            )
            .expect("finish image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<end status>")
            .send(&status))
        .await
        .expect("send end status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv end status")
                .code(),
            MgmtStatusCode::Ok
        );

        let activate = MgmtImageActivate::new(0, 88);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
            .expect("management flow<activate>")
            .send(&activate))
        .await
        .expect("send activate");
        let received_activate = (supervisor
            .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>())
        .await
        .expect("recv activate");
        assert_eq!(received_activate, activate);
        let status = images
            .activate_with_control(
                mgmt_grant,
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                received_activate,
                ActivationBoundary::single_node(true, true, received_activate.fence_epoch()),
            )
            .expect("activate without outstanding leases");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<activate status>")
            .send(&status))
        .await
        .expect("send activate status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("management recv activate status")
                .code(),
            MgmtStatusCode::Ok
        );
        assert_eq!(images.active_slot(), Some(0));

        let update = NodeImageUpdated::new(ACTUATOR, begin.slot(), begin.generation(), true);
        (supervisor
            .flow::<NodeImageUpdatedMsg>()
            .expect("supervisor flow<node image updated>")
            .send(&update))
        .await
        .expect("send node image update to observer");
        let _node_image_updated_frame_label = medium
            .peek_label(observer_node)
            .expect("swarm transport exposes image update frame label hint");
        let observed = (observer.recv::<NodeImageUpdatedMsg>())
            .await
            .expect("observer recv node image update");
        assert_eq!(observed.node_id(), ACTUATOR);
        assert_eq!(observed.slot(), begin.slot());
        assert_eq!(observed.image_generation(), begin.generation());
        assert!(observed.accepted());
    });
}
