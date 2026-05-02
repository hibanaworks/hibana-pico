use hibana::{
    g,
    g::{Msg, Role},
    substrate::program::{RoleProgram, project},
};

use crate::{
    choreography::protocol::{
        EngineReq, EngineRet, FdErrorMsg, LABEL_MEM_BORROW_READ, LABEL_MEM_RELEASE,
        LABEL_MGMT_IMAGE_ACTIVATE, LABEL_MGMT_IMAGE_BEGIN, LABEL_MGMT_IMAGE_CHUNK,
        LABEL_MGMT_IMAGE_END, LABEL_MGMT_IMAGE_STATUS, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, MemBorrow, MemReadGrantControl, MemRelease, MgmtImageActivate,
        MgmtImageBegin, MgmtImageChunk, MgmtImageEnd, MgmtStatus, NetworkDatagramSendRouteControl,
        NetworkRejectRouteControl, NetworkStreamWriteRouteControl,
    },
    kernel::network::{DatagramAckMsg, DatagramSendMsg, StreamAckMsg, StreamWriteMsg},
    kernel::policy::{NodeImageUpdatedMsg, SwarmTelemetryMsg},
    kernel::remote::{
        RemoteActuateReqMsg, RemoteActuateRetMsg, RemoteSampleReqMsg, RemoteSampleRetMsg,
    },
};

type MgmtBeginMsg = Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>;
type MgmtChunkMsg = Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>;
type MgmtEndMsg = Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>;
type MgmtActivateMsg = Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>;
type MgmtStatusMsg = Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>;

macro_rules! seq_chain {
    ($head:expr, $($tail:expr),+ $(,)?) => {
        g::seq($head, seq_chain!($($tail),+))
    };
    ($last:expr $(,)?) => {
        $last
    };
}

macro_rules! sample_program {
    ($role:literal) => {
        seq_chain!(
            g::send::<Role<0>, Role<$role>, RemoteSampleReqMsg, 0>(),
            g::send::<Role<$role>, Role<0>, RemoteSampleRetMsg, 0>(),
        )
    };
}

macro_rules! wasip1_program {
    ($role:literal) => {
        seq_chain!(
            g::send::<Role<$role>, Role<0>, Msg<LABEL_MEM_BORROW_READ, MemBorrow>, 1>(),
            g::send::<Role<0>, Role<$role>, MemReadGrantControl, 1>(),
            g::send::<Role<$role>, Role<0>, Msg<LABEL_WASI_FD_WRITE, EngineReq>, 1>(),
            g::send::<Role<0>, Role<$role>, Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>, 1>(),
            g::send::<Role<$role>, Role<0>, Msg<LABEL_MEM_RELEASE, MemRelease>, 1>(),
        )
    };
}

macro_rules! wasip1_start_program {
    ($role:literal) => {
        seq_chain!(
            g::send::<Role<0>, Role<$role>, RemoteActuateReqMsg, 1>(),
            g::send::<Role<$role>, Role<0>, RemoteActuateRetMsg, 1>(),
        )
    };
}

macro_rules! aggregate_program {
    ($role:literal) => {
        seq_chain!(
            g::send::<Role<0>, Role<$role>, RemoteActuateReqMsg, 1>(),
            g::send::<Role<$role>, Role<0>, RemoteActuateRetMsg, 1>(),
        )
    };
}

macro_rules! actuator_program {
    ($role:literal) => {
        seq_chain!(
            g::send::<Role<0>, Role<$role>, RemoteActuateReqMsg, 18>(),
            g::send::<Role<$role>, Role<0>, RemoteActuateRetMsg, 18>(),
        )
    };
}

macro_rules! gateway_telemetry_program {
    ($source:literal, $gateway:literal) => {
        seq_chain!(
            g::send::<Role<$source>, Role<$gateway>, SwarmTelemetryMsg, 20>(),
            g::send::<Role<$gateway>, Role<0>, SwarmTelemetryMsg, 20>(),
        )
    };
}

macro_rules! network_object_program {
    ($gateway:literal) => {
        g::route(
            seq_chain!(
                g::send::<Role<0>, Role<0>, NetworkDatagramSendRouteControl, 22>(),
                g::send::<Role<0>, Role<$gateway>, DatagramSendMsg, 22>(),
                g::send::<Role<$gateway>, Role<0>, DatagramAckMsg, 22>(),
                g::send::<Role<0>, Role<0>, NetworkStreamWriteRouteControl, 23>(),
                g::send::<Role<0>, Role<$gateway>, StreamWriteMsg, 23>(),
                g::send::<Role<$gateway>, Role<0>, StreamAckMsg, 23>(),
            ),
            seq_chain!(
                g::send::<Role<0>, Role<0>, NetworkRejectRouteControl, 22>(),
                g::send::<Role<0>, Role<$gateway>, FdErrorMsg, 22>(),
            ),
        )
    };
}

macro_rules! management_program {
    ($managed:literal) => {
        seq_chain!(
            g::send::<Role<0>, Role<$managed>, MgmtBeginMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, MgmtStatusMsg, 19>(),
            g::send::<Role<0>, Role<$managed>, MgmtChunkMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, MgmtStatusMsg, 19>(),
            g::send::<Role<0>, Role<$managed>, MgmtEndMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, MgmtStatusMsg, 19>(),
            g::send::<Role<0>, Role<$managed>, MgmtActivateMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, MgmtStatusMsg, 19>(),
            g::send::<Role<0>, Role<$managed>, MgmtActivateMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, MgmtStatusMsg, 19>(),
            g::send::<Role<$managed>, Role<0>, NodeImageUpdatedMsg, 19>(),
        )
    };
}

macro_rules! swarm_program_2 {
    () => {
        seq_chain!(
            sample_program!(1),
            wasip1_start_program!(1),
            wasip1_program!(1),
            aggregate_program!(1)
        )
    };
}

macro_rules! swarm_program_3 {
    () => {
        seq_chain!(
            sample_program!(1),
            sample_program!(2),
            wasip1_start_program!(1),
            wasip1_program!(1),
            wasip1_start_program!(2),
            wasip1_program!(2),
            aggregate_program!(1),
            aggregate_program!(2),
        )
    };
}

macro_rules! swarm_program_4 {
    () => {
        seq_chain!(
            sample_program!(1),
            sample_program!(2),
            sample_program!(3),
            wasip1_start_program!(1),
            wasip1_program!(1),
            wasip1_start_program!(2),
            wasip1_program!(2),
            wasip1_start_program!(3),
            wasip1_program!(3),
            aggregate_program!(1),
            aggregate_program!(2),
            aggregate_program!(3),
        )
    };
}

macro_rules! swarm_program_5 {
    () => {
        seq_chain!(
            sample_program!(1),
            sample_program!(2),
            sample_program!(3),
            sample_program!(4),
            wasip1_start_program!(1),
            wasip1_program!(1),
            wasip1_start_program!(2),
            wasip1_program!(2),
            wasip1_start_program!(3),
            wasip1_program!(3),
            wasip1_start_program!(4),
            wasip1_program!(4),
            aggregate_program!(1),
            aggregate_program!(2),
            aggregate_program!(3),
            aggregate_program!(4),
        )
    };
}

macro_rules! swarm_program_6 {
    () => {
        seq_chain!(
            sample_program!(1),
            sample_program!(2),
            sample_program!(3),
            sample_program!(4),
            sample_program!(5),
            wasip1_start_program!(1),
            wasip1_program!(1),
            wasip1_start_program!(2),
            wasip1_program!(2),
            wasip1_start_program!(3),
            wasip1_program!(3),
            wasip1_start_program!(4),
            wasip1_program!(4),
            wasip1_start_program!(5),
            wasip1_program!(5),
            aggregate_program!(1),
            aggregate_program!(2),
            aggregate_program!(3),
            aggregate_program!(4),
            aggregate_program!(5),
            actuator_program!(2),
            gateway_telemetry_program!(2, 3),
            network_object_program!(3),
            management_program!(4),
        )
    };
}

static COORDINATOR_PROGRAM_2: RoleProgram<0> = project(&swarm_program_2!());
static COORDINATOR_PROGRAM_3: RoleProgram<0> = project(&swarm_program_3!());
static COORDINATOR_PROGRAM_4: RoleProgram<0> = project(&swarm_program_4!());
static COORDINATOR_PROGRAM_5: RoleProgram<0> = project(&swarm_program_5!());
static COORDINATOR_PROGRAM_6: RoleProgram<0> = project(&swarm_program_6!());

static ROLE1_PROGRAM_2: RoleProgram<1> = project(&swarm_program_2!());
static ROLE1_PROGRAM_3: RoleProgram<1> = project(&swarm_program_3!());
static ROLE1_PROGRAM_4: RoleProgram<1> = project(&swarm_program_4!());
static ROLE1_PROGRAM_5: RoleProgram<1> = project(&swarm_program_5!());
static ROLE1_PROGRAM_6: RoleProgram<1> = project(&swarm_program_6!());

static ROLE2_PROGRAM_3: RoleProgram<2> = project(&swarm_program_3!());
static ROLE2_PROGRAM_4: RoleProgram<2> = project(&swarm_program_4!());
static ROLE2_PROGRAM_5: RoleProgram<2> = project(&swarm_program_5!());
static ROLE2_PROGRAM_6: RoleProgram<2> = project(&swarm_program_6!());

static ROLE3_PROGRAM_4: RoleProgram<3> = project(&swarm_program_4!());
static ROLE3_PROGRAM_5: RoleProgram<3> = project(&swarm_program_5!());
static ROLE3_PROGRAM_6: RoleProgram<3> = project(&swarm_program_6!());

static ROLE4_PROGRAM_5: RoleProgram<4> = project(&swarm_program_5!());
static ROLE4_PROGRAM_6: RoleProgram<4> = project(&swarm_program_6!());

static ROLE5_PROGRAM_6: RoleProgram<5> = project(&swarm_program_6!());

pub fn coordinator_program_6() -> &'static RoleProgram<0> {
    &COORDINATOR_PROGRAM_6
}

pub fn role1_program_6() -> &'static RoleProgram<1> {
    &ROLE1_PROGRAM_6
}

pub fn role2_program_6() -> &'static RoleProgram<2> {
    &ROLE2_PROGRAM_6
}

pub fn role3_program_6() -> &'static RoleProgram<3> {
    &ROLE3_PROGRAM_6
}

pub fn role4_program_6() -> &'static RoleProgram<4> {
    &ROLE4_PROGRAM_6
}

pub fn role5_program_6() -> &'static RoleProgram<5> {
    &ROLE5_PROGRAM_6
}

pub fn coordinator_program_for(node_count: u8) -> Option<&'static RoleProgram<0>> {
    match node_count {
        2 => Some(&COORDINATOR_PROGRAM_2),
        3 => Some(&COORDINATOR_PROGRAM_3),
        4 => Some(&COORDINATOR_PROGRAM_4),
        5 => Some(&COORDINATOR_PROGRAM_5),
        6 => Some(&COORDINATOR_PROGRAM_6),
        _ => None,
    }
}

pub fn role1_program_for(node_count: u8) -> Option<&'static RoleProgram<1>> {
    match node_count {
        2 => Some(&ROLE1_PROGRAM_2),
        3 => Some(&ROLE1_PROGRAM_3),
        4 => Some(&ROLE1_PROGRAM_4),
        5 => Some(&ROLE1_PROGRAM_5),
        6 => Some(&ROLE1_PROGRAM_6),
        _ => None,
    }
}

pub fn role2_program_for(node_count: u8) -> Option<&'static RoleProgram<2>> {
    match node_count {
        3 => Some(&ROLE2_PROGRAM_3),
        4 => Some(&ROLE2_PROGRAM_4),
        5 => Some(&ROLE2_PROGRAM_5),
        6 => Some(&ROLE2_PROGRAM_6),
        _ => None,
    }
}

pub fn role3_program_for(node_count: u8) -> Option<&'static RoleProgram<3>> {
    match node_count {
        4 => Some(&ROLE3_PROGRAM_4),
        5 => Some(&ROLE3_PROGRAM_5),
        6 => Some(&ROLE3_PROGRAM_6),
        _ => None,
    }
}

pub fn role4_program_for(node_count: u8) -> Option<&'static RoleProgram<4>> {
    match node_count {
        5 => Some(&ROLE4_PROGRAM_5),
        6 => Some(&ROLE4_PROGRAM_6),
        _ => None,
    }
}

pub fn role5_program_for(node_count: u8) -> Option<&'static RoleProgram<5>> {
    match node_count {
        6 => Some(&ROLE5_PROGRAM_6),
        _ => None,
    }
}
