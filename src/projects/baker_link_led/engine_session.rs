#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use super::device_session::{
    gpio_device_recv_abort_terminal_entry_set_once, gpio_device_recv_abort_terminal_seq_set_once,
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::stages::*;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::status;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::status::mark_stage;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::status::record_failure_stage;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        not(feature = "baker-abort-safe-demo"),
        feature = "baker-recoverable-abort-demo"
    )
))]
use super::storage::BAKER_LINK_WASM_FUEL_PER_ACTIVATION;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::storage::EngineEndpoint;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use super::storage::GpioEndpoint;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::storage::{TEST_LED_PTR, TEST_MEMORY_EPOCH};

#[cfg(any(
    all(
        target_arch = "arm",
        target_os = "none",
        not(feature = "baker-abort-safe-demo")
    ),
    all(
        target_arch = "arm",
        target_os = "none",
        feature = "baker-recoverable-abort-demo"
    )
))]
use hibana::g::Msg;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::choreography::protocol::ChoreoFsOpenAdmitRouteMsg;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-choreofs-bad-path-demo"
))]
use hibana_pico::choreography::protocol::ChoreoFsOpenRejectRouteMsg;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-recoverable-abort-demo"
))]
use hibana_pico::choreography::protocol::{
    BudgetRunMsg, EngineReq, LABEL_WASI_PROC_EXIT, ProcExitStatus,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::choreography::protocol::{
    LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET, PathOpen,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
use hibana_pico::kernel::engine::wasm::PathKind;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::proof::baker_link::choreography::{
    BakerTrafficLoopBreakControl, BakerTrafficLoopContinueControl,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::{
    choreography::protocol::{
        BudgetRun, BudgetRunMsg, EngineReq, EngineRet, FdWrite, LABEL_MEM_BORROW_READ,
        LABEL_MEM_RELEASE, LABEL_WASI_FD_WRITE, LABEL_WASI_FD_WRITE_RET, LABEL_WASI_POLL_ONEOFF,
        LABEL_WASI_POLL_ONEOFF_RET, LABEL_WASI_PROC_EXIT, MemBorrow, MemReadGrantControl,
        MemRelease, MemRights, PollOneoff, ProcExitStatus,
    },
    kernel::{
        engine::wasm::{Call, Event, Guest},
        guest_ledger::WASI_ERRNO_SUCCESS,
    },
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::{
    choreography::protocol::{
        EngineAbort, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
        EngineAbortMsg, EngineAbortReason, EngineAbortRouteControl,
    },
    machine::rp2040::baker_link::BAKER_LINK_SAFE_GPIO_LEVELS,
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_fd_write(endpoint: &mut EngineEndpoint, fd: u8, payload: &[u8]) -> u16 {
    mark_stage(STAGE_ENGINE_FD_WRITE_BEGIN);
    let borrow = MemBorrow::new(TEST_LED_PTR, payload.len() as u8, TEST_MEMORY_EPOCH);
    let flow = endpoint
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_ENGINE_BORROW_FLOW_ERR);
            panic!()
        });
    match flow.send(&borrow).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_BORROW_SEND_ERR);
            panic!();
        }
    }
    mark_stage(STAGE_ENGINE_FD_WRITE_BORROW_SENT);

    let grant = match endpoint.recv::<MemReadGrantControl>().await {
        Ok(grant) => grant,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_GRANT_RECV_ERR);
            panic!()
        }
    };
    let (rights, lease_id) = grant.decode_handle().unwrap_or_else(|_| {
        record_failure_stage(STAGE_ENGINE_GRANT_DECODE_ERR);
        panic!()
    });
    if rights != MemRights::Read.tag() || lease_id > u8::MAX as u64 {
        record_failure_stage(STAGE_ENGINE_GRANT_MISMATCH);
        panic!();
    }

    let write = FdWrite::new_with_lease(fd, lease_id as u8, payload).unwrap_or_else(|_| panic!());
    let request = EngineReq::FdWrite(write);
    let flow = endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_REQ_FLOW_ERR);
            panic!()
        });
    match flow.send(&request).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_REQ_SEND_ERR);
            panic!()
        }
    }

    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_RECV_ERR);
            panic!()
        }
    };
    let EngineRet::FdWriteDone(done) = reply else {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    };
    if done.fd() != fd {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }
    if done.errno() == WASI_ERRNO_SUCCESS && done.written() != payload.len() as u8 {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }
    if done.errno() != WASI_ERRNO_SUCCESS && done.written() != 0 {
        record_failure_stage(STAGE_ENGINE_FD_WRITE_RET_MISMATCH);
        panic!();
    }

    let release = MemRelease::new(lease_id as u8);
    match endpoint
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<led release>")
        .send(&release)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    done.errno()
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    not(feature = "baker-bad-order-demo")
))]
async fn engine_poll_oneoff(endpoint: &mut EngineEndpoint, tick: u64) {
    let request = EngineReq::PollOneoff(PollOneoff::new(tick));
    match endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .expect("engine flow<led poll_oneoff>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }

    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => panic!(),
    };
    let EngineRet::PollReady(ready) = reply else {
        panic!();
    };
    if ready.ready() != 1 {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    feature = "baker-bad-order-demo"
))]
async fn engine_expect_poll_oneoff_rejected(endpoint: &mut EngineEndpoint) {
    if endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .is_ok()
    {
        panic!();
    }
    record_failure_stage(STAGE_BAD_ORDER_POLL_REJECTED);
    mark_stage(RESULT_EXPECTED_REJECT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo"),
    any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )
))]
async fn engine_path_open(
    endpoint: &mut EngineEndpoint,
    call: hibana_pico::kernel::engine::wasm::Pending<
        '_,
        '_,
        hibana_pico::kernel::engine::wasm::Path,
    >,
) {
    mark_stage(STAGE_ENGINE_PATH_OPEN_BEGIN);
    if call.kind() != PathKind::PathOpen {
        panic!();
    }
    let ptr = call.arg_i32(2).unwrap_or_else(|_| panic!());
    let len = call.arg_i32(3).unwrap_or_else(|_| panic!());
    if len > u8::MAX as u32 {
        panic!();
    }
    let preopen_fd = call.fd().unwrap_or_else(|_| panic!());
    let rights_base = call.arg_i64(5).unwrap_or_else(|_| panic!());

    let borrow = MemBorrow::new(ptr, len as u8, TEST_MEMORY_EPOCH);
    match endpoint
        .flow::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
        .expect("engine flow<path borrow>")
        .send(&borrow)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_BORROW_SENT);
    let grant = match endpoint.recv::<MemReadGrantControl>().await {
        Ok(grant) => grant,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_ENGINE_PATH_OPEN_GRANT_RECV);
    let (rights, lease_id) = grant.decode_handle().unwrap_or_else(|_| panic!());
    if rights != MemRights::Read.tag() || lease_id > u8::MAX as u64 {
        panic!();
    }

    let path = call.path_bytes().unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_PATH_OPEN_PATH_DECODED);
    let request = EngineReq::PathOpen(
        PathOpen::new(preopen_fd, lease_id as u8, rights_base, path.as_bytes())
            .unwrap_or_else(|_| panic!()),
    );
    match endpoint
        .flow::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
        .expect("engine flow<path_open>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_REQ_SENT);
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    {
        if endpoint.recv::<ChoreoFsOpenRejectRouteMsg>().await.is_err() {
            panic!();
        }
    }
    #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
    {
        if endpoint.recv::<ChoreoFsOpenAdmitRouteMsg>().await.is_err() {
            panic!();
        }
    }
    let reply = match endpoint
        .recv::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
        .await
    {
        Ok(reply) => reply,
        Err(_) => panic!(),
    };
    let EngineRet::PathOpened(opened) = reply else {
        panic!();
    };
    mark_stage(STAGE_ENGINE_PATH_OPEN_RET_RECV);

    let release = MemRelease::new(lease_id as u8);
    match endpoint
        .flow::<Msg<LABEL_MEM_RELEASE, MemRelease>>()
        .expect("engine flow<path release>")
        .send(&release)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PATH_OPEN_RELEASE_SENT);

    call.complete_path_open(opened.fd() as u32, opened.errno() as u32)
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_PATH_OPEN_COMPLETED);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
pub(super) async fn engine_session(endpoint: &mut EngineEndpoint, guest: &mut Guest<'static>) {
    #[cfg(not(feature = "baker-bad-order-demo"))]
    let mut tick = 0u64;
    let run = engine_recv_traffic_run(endpoint, 0).await;
    loop {
        let event = match guest.resume(run) {
            Ok(event) => event,
            Err(error) => {
                let _ = error;
                if status::mark_expected_reject_if_recorded() {
                    break;
                }
                record_failure_stage(STAGE_ENGINE_RESUME_ERR_TRAP);
                panic!();
            }
        };
        match event {
            Event::Call(Call::FdWrite(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_FD_WRITE);
                engine_continue_traffic_loop(endpoint).await;
                let payload = call.payload().unwrap_or_else(|_| panic!());
                let errno = engine_fd_write(endpoint, call.fd(), payload.as_bytes()).await;
                if call.complete(errno as u32).is_err() {
                    if status::mark_expected_reject_if_recorded() {
                        break;
                    }
                    panic!();
                }
            }
            Event::Call(Call::PollOneoff(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_POLL_ONEOFF);
                let delay_ticks = call.delay_ticks().unwrap_or_else(|_| panic!());
                #[cfg(feature = "baker-bad-order-demo")]
                {
                    if delay_ticks != 50 {
                        panic!();
                    }
                    engine_expect_poll_oneoff_rejected(endpoint).await;
                    break;
                }
                #[cfg(not(feature = "baker-bad-order-demo"))]
                {
                    tick = tick.saturating_add(delay_ticks);
                    engine_poll_oneoff(endpoint, tick).await;
                    call.complete(1, 0).unwrap_or_else(|_| panic!());
                }
            }
            Event::Call(
                Call::FdRead(_)
                | Call::FdFdstatGet(_)
                | Call::FdClose(_)
                | Call::ClockResGet(_)
                | Call::ClockTimeGet(_)
                | Call::RandomGet(_)
                | Call::SchedYield(_)
                | Call::Socket(_)
                | Call::ProcRaise(_),
            ) => {
                mark_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                record_failure_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                panic!();
            }
            Event::Call(Call::Path(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_PATH_OPEN);
                #[cfg(any(
                    feature = "baker-choreofs-demo",
                    feature = "baker-choreofs-bad-path-demo",
                    feature = "baker-choreofs-bad-payload-demo",
                    feature = "baker-choreofs-wrong-object-demo"
                ))]
                {
                    engine_path_open(endpoint, call).await;
                }
                #[cfg(not(any(
                    feature = "baker-choreofs-demo",
                    feature = "baker-choreofs-bad-path-demo",
                    feature = "baker-choreofs-bad-payload-demo",
                    feature = "baker-choreofs-wrong-object-demo"
                )))]
                {
                    let _ = call;
                    record_failure_stage(STAGE_ENGINE_TRAP_PATH_OPEN);
                    panic!();
                }
            }
            Event::Call(Call::ArgsSizesGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ARGS_SIZES);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ARGS_SIZES);
                panic!();
            }
            Event::Call(Call::ArgsGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ARGS_GET);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ARGS_GET);
                panic!();
            }
            Event::Call(Call::EnvironSizesGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ENVIRON_SIZES);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ENVIRON_SIZES);
                panic!();
            }
            Event::Call(Call::EnvironGet(call)) => {
                mark_stage(STAGE_ENGINE_TRAP_ENVIRON_GET);
                let _ = call;
                record_failure_stage(STAGE_ENGINE_TRAP_ENVIRON_GET);
                panic!();
            }
            Event::Exit(status) => {
                if status.status() > u8::MAX as u32 {
                    panic!();
                }
                engine_break_traffic_loop(endpoint).await;
                engine_proc_exit(endpoint, status.status() as u8).await;
                break;
            }
            Event::Call(Call::MemoryGrow(_)) => {
                mark_stage(STAGE_ENGINE_TRAP_MEMORY_GROW);
                record_failure_stage(STAGE_ENGINE_TRAP_MEMORY_GROW);
                panic!();
            }
            Event::BudgetExpired(_) => {
                mark_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                record_failure_stage(STAGE_ENGINE_TRAP_UNSUPPORTED);
                panic!();
            }
            Event::Done => {
                if status::mark_expected_reject_if_recorded() {
                    break;
                }
                engine_break_traffic_loop(endpoint).await;
                engine_proc_exit(endpoint, 0).await;
                break;
            }
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
pub(super) async fn engine_abort_safe_session(
    endpoint: &mut EngineEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
) {
    #[cfg(feature = "baker-recoverable-abort-demo")]
    engine_recv_recoverable_budget_run(endpoint, 0).await;

    mark_stage(STAGE_ENGINE_ABORT_ROUTE_SENT);

    endpoint
        .flow::<EngineAbortRouteControl>()
        .expect("engine flow<abort route>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());

    let abort = EngineAbort::new(EngineAbortReason::GuestTrap, 1);
    endpoint
        .flow::<EngineAbortMsg>()
        .expect("engine flow<abort reason>")
        .send(&abort)
        .await
        .unwrap_or_else(|_| panic!());
    endpoint
        .flow::<EngineAbortBeginControl>()
        .expect("engine flow<abort begin>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());

    endpoint
        .recv::<EngineAbortFenceControl>()
        .await
        .unwrap_or_else(|_| panic!());
    gpio_device_recv_abort_terminal_entry_set_once(gpio_endpoint).await;
    for _ in 1..BAKER_LINK_SAFE_GPIO_LEVELS.len() {
        gpio_device_recv_abort_terminal_seq_set_once(gpio_endpoint).await;
    }
    endpoint
        .recv::<EngineAbortAckControl>()
        .await
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_ENGINE_ABORT_ACK_RECV);
    #[cfg(feature = "baker-recoverable-abort-demo")]
    {
        engine_recv_recoverable_budget_run(endpoint, 1).await;
        mark_stage(STAGE_ENGINE_ABORT_REENTER_RUN_RECV);
        let request = EngineReq::ProcExit(ProcExitStatus::new(0));
        endpoint
            .flow::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
            .expect("engine flow<recoverable proc_exit>")
            .send(&request)
            .await
            .unwrap_or_else(|_| panic!());
    }
    #[cfg(not(feature = "baker-recoverable-abort-demo"))]
    mark_stage(RESULT_ABORT_SAFE_OK);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-recoverable-abort-demo"
))]
async fn engine_recv_recoverable_budget_run(endpoint: &mut EngineEndpoint, expected_run_id: u16) {
    let run = endpoint
        .recv::<BudgetRunMsg>()
        .await
        .unwrap_or_else(|_| panic!());
    if run.run_id() != expected_run_id
        || run.generation() != expected_run_id.saturating_add(1)
        || run.fuel() != BAKER_LINK_WASM_FUEL_PER_ACTIVATION
    {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_continue_traffic_loop(endpoint: &mut EngineEndpoint) {
    match endpoint
        .flow::<BakerTrafficLoopContinueControl>()
        .expect("engine flow<traffic loop continue>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_LOOP_CONTINUE_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_break_traffic_loop(endpoint: &mut EngineEndpoint) {
    match endpoint
        .flow::<BakerTrafficLoopBreakControl>()
        .expect("engine flow<traffic loop break>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_LOOP_BREAK_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_proc_exit(endpoint: &mut EngineEndpoint, code: u8) {
    let request = EngineReq::ProcExit(ProcExitStatus::new(code));
    match endpoint
        .flow::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
        .expect("engine flow<proc_exit>")
        .send(&request)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_ENGINE_PROC_EXIT_SENT);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn engine_recv_traffic_run(endpoint: &mut EngineEndpoint, expected_cycle: u16) -> BudgetRun {
    mark_stage(STAGE_ENGINE_RUN_RECV_BEGIN);
    let run = match endpoint.recv::<BudgetRunMsg>().await {
        Ok(run) => run,
        Err(_) => {
            record_failure_stage(STAGE_ENGINE_RUN_RECV_ERR);
            panic!()
        }
    };
    if run.run_id() != expected_cycle
        || run.generation() != 1
        || run.fuel() != BAKER_LINK_WASM_FUEL_PER_ACTIVATION
    {
        {
            record_failure_stage(STAGE_ENGINE_RUN_MISMATCH);
            panic!()
        };
    }
    mark_stage(STAGE_ENGINE_RUN_RECV_DONE);
    run
}
