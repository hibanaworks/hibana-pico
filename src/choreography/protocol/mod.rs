use hibana::{
    g::Msg,
    substrate::{
        cap::{
            CapShot, ControlResourceKind, GenericCapToken, ResourceKind,
            advanced::{
                CAP_HANDLE_LEN, CapError, ControlOp, ControlPath, ControlScopeKind,
                RouteDecisionKind, ScopeId,
            },
        },
        ids::{Lane, SessionId},
        wire::{CodecError, Payload, WireEncode, WirePayload},
    },
};

mod labels;
pub use labels::*;
mod route;
pub use route::*;
mod control;
pub use control::*;
mod management;
pub use management::*;
mod wasi;
pub use wasi::*;
#[cfg(test)]
mod tests {
    use super::{
        ArgsDone, ArgsGet, ArgsSizes, ArgsSizesGet, BudgetExpired, BudgetRestart, BudgetRun,
        BudgetSuspend, ClockNow, ClockResGet, ClockResolution, ClockTimeGet, EngineAbort,
        EngineAbortReason, EngineReq, EngineRet, EnvironDone, EnvironGet, EnvironSizes,
        EnvironSizesGet, FdClosed, FdRead, FdReadDone, FdRequest, FdStat, FdWrite, FdWriteDone,
        MGMT_IMAGE_CHUNK_CAPACITY, MemBorrow, MemCommit, MemFence, MemFenceReason, MemGrant,
        MemRelease, MemRights, MgmtImageActivate, MgmtImageBegin, MgmtImageChunk, MgmtImageEnd,
        MgmtImageRollback, MgmtStatus, MgmtStatusCode, PathOpen, PathOpened, PollOneoff, PollReady,
        ProcExitStatus, RandomDone, RandomGet, RandomSeed, StderrChunk, StdinChunk, StdinRequest,
        StdoutChunk, Wasip1ExitStatus,
    };
    use hibana::substrate::{
        cap::{
            CapShot, ControlResourceKind, ResourceKind,
            advanced::{ControlOp, ControlPath, ControlScopeKind, ScopeId},
        },
        ids::{Lane, SessionId},
        wire::{CodecError, Payload, WireEncode, WirePayload},
    };

    fn encode<T: WireEncode>(value: &T, out: &mut [u8]) -> usize {
        value.encode_into(out).expect("encode payload")
    }

    #[test]
    fn plain_payload_labels_avoid_hibana_reserved_control_labels() {
        let labels = [
            super::LABEL_WASI_FD_WRITE,
            super::LABEL_WASI_FD_WRITE_RET,
            super::LABEL_WASI_FD_READ,
            super::LABEL_WASI_FD_READ_RET,
            super::LABEL_WASI_FD_FDSTAT_GET,
            super::LABEL_WASI_FD_FDSTAT_GET_RET,
            super::LABEL_WASI_FD_CLOSE,
            super::LABEL_WASI_FD_CLOSE_RET,
            super::LABEL_WASI_CLOCK_RES_GET,
            super::LABEL_WASI_CLOCK_RES_GET_RET,
            super::LABEL_WASI_CLOCK_TIME_GET,
            super::LABEL_WASI_CLOCK_TIME_GET_RET,
            super::LABEL_WASI_POLL_ONEOFF,
            super::LABEL_WASI_POLL_ONEOFF_RET,
            super::LABEL_WASI_RANDOM_GET,
            super::LABEL_WASI_RANDOM_GET_RET,
            super::LABEL_WASI_PROC_EXIT,
            super::LABEL_WASI_ARGS_SIZES_GET,
            super::LABEL_WASI_ARGS_SIZES_GET_RET,
            super::LABEL_WASI_ARGS_GET,
            super::LABEL_WASI_ARGS_GET_RET,
            super::LABEL_WASI_ENVIRON_SIZES_GET,
            super::LABEL_WASI_ENVIRON_SIZES_GET_RET,
            super::LABEL_WASI_ENVIRON_GET,
            super::LABEL_WASI_ENVIRON_GET_RET,
            super::LABEL_WASI_PATH_OPEN,
            super::LABEL_WASI_PATH_OPEN_RET,
            super::LABEL_ENGINE_RUN,
            super::LABEL_ENGINE_BUDGET_EXPIRED,
            super::LABEL_ENGINE_SUSPEND,
            super::LABEL_ENGINE_RESTART,
            super::LABEL_NET_STREAM_WRITE,
            super::LABEL_NET_STREAM_ACK,
            super::LABEL_NET_STREAM_READ,
            super::LABEL_NET_STREAM_READ_RET,
        ];
        for label in labels {
            assert_ne!(label, 48);
            assert_ne!(label, 49);
            assert_ne!(label, 57);
            assert!(
                label
                    <= <super::BuiltInLabelUniverse as hibana::substrate::runtime::LabelUniverse>::MAX_LABEL
            );
        }
    }

    #[test]
    fn route_control_arm_ids_are_distinct_and_scope_preserving() {
        let scope = ScopeId::route(42);
        let sid = SessionId::new(7);
        let lane = Lane::new(17);

        let remote_sensor =
            <super::RemoteSensorRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);
        let remote_actuator =
            <super::RemoteActuatorRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);
        let remote_management =
            <super::RemoteManagementRouteKind as ControlResourceKind>::mint_handle(
                sid, lane, scope,
            );
        let remote_telemetry =
            <super::RemoteTelemetryRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);
        let remote_reject =
            <super::RemoteRejectRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);

        assert_eq!(remote_sensor, (0, scope.raw()));
        assert_eq!(remote_actuator, (1, scope.raw()));
        assert_eq!(remote_management, (2, scope.raw()));
        assert_eq!(remote_telemetry, (3, scope.raw()));
        assert_eq!(remote_reject, (4, scope.raw()));

        let network_datagram_send =
            <super::NetworkDatagramSendRouteKind as ControlResourceKind>::mint_handle(
                sid, lane, scope,
            );
        let network_datagram_recv =
            <super::NetworkDatagramRecvRouteKind as ControlResourceKind>::mint_handle(
                sid, lane, scope,
            );
        let network_stream_write =
            <super::NetworkStreamWriteRouteKind as ControlResourceKind>::mint_handle(
                sid, lane, scope,
            );
        let network_stream_read =
            <super::NetworkStreamReadRouteKind as ControlResourceKind>::mint_handle(
                sid, lane, scope,
            );
        let network_reject =
            <super::NetworkRejectRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);
        let network_accept =
            <super::NetworkAcceptRouteKind as ControlResourceKind>::mint_handle(sid, lane, scope);

        assert_eq!(network_datagram_send, (0, scope.raw()));
        assert_eq!(network_datagram_recv, (1, scope.raw()));
        assert_eq!(network_stream_write, (2, scope.raw()));
        assert_eq!(network_stream_read, (3, scope.raw()));
        assert_eq!(network_reject, (4, scope.raw()));
        assert_eq!(network_accept, (5, scope.raw()));
    }

    #[test]
    fn engine_abort_control_uses_hibana_abort_fence_ack_ops() {
        let scope = ScopeId::generic(9);
        let sid = SessionId::new(11);
        let lane = Lane::new(1);

        assert_eq!(
            <super::EngineAbortBeginKind as ControlResourceKind>::SCOPE,
            ControlScopeKind::Abort
        );
        assert_eq!(
            <super::EngineAbortBeginKind as ControlResourceKind>::SHOT,
            CapShot::One
        );
        assert_eq!(
            <super::EngineAbortBeginKind as ControlResourceKind>::PATH,
            ControlPath::Wire
        );
        assert_eq!(
            <super::EngineAbortBeginKind as ControlResourceKind>::OP,
            ControlOp::AbortBegin
        );
        assert_eq!(
            <super::EngineAbortFenceKind as ControlResourceKind>::OP,
            ControlOp::Fence
        );
        assert_eq!(
            <super::EngineAbortAckKind as ControlResourceKind>::OP,
            ControlOp::AbortAck
        );

        assert_eq!(
            <super::EngineAbortBeginKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (sid.raw(), lane.raw() as u16)
        );
        assert_eq!(
            <super::EngineAbortFenceKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (sid.raw(), lane.raw() as u16)
        );
        assert_eq!(
            <super::EngineAbortAckKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (sid.raw(), lane.raw() as u16)
        );
    }

    #[test]
    fn activation_authority_and_activation_use_many_to_one_cap_delegate() {
        let scope = ScopeId::generic(13);
        let sid = SessionId::new(12);
        let lane = Lane::new(1);

        assert_eq!(
            <super::ActivationAuthorityKind as ControlResourceKind>::OP,
            ControlOp::CapDelegate
        );
        assert_eq!(
            <super::ActivationAuthorityKind as ControlResourceKind>::SHOT,
            CapShot::Many
        );
        assert_eq!(
            <super::ActivationKind as ControlResourceKind>::OP,
            ControlOp::CapDelegate
        );
        assert_eq!(
            <super::ActivationKind as ControlResourceKind>::SHOT,
            CapShot::One
        );
        assert_eq!(
            <super::ActivationAuthorityKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (0, scope.raw())
        );
        assert_eq!(
            <super::ActivationKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (1, scope.raw())
        );
    }

    #[test]
    fn topology_transaction_and_state_controls_use_hibana_control_ops() {
        let scope = ScopeId::generic(21);
        let sid = SessionId::new(22);
        let lane = Lane::new(1);

        assert_eq!(
            <super::TopologyBeginKind as ControlResourceKind>::SCOPE,
            ControlScopeKind::Topology
        );
        assert_eq!(
            <super::TopologyBeginKind as ControlResourceKind>::OP,
            ControlOp::TopologyBegin
        );
        assert_eq!(
            <super::TopologyAckKind as ControlResourceKind>::OP,
            ControlOp::TopologyAck
        );
        assert_eq!(
            <super::TopologyCommitKind as ControlResourceKind>::OP,
            ControlOp::TopologyCommit
        );
        assert_eq!(
            <super::TopologyBeginKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (0, scope.raw())
        );
        assert_eq!(
            <super::TopologyAckKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (1, scope.raw())
        );
        assert_eq!(
            <super::TopologyCommitKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (2, scope.raw())
        );

        assert_eq!(
            <super::TxCommitKind as ControlResourceKind>::OP,
            ControlOp::TxCommit
        );
        assert_eq!(
            <super::TxAbortKind as ControlResourceKind>::OP,
            ControlOp::TxAbort
        );
        assert_eq!(
            <super::TxCommitKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (0, scope.raw())
        );
        assert_eq!(
            <super::TxAbortKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (1, scope.raw())
        );

        assert_eq!(
            <super::StateSnapshotKind as ControlResourceKind>::SCOPE,
            ControlScopeKind::State
        );
        assert_eq!(
            <super::StateSnapshotKind as ControlResourceKind>::OP,
            ControlOp::StateSnapshot
        );
        assert_eq!(
            <super::StateRestoreKind as ControlResourceKind>::OP,
            ControlOp::StateRestore
        );
        assert_eq!(
            <super::StateSnapshotKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (0, scope.raw())
        );
        assert_eq!(
            <super::StateRestoreKind as ControlResourceKind>::mint_handle(sid, lane, scope),
            (1, scope.raw())
        );
    }

    #[test]
    fn engine_abort_payload_round_trips() {
        let abort = EngineAbort::new(EngineAbortReason::FuelExhausted, 17);
        let mut buf = [0u8; 3];
        let len = encode(&abort, &mut buf);
        let decoded = EngineAbort::decode_payload(Payload::new(&buf[..len]))
            .expect("decode engine abort payload");
        assert_eq!(decoded.reason(), EngineAbortReason::FuelExhausted);
        assert_eq!(decoded.code(), 17);
        assert_eq!(decoded, abort);
    }

    #[test]
    fn abort_control_handles_round_trip() {
        let handle = (0x1122_3344u32, 7u16);
        let encoded = <super::EngineAbortAckKind as ResourceKind>::encode_handle(&handle);
        let decoded = <super::EngineAbortAckKind as ResourceKind>::decode_handle(encoded)
            .expect("decode abort control handle");
        assert_eq!(decoded, handle);
    }

    #[test]
    fn engine_req_round_trips_log_u32() {
        let req = EngineReq::LogU32(0x4849_4241);
        let mut buf = [0u8; 5];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_yield() {
        let req = EngineReq::Yield;
        let mut buf = [0u8; 1];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_stdout() {
        let req = EngineReq::Wasip1Stdout(
            StdoutChunk::new_with_lease(1, b"hibana wasip1 stdout\n").expect("chunk"),
        );
        let mut buf = [0u8; 33];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_stderr() {
        let req = EngineReq::Wasip1Stderr(
            StderrChunk::new_with_lease(2, b"hibana wasip1 stderr\n").expect("chunk"),
        );
        let mut buf = [0u8; 33];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_stdin() {
        let req = EngineReq::Wasip1Stdin(StdinRequest::new_with_lease(3, 24).expect("request"));
        let mut buf = [0u8; 3];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_clock_now() {
        let req = EngineReq::Wasip1ClockNow;
        let mut buf = [0u8; 1];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_random_seed() {
        let req = EngineReq::Wasip1RandomSeed;
        let mut buf = [0u8; 1];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasip1_exit() {
        let req = EngineReq::Wasip1Exit(Wasip1ExitStatus::new(7));
        let mut buf = [0u8; 2];
        let len = encode(&req, &mut buf);
        let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
        assert_eq!(decoded, req);
    }

    #[test]
    fn engine_req_round_trips_wasi_p1_subset() {
        let requests = [
            EngineReq::FdWrite(FdWrite::new_with_lease(1, 11, b"stdout").expect("fd_write")),
            EngineReq::FdRead(FdRead::new_with_lease(0, 12, 8).expect("fd_read")),
            EngineReq::FdFdstatGet(FdRequest::new(1)),
            EngineReq::FdClose(FdRequest::new(4)),
            EngineReq::ClockResGet(ClockResGet::new(0)),
            EngineReq::ClockTimeGet(ClockTimeGet::new(0, 1000)),
            EngineReq::PollOneoff(PollOneoff::new(44)),
            EngineReq::RandomGet(RandomGet::new_with_lease(13, 8).expect("random_get")),
            EngineReq::ProcExit(ProcExitStatus::new(7)),
            EngineReq::ArgsSizesGet(ArgsSizesGet::new()),
            EngineReq::ArgsGet(ArgsGet::new_with_lease(14, 16).expect("args_get")),
            EngineReq::EnvironSizesGet(EnvironSizesGet::new()),
            EngineReq::EnvironGet(EnvironGet::new_with_lease(15, 16).expect("environ_get")),
            EngineReq::PathOpen(
                PathOpen::new(9, 16, 1 << 6, b"object/traffic").expect("path_open"),
            ),
        ];
        let mut buf = [0u8; 80];
        for req in requests {
            let len = encode(&req, &mut buf);
            let decoded = EngineReq::decode_payload(Payload::new(&buf[..len])).expect("decode req");
            assert_eq!(decoded, req);
        }
    }

    #[test]
    fn engine_ret_round_trips_path_opened() {
        let ret = EngineRet::PathOpened(PathOpened::new(3, 0));
        let mut buf = [0u8; 4];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_logged() {
        let ret = EngineRet::Logged(0x4849_4241);
        let mut buf = [0u8; 5];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_yielded() {
        let ret = EngineRet::Yielded;
        let mut buf = [0u8; 1];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasip1_stdout_written() {
        let ret = EngineRet::Wasip1StdoutWritten(21);
        let mut buf = [0u8; 2];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasip1_stderr_written() {
        let ret = EngineRet::Wasip1StderrWritten(21);
        let mut buf = [0u8; 2];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasip1_stdin_read() {
        let ret = EngineRet::Wasip1StdinRead(
            StdinChunk::new_with_lease(4, b"hibana stdin\n").expect("chunk"),
        );
        let mut buf = [0u8; 33];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasip1_clock_now() {
        let ret = EngineRet::Wasip1ClockNow(ClockNow::new(123_456_789));
        let mut buf = [0u8; 9];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasip1_random_seed() {
        let ret = EngineRet::Wasip1RandomSeed(RandomSeed::new(
            0x4849_4241_5241_4e44,
            0x5345_4544_0000_0001,
        ));
        let mut buf = [0u8; 17];
        let len = encode(&ret, &mut buf);
        let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
        assert_eq!(decoded, ret);
    }

    #[test]
    fn engine_ret_round_trips_wasi_p1_subset() {
        let replies = [
            EngineRet::FdWriteDone(FdWriteDone::new(1, 6)),
            EngineRet::FdReadDone(FdReadDone::new_with_lease(0, 12, b"stdin").expect("fd_read")),
            EngineRet::FdStat(FdStat::new(1, MemRights::Write)),
            EngineRet::FdClosed(FdClosed::new(4)),
            EngineRet::ClockResolution(ClockResolution::new(1_000_000)),
            EngineRet::ClockTime(ClockNow::new(123_456_789)),
            EngineRet::PollReady(PollReady::new(1)),
            EngineRet::RandomDone(RandomDone::new_with_lease(13, b"12345678").expect("random")),
            EngineRet::ArgsSizes(ArgsSizes::new(1, 4)),
            EngineRet::ArgsDone(ArgsDone::new_with_lease(14, b"arg").expect("args")),
            EngineRet::EnvironSizes(EnvironSizes::new(1, 4)),
            EngineRet::EnvironDone(EnvironDone::new_with_lease(15, b"K=V").expect("env")),
        ];
        let mut buf = [0u8; 40];
        for ret in replies {
            let len = encode(&ret, &mut buf);
            let decoded = EngineRet::decode_payload(Payload::new(&buf[..len])).expect("decode ret");
            assert_eq!(decoded, ret);
        }
    }

    #[test]
    fn budget_control_payloads_round_trip() {
        let run = BudgetRun::new(7, 3, 1000, 123_456);
        let mut run_buf = [0u8; 16];
        let run_len = encode(&run, &mut run_buf);
        assert_eq!(
            BudgetRun::decode_payload(Payload::new(&run_buf[..run_len])).expect("decode run"),
            run
        );

        let expired = BudgetExpired::new(7, 3);
        let mut expired_buf = [0u8; 4];
        let expired_len = encode(&expired, &mut expired_buf);
        assert_eq!(
            BudgetExpired::decode_payload(Payload::new(&expired_buf[..expired_len]))
                .expect("decode expired"),
            expired
        );

        let suspend = BudgetSuspend::new(7, 3);
        let mut suspend_buf = [0u8; 4];
        let suspend_len = encode(&suspend, &mut suspend_buf);
        assert_eq!(
            BudgetSuspend::decode_payload(Payload::new(&suspend_buf[..suspend_len]))
                .expect("decode suspend"),
            suspend
        );

        let restart = BudgetRestart::new(8, 4, 500, 124_000);
        let mut restart_buf = [0u8; 16];
        let restart_len = encode(&restart, &mut restart_buf);
        assert_eq!(
            BudgetRestart::decode_payload(Payload::new(&restart_buf[..restart_len]))
                .expect("decode restart"),
            restart
        );
    }

    #[test]
    fn memory_control_payloads_round_trip() {
        let borrow = MemBorrow::new(0x1000, 21, 3);
        let mut borrow_buf = [0u8; 9];
        let borrow_len = encode(&borrow, &mut borrow_buf);
        assert_eq!(
            MemBorrow::decode_payload(Payload::new(&borrow_buf[..borrow_len]))
                .expect("decode borrow"),
            borrow
        );

        let grant = MemGrant::new(5, 0x1000, 21, 3, MemRights::Read);
        let mut grant_buf = [0u8; 11];
        let grant_len = encode(&grant, &mut grant_buf);
        assert_eq!(
            MemGrant::decode_payload(Payload::new(&grant_buf[..grant_len])).expect("decode grant"),
            grant
        );

        let release = MemRelease::new(5);
        let mut release_buf = [0u8; 1];
        let release_len = encode(&release, &mut release_buf);
        assert_eq!(
            MemRelease::decode_payload(Payload::new(&release_buf[..release_len]))
                .expect("decode release"),
            release
        );

        let commit = MemCommit::new(5, 12);
        let mut commit_buf = [0u8; 2];
        let commit_len = encode(&commit, &mut commit_buf);
        assert_eq!(
            MemCommit::decode_payload(Payload::new(&commit_buf[..commit_len]))
                .expect("decode commit"),
            commit
        );

        let fence = MemFence::new(MemFenceReason::HotSwap, 4);
        let mut fence_buf = [0u8; 5];
        let fence_len = encode(&fence, &mut fence_buf);
        assert_eq!(
            MemFence::decode_payload(Payload::new(&fence_buf[..fence_len])).expect("decode fence"),
            fence
        );
    }

    #[test]
    fn management_payloads_round_trip() {
        let begin = MgmtImageBegin::new(1, 128, 7);
        let mut begin_buf = [0u8; 9];
        let begin_len = encode(&begin, &mut begin_buf);
        assert_eq!(
            MgmtImageBegin::decode_payload(Payload::new(&begin_buf[..begin_len]))
                .expect("decode begin"),
            begin
        );

        let chunk = MgmtImageChunk::new(1, 24, b"hibana-image-chunk").expect("chunk");
        let mut chunk_buf = [0u8; 6 + MGMT_IMAGE_CHUNK_CAPACITY];
        let chunk_len = encode(&chunk, &mut chunk_buf);
        assert_eq!(
            MgmtImageChunk::decode_payload(Payload::new(&chunk_buf[..chunk_len]))
                .expect("decode chunk"),
            chunk
        );

        let end = MgmtImageEnd::new(1, 128);
        let mut end_buf = [0u8; 5];
        let end_len = encode(&end, &mut end_buf);
        assert_eq!(
            MgmtImageEnd::decode_payload(Payload::new(&end_buf[..end_len])).expect("decode end"),
            end
        );

        let activate = MgmtImageActivate::new(1, 8);
        let mut activate_buf = [0u8; 5];
        let activate_len = encode(&activate, &mut activate_buf);
        assert_eq!(
            MgmtImageActivate::decode_payload(Payload::new(&activate_buf[..activate_len]))
                .expect("decode activate"),
            activate
        );

        let rollback = MgmtImageRollback::new(0);
        let mut rollback_buf = [0u8; 1];
        let rollback_len = encode(&rollback, &mut rollback_buf);
        assert_eq!(
            MgmtImageRollback::decode_payload(Payload::new(&rollback_buf[..rollback_len]))
                .expect("decode rollback"),
            rollback
        );

        for code in [
            MgmtStatusCode::Ok,
            MgmtStatusCode::InvalidImage,
            MgmtStatusCode::NeedFence,
            MgmtStatusCode::BadSlot,
            MgmtStatusCode::RollbackEmpty,
            MgmtStatusCode::BadFenceEpoch,
            MgmtStatusCode::AuthFailed,
            MgmtStatusCode::BadSessionGeneration,
            MgmtStatusCode::ImageTooLarge,
            MgmtStatusCode::OffsetMismatch,
            MgmtStatusCode::LengthMismatch,
            MgmtStatusCode::BadChunkIndex,
        ] {
            let status = MgmtStatus::new(1, code);
            let mut status_buf = [0u8; 2];
            let status_len = encode(&status, &mut status_buf);
            assert_eq!(
                MgmtStatus::decode_payload(Payload::new(&status_buf[..status_len]))
                    .expect("decode status"),
                status
            );
        }
    }

    #[test]
    fn management_payloads_reject_on_invalid_encoding() {
        let oversized = [0u8; MGMT_IMAGE_CHUNK_CAPACITY + 1];
        assert!(matches!(
            MgmtImageChunk::new(1, 0, &oversized),
            Err(CodecError::Invalid(_))
        ));
        assert!(matches!(
            MgmtStatus::decode_payload(Payload::new(&[1, 99])),
            Err(CodecError::Invalid(_))
        ));
        assert!(matches!(
            MgmtImageChunk::decode_payload(Payload::new(&[1, 0, 0, 0, 0, 4, 1])),
            Err(CodecError::Invalid(_))
        ));
    }
}
