#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::device_session::{gpio_device_recv_set_once, timer_device_recv_sleep_once};
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
use super::storage::BAKER_LINK_WASM_FUEL_PER_ACTIVATION;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::storage::{BakerLedger, GpioEndpoint, TimerEndpoint};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::storage::{KernelEndpoint, TEST_LED_PTR, TEST_MEMORY_EPOCH, TEST_MEMORY_LEN};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::g::Msg;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::choreography::protocol::PollOneoff;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-recoverable-abort-demo"
))]
use hibana_pico::choreography::protocol::{
    BudgetRun, BudgetRunMsg, EngineReq, LABEL_WASI_PROC_EXIT,
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
    ChoreoFsOpenAdmitRoute, ChoreoFsOpenAdmitRouteMsg, ChoreoFsOpenRejectRoute,
    ChoreoFsOpenRejectRouteMsg, LABEL_WASI_PATH_OPEN, LABEL_WASI_PATH_OPEN_RET,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::choreography::protocol::{
    EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl, EngineAbortMsg,
    EngineAbortReason, LABEL_ENGINE_ABORT_REASON,
};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::choreography::protocol::{
    GpioSet, LABEL_GPIO_SET, LABEL_GPIO_SET_DONE, MemBorrow, MemRelease, TimerSleepDone,
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
use hibana_pico::machine::rp2040::baker_link::BAKER_LINK_SAFE_GPIO_LEVELS;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::port::exec::park;
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
use hibana_pico::{
    choreography::protocol::PathOpened,
    projects::baker_link_led::ledger::{
        baker_link_choreofs_ledger, mint_baker_link_choreofs_fd, resolve_baker_link_choreofs_path,
    },
    projects::baker_link_led::manifest::{
        BAKER_LINK_CHOREOFS_PREOPEN_FD, BakerLinkLedResourceStore, baker_link_led_resource_store,
    },
};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::{
    choreography::protocol::{
        BudgetRun, BudgetRunMsg, EngineReq, EngineRet, FdWriteDone, LABEL_MEM_BORROW_READ,
        LABEL_MEM_RELEASE, LABEL_TIMER_SLEEP_DONE, LABEL_TIMER_SLEEP_UNTIL, LABEL_WASI_FD_WRITE,
        LABEL_WASI_FD_WRITE_RET, LABEL_WASI_POLL_ONEOFF, LABEL_WASI_POLL_ONEOFF_RET,
        LABEL_WASI_PROC_EXIT, MemReadGrantControl, PollReady, TimerSleepUntil,
        WASIP1_STREAM_CHUNK_CAPACITY,
    },
    kernel::{
        fd_object::check_gpio_object_fd_write, guest_ledger::WASI_ERRNO_SUCCESS,
        resolver::PicoInterruptResolver,
    },
    projects::baker_link_led::manifest::{
        BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS, baker_link_led_fd_write_route,
    },
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_send_gpio_set(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    set: GpioSet,
) {
    match endpoint
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<gpio set>")
        .send(&set)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    gpio_device_recv_set_once(gpio_endpoint).await;
    let done = match endpoint.recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>().await {
        Ok(done) => done,
        Err(_) => panic!(),
    };
    if done != set {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
async fn kernel_send_gpio_set_remote(endpoint: &mut KernelEndpoint, set: GpioSet) {
    endpoint
        .flow::<Msg<LABEL_GPIO_SET, GpioSet>>()
        .expect("kernel flow<abort safe gpio set>")
        .send(&set)
        .await
        .unwrap_or_else(|_| panic!());
    let done = endpoint
        .recv::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .await
        .unwrap_or_else(|_| panic!());
    if done != set {
        panic!();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_fd_write(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    ledger: &mut BakerLedger,
    borrow: MemBorrow,
) {
    mark_stage(STAGE_KERNEL_FD_WRITE_BEGIN);
    mark_stage(STAGE_KERNEL_FD_WRITE_BORROW_RECV);
    if borrow.ptr() != TEST_LED_PTR
        || borrow.len() == 0
        || borrow.len() as usize > WASIP1_STREAM_CHUNK_CAPACITY
        || borrow.epoch() != TEST_MEMORY_EPOCH
    {
        panic!();
    }
    let grant = ledger.grant_read_lease(borrow).unwrap_or_else(|_| panic!());
    match endpoint
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<led grant>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_FD_WRITE_GRANT_SENT);

    let request = match endpoint.recv::<Msg<LABEL_WASI_FD_WRITE, EngineReq>>().await {
        Ok(request) => request,
        Err(_) => {
            record_failure_stage(STAGE_KERNEL_FD_WRITE_REQ_RECV_ERR);
            panic!()
        }
    };
    mark_stage(STAGE_KERNEL_FD_WRITE_REQ_RECV);
    let EngineReq::FdWrite(write) = request else {
        record_failure_stage(STAGE_KERNEL_FD_WRITE_REQ_MISMATCH);
        panic!();
    };
    if ledger.validate_fd_write_lease(&write, grant).is_err() {
        record_failure_stage(STAGE_KERNEL_FD_WRITE_LEASE_ERR);
        panic!();
    }
    let (written, errno) =
        match check_gpio_object_fd_write(ledger.fd_view(), &write, baker_link_led_fd_write_route())
        {
            Ok(set) => {
                kernel_send_gpio_set(endpoint, gpio_endpoint, set).await;
                mark_stage(STAGE_KERNEL_FD_WRITE_GPIO_DONE);
                (write.len() as u8, WASI_ERRNO_SUCCESS)
            }
            Err(error) => {
                let _ = gpio_endpoint;
                let errno = status::gpio_fd_write_errno(error);
                if status::mark_expected_reject_if_recorded() {
                    park();
                }
                (0, errno)
            }
        };

    let reply = EngineRet::FdWriteDone(FdWriteDone::new_with_errno(write.fd(), written, errno));
    match endpoint
        .flow::<Msg<LABEL_WASI_FD_WRITE_RET, EngineRet>>()
        .expect("kernel flow<led fd_write ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }

    let release = match endpoint.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>().await {
        Ok(release) => release,
        Err(_) => panic!(),
    };
    if release.lease_id() != grant.lease_id() {
        panic!();
    }
    ledger.release_lease(release).unwrap_or_else(|_| panic!());
    if status::mark_expected_reject_if_recorded() {
        park();
    }
}

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
async fn kernel_path_open(
    endpoint: &mut KernelEndpoint,
    ledger: &mut BakerLedger,
    store: &BakerLinkLedResourceStore,
    borrow: MemBorrow,
) {
    mark_stage(STAGE_KERNEL_PATH_OPEN_BORROW_RECV);
    if borrow.len() == 0 || borrow.epoch() != TEST_MEMORY_EPOCH {
        panic!();
    }
    let grant = ledger.grant_read_lease(borrow).unwrap_or_else(|_| panic!());
    match endpoint
        .flow::<MemReadGrantControl>()
        .expect("kernel flow<path grant>")
        .send(())
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_PATH_OPEN_GRANT_SENT);

    let request = match endpoint
        .recv::<Msg<LABEL_WASI_PATH_OPEN, EngineReq>>()
        .await
    {
        Ok(request) => request,
        Err(_) => panic!(),
    };
    let EngineReq::PathOpen(open) = request else {
        panic!();
    };
    mark_stage(STAGE_KERNEL_PATH_OPEN_REQ_RECV);
    if open.preopen_fd() != BAKER_LINK_CHOREOFS_PREOPEN_FD
        || open.lease_id() != grant.lease_id()
        || open.len() > borrow.len() as usize
    {
        panic!();
    }

    let (opened, errno) =
        match resolve_baker_link_choreofs_path(store, ledger, open.path(), open.rights_base()) {
            Ok(opened) => {
                mark_stage(STAGE_KERNEL_PATH_OPEN_OBJECT_OPENED);
                (Some(opened), WASI_ERRNO_SUCCESS)
            }
            Err(error) => {
                status::record_choreofs_open_reject(error);
                (None, error.wasi_errno())
            }
        };
    let opened_fd = if let Some(opened) = opened {
        match endpoint
            .flow::<ChoreoFsOpenAdmitRouteMsg>()
            .expect("kernel flow<choreofs open admit route>")
            .send(&ChoreoFsOpenAdmitRoute)
            .await
        {
            Ok(_) => {}
            Err(_) => panic!(),
        }
        match mint_baker_link_choreofs_fd(ledger, opened) {
            Ok(fd) => fd.fd(),
            Err(_) => panic!(),
        }
    } else {
        match endpoint
            .flow::<ChoreoFsOpenRejectRouteMsg>()
            .expect("kernel flow<choreofs open reject route>")
            .send(&ChoreoFsOpenRejectRoute)
            .await
        {
            Ok(_) => {}
            Err(_) => panic!(),
        }
        0
    };
    let rejected = errno != WASI_ERRNO_SUCCESS;
    #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
    let _ = rejected;
    let reply = EngineRet::PathOpened(PathOpened::new(opened_fd, errno));
    match endpoint
        .flow::<Msg<LABEL_WASI_PATH_OPEN_RET, EngineRet>>()
        .expect("kernel flow<path_open ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_PATH_OPEN_RET_SENT);

    let release = match endpoint.recv::<Msg<LABEL_MEM_RELEASE, MemRelease>>().await {
        Ok(release) => release,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_KERNEL_PATH_OPEN_RELEASE_RECV);
    if release.lease_id() != grant.lease_id() {
        panic!();
    }
    ledger.release_lease(release).unwrap_or_else(|_| panic!());
    #[cfg(feature = "baker-choreofs-bad-path-demo")]
    if rejected {
        record_failure_stage(STAGE_BAD_PATH_REJECTED);
        mark_stage(RESULT_EXPECTED_REJECT);
        park();
    }
    if status::mark_expected_reject_if_recorded() {
        park();
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_poll_oneoff(
    endpoint: &mut KernelEndpoint,
    timer_endpoint: &mut TimerEndpoint,
    ledger: &mut BakerLedger,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    last_tick: &mut u64,
) {
    let request = match endpoint
        .recv::<Msg<LABEL_WASI_POLL_ONEOFF, EngineReq>>()
        .await
    {
        Ok(request) => request,
        Err(_) => panic!(),
    };
    mark_stage(STAGE_KERNEL_POLL_RECV);
    let EngineReq::PollOneoff(poll) = request else {
        panic!();
    };
    if poll.timeout_tick() < *last_tick {
        panic!();
    }
    let pending_poll = ledger.begin_poll_oneoff(poll).unwrap_or_else(|_| panic!());
    let delta = poll.timeout_tick() - *last_tick;
    if delta > u32::MAX as u64 {
        panic!();
    }
    let delay_ticks = delta as u32;
    *last_tick = poll.timeout_tick();

    let sleep = TimerSleepUntil::new(poll.timeout_tick());
    match endpoint
        .flow::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .expect("kernel flow<timer sleep>")
        .send(&sleep)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_KERNEL_TIMER_SLEEP_SENT);
    timer_device_recv_sleep_once(timer_endpoint, resolver, delay_ticks).await;
    let done = match endpoint
        .recv::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>()
        .await
    {
        Ok(done) => done,
        Err(_) => panic!(),
    };
    if done.tick() != poll.timeout_tick() {
        panic!();
    }
    ledger
        .complete_poll_oneoff(pending_poll, done)
        .unwrap_or_else(|_| panic!());

    let reply = EngineRet::PollReady(PollReady::new(1));
    match endpoint
        .flow::<Msg<LABEL_WASI_POLL_ONEOFF_RET, EngineRet>>()
        .expect("kernel flow<led poll_oneoff ret>")
        .send(&reply)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_recv_proc_exit(endpoint: &mut KernelEndpoint) {
    let branch = endpoint.offer().await.unwrap_or_else(|_| panic!());
    if branch.label() != LABEL_WASI_PROC_EXIT {
        panic!();
    }
    let request = branch
        .decode::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
        .await
        .unwrap_or_else(|_| panic!());
    let EngineReq::ProcExit(status) = request else {
        panic!();
    };
    if status.code() != 0 {
        panic!();
    }
    mark_stage(STAGE_KERNEL_PROC_EXIT_RECV);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_process_traffic_fd_write(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    ledger: &mut BakerLedger,
) {
    let branch = endpoint.offer().await.unwrap_or_else(|_| {
        record_failure_stage(STAGE_KERNEL_TRAFFIC_OFFER_ERR);
        panic!()
    });
    match branch.label() {
        LABEL_MEM_BORROW_READ => {
            let borrow = {
                let decoded = branch
                    .decode::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
                    .await
                    .unwrap_or_else(|_| {
                        record_failure_stage(STAGE_KERNEL_TRAFFIC_MEM_RECV_ERR);
                        panic!()
                    });
                MemBorrow::new(decoded.ptr(), decoded.len(), decoded.epoch())
            };
            if borrow.ptr() != TEST_LED_PTR
                || borrow.len() == 0
                || borrow.len() as usize > WASIP1_STREAM_CHUNK_CAPACITY
                || borrow.epoch() != TEST_MEMORY_EPOCH
            {
                record_failure_stage(STAGE_KERNEL_TRAFFIC_MEM_MISMATCH);
                panic!();
            }
            kernel_fd_write(endpoint, gpio_endpoint, ledger, borrow).await;
        }
        _ => {
            record_failure_stage(STAGE_KERNEL_TRAFFIC_OFFER_ERR);
            panic!()
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
pub(super) async fn kernel_session(
    endpoint: &mut KernelEndpoint,
    gpio_endpoint: &mut GpioEndpoint,
    timer_endpoint: &mut TimerEndpoint,
) {
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    let store = baker_link_led_resource_store().unwrap_or_else(|_| panic!());
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    let mut ledger =
        baker_link_choreofs_ledger::<4, 1, 1>(&store, TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
            .unwrap_or_else(|_| panic!());
    #[cfg(not(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    )))]
    let mut ledger = hibana_pico::projects::baker_link_led::ledger::baker_link_pico_min_ledger::<
        1,
        1,
    >(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
    .unwrap_or_else(|_| panic!());
    let mut resolver: PicoInterruptResolver<2, 4, 1> = PicoInterruptResolver::new();

    let activation_id = 0u16;
    kernel_start_app_activation(endpoint, activation_id, 0).await;
    #[cfg(any(
        feature = "baker-choreofs-demo",
        feature = "baker-choreofs-bad-path-demo",
        feature = "baker-choreofs-bad-payload-demo",
        feature = "baker-choreofs-wrong-object-demo"
    ))]
    {
        #[cfg(feature = "baker-choreofs-bad-path-demo")]
        let path_open_count = 1usize;
        #[cfg(not(feature = "baker-choreofs-bad-path-demo"))]
        let path_open_count = 3usize;
        for _ in 0..path_open_count {
            let borrow = endpoint
                .recv::<Msg<LABEL_MEM_BORROW_READ, MemBorrow>>()
                .await
                .unwrap_or_else(|_| panic!());
            kernel_path_open(endpoint, &mut ledger, &store, borrow).await;
        }
    }
    let mut tick = 0u64;
    for step in 0..BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS {
        kernel_process_traffic_fd_write(endpoint, gpio_endpoint, &mut ledger).await;
        if step == 0 {
            mark_stage(STAGE_FIRST_LED_WRITE_DONE);
        }
        kernel_poll_oneoff(
            endpoint,
            timer_endpoint,
            &mut ledger,
            &mut resolver,
            &mut tick,
        )
        .await;
        if step == 0 {
            mark_stage(STAGE_POLL_ON_DONE);
        }
        mark_stage(STAGE_FINAL_LED_WRITE_DONE);
    }
    kernel_recv_proc_exit(endpoint).await;
    mark_stage(RESULT_SUCCESS);
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
pub(super) async fn kernel_abort_safe_session(endpoint: &mut KernelEndpoint) {
    #[cfg(feature = "baker-recoverable-abort-demo")]
    kernel_send_abort_budget_run(endpoint, 0).await;

    let mut ledger = hibana_pico::projects::baker_link_led::ledger::baker_link_pico_min_ledger::<
        1,
        1,
    >(TEST_MEMORY_LEN, TEST_MEMORY_EPOCH)
    .unwrap_or_else(|_| panic!());
    let grant = ledger
        .grant_read_lease(MemBorrow::new(TEST_LED_PTR, 1, TEST_MEMORY_EPOCH))
        .unwrap_or_else(|_| panic!());
    let pending = ledger
        .begin_poll_oneoff(PollOneoff::new(50))
        .unwrap_or_else(|_| panic!());
    if ledger.fd_view().active_count() != 3
        || ledger.lease_table().outstanding_lease_count() != 1
        || ledger.pending_table().pending_count() != 1
    {
        panic!();
    }

    let branch = endpoint.offer().await.unwrap_or_else(|_| panic!());
    if branch.label() != LABEL_ENGINE_ABORT_REASON {
        panic!();
    }
    let abort = branch
        .decode::<EngineAbortMsg>()
        .await
        .unwrap_or_else(|_| panic!());
    if abort.reason() != EngineAbortReason::GuestTrap || abort.code() != 1 {
        panic!();
    }

    endpoint
        .recv::<EngineAbortBeginControl>()
        .await
        .unwrap_or_else(|_| panic!());

    ledger.apply_abort_fence(TEST_MEMORY_EPOCH + 1);
    if ledger.fd_view().active_count() != 0
        || ledger.lease_table().outstanding_lease_count() != 0
        || ledger.pending_table().pending_count() != 0
        || ledger
            .release_lease(MemRelease::new(grant.lease_id()))
            .is_ok()
        || ledger
            .complete_poll_oneoff(pending, TimerSleepDone::new(50))
            .is_ok()
    {
        panic!();
    }
    mark_stage(STAGE_KERNEL_ABORT_FENCE_APPLIED);

    endpoint
        .flow::<EngineAbortFenceControl>()
        .expect("kernel flow<abort fence>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());
    mark_stage(STAGE_KERNEL_ABORT_FENCE_SENT);

    for safe in BAKER_LINK_SAFE_GPIO_LEVELS {
        mark_stage(STAGE_KERNEL_ABORT_SAFE_GPIO_BEGIN);
        kernel_send_gpio_set_remote(endpoint, GpioSet::new(safe.pin(), safe.high())).await;
    }
    mark_stage(STAGE_KERNEL_ABORT_SAFE_GPIO_DONE);

    endpoint
        .flow::<EngineAbortAckControl>()
        .expect("kernel flow<abort ack>")
        .send(())
        .await
        .unwrap_or_else(|_| panic!());

    #[cfg(feature = "baker-recoverable-abort-demo")]
    {
        kernel_send_abort_budget_run(endpoint, 1).await;
        mark_stage(STAGE_KERNEL_ABORT_REENTER_RUN_SENT);
        let request = endpoint
            .recv::<Msg<LABEL_WASI_PROC_EXIT, EngineReq>>()
            .await
            .unwrap_or_else(|_| panic!());
        let EngineReq::ProcExit(status) = request else {
            panic!();
        };
        if status.code() != 0 {
            panic!();
        }
        mark_stage(STAGE_KERNEL_ABORT_REENTER_PROC_EXIT);
        mark_stage(RESULT_RECOVERABLE_ABORT_OK);
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-recoverable-abort-demo"
))]
async fn kernel_send_abort_budget_run(endpoint: &mut KernelEndpoint, activation_id: u16) {
    let run = BudgetRun::new(
        activation_id,
        activation_id.saturating_add(1),
        BAKER_LINK_WASM_FUEL_PER_ACTIVATION,
        0,
    );
    endpoint
        .flow::<BudgetRunMsg>()
        .expect("kernel flow<recoverable budget run>")
        .send(&run)
        .await
        .unwrap_or_else(|_| panic!());
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn kernel_start_app_activation(endpoint: &mut KernelEndpoint, activation_id: u16, tick: u64) {
    mark_stage(STAGE_KERNEL_RUN_SEND_BEGIN);
    let run = BudgetRun::new(activation_id, 1, BAKER_LINK_WASM_FUEL_PER_ACTIVATION, tick);
    let flow = endpoint.flow::<BudgetRunMsg>().unwrap_or_else(|_| {
        record_failure_stage(STAGE_KERNEL_RUN_FLOW_ERR);
        panic!()
    });
    match flow.send(&run).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_KERNEL_RUN_SEND_ERR);
            panic!()
        }
    }
    mark_stage(STAGE_KERNEL_RUN_SEND_DONE);
}
