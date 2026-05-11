#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::hardware::*;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::stages::*;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::status::mark_stage;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::status::record_failure_stage;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use super::storage::GpioEndpoint;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use super::storage::TimerEndpoint;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana::g::Msg;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use hibana_pico::choreography::protocol::{GpioSet, LABEL_GPIO_SET, LABEL_GPIO_SET_DONE};
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
use hibana_pico::{
    choreography::protocol::{
        LABEL_TIMER_SLEEP_DONE, LABEL_TIMER_SLEEP_UNTIL, TimerSleepDone, TimerSleepUntil,
    },
    kernel::resolver::{InterruptEvent, PicoInterruptResolver, ResolvedInterrupt},
    machine::rp2040::timer,
    port::exec::wait_until,
};

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
async fn gpio_device_recv_set_payload_once(endpoint: &mut GpioEndpoint) -> GpioSet {
    let branch = endpoint.offer().await.unwrap_or_else(|_| {
        record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
        panic!()
    });
    if branch.label() != LABEL_GPIO_SET {
        record_failure_stage(STAGE_GPIO_SET_LABEL_ERR);
        panic!();
    }
    let set = match branch.decode::<Msg<LABEL_GPIO_SET, GpioSet>>().await {
        Ok(set) => set,
        Err(_) => {
            record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
            panic!()
        }
    };
    rp2040_gpio_apply_baker_led_set(set);
    set
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn gpio_device_send_set_done_once(endpoint: &mut GpioEndpoint, set: GpioSet) {
    let flow = endpoint
        .flow::<Msg<LABEL_GPIO_SET_DONE, GpioSet>>()
        .unwrap_or_else(|_| {
            record_failure_stage(STAGE_GPIO_SET_DONE_SEND_ERR);
            panic!()
        });
    match flow.send(&set).await {
        Ok(_) => {}
        Err(_) => {
            record_failure_stage(STAGE_GPIO_SET_DONE_SEND_ERR);
            panic!()
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
pub(super) async fn gpio_device_recv_set_once(endpoint: &mut GpioEndpoint) {
    let set = gpio_device_recv_set_payload_once(endpoint).await;
    gpio_device_send_set_done_once(endpoint, set).await;
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "baker-abort-safe-demo"
))]
pub(super) async fn gpio_device_recv_abort_set_once(endpoint: &mut GpioEndpoint, route_depth: u8) {
    let set = if route_depth == 0 {
        endpoint
            .recv::<Msg<LABEL_GPIO_SET, GpioSet>>()
            .await
            .unwrap_or_else(|_| {
                record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
                panic!()
            })
    } else {
        let branch = endpoint.offer().await.unwrap_or_else(|_| {
            record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
            panic!()
        });
        if branch.label() != LABEL_GPIO_SET {
            record_failure_stage(STAGE_GPIO_SET_LABEL_ERR);
            panic!();
        }
        branch
            .decode::<Msg<LABEL_GPIO_SET, GpioSet>>()
            .await
            .unwrap_or_else(|_| {
                record_failure_stage(STAGE_GPIO_SET_DECODE_ERR);
                panic!()
            })
    };
    rp2040_gpio_apply_baker_led_set(set);
    gpio_device_send_set_done_once(endpoint, set).await;
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    not(feature = "baker-abort-safe-demo")
))]
pub(super) async fn timer_device_recv_sleep_once(
    endpoint: &mut TimerEndpoint,
    resolver: &mut PicoInterruptResolver<2, 4, 1>,
    delay_ticks: u32,
) {
    let branch = match endpoint.offer().await {
        Ok(branch) => branch,
        Err(_) => {
            record_failure_stage(STAGE_TIMER_SLEEP_RECV);
            panic!()
        }
    };
    if branch.label() != LABEL_TIMER_SLEEP_UNTIL {
        record_failure_stage(STAGE_TIMER_SLEEP_RECV);
        panic!();
    }
    let sleep = match branch
        .decode::<Msg<LABEL_TIMER_SLEEP_UNTIL, TimerSleepUntil>>()
        .await
    {
        Ok(sleep) => sleep,
        Err(_) => {
            record_failure_stage(STAGE_TIMER_SLEEP_RECV);
            panic!()
        }
    };
    mark_stage(STAGE_TIMER_SLEEP_RECV);

    resolver
        .request_timer_sleep(sleep)
        .unwrap_or_else(|_| panic!());
    resolver
        .push_irq(InterruptEvent::TimerTick {
            tick: sleep.tick().saturating_sub(1),
        })
        .unwrap_or_else(|_| panic!());
    if resolver
        .resolve_next()
        .unwrap_or_else(|_| panic!())
        .is_some()
    {
        panic!();
    }

    timer::arm_alarm0_after_ticks(delay_ticks);
    mark_stage(STAGE_TIMER_ALARM_ARMED);
    wait_until(timer::alarm0_ready);
    mark_stage(STAGE_TIMER_RAW_READY);
    let Some(_ready) = timer::take_alarm0_ready() else {
        panic!();
    };
    resolver
        .push_irq(InterruptEvent::TimerTick { tick: sleep.tick() })
        .unwrap_or_else(|_| panic!());
    let Some(ResolvedInterrupt::TimerSleepDone(done)) =
        resolver.resolve_next().unwrap_or_else(|_| panic!())
    else {
        panic!();
    };
    if done.tick() != sleep.tick() {
        panic!();
    }

    match endpoint
        .flow::<Msg<LABEL_TIMER_SLEEP_DONE, TimerSleepDone>>()
        .expect("timer flow<sleep done>")
        .send(&done)
        .await
    {
        Ok(_) => {}
        Err(_) => panic!(),
    }
    mark_stage(STAGE_TIMER_DONE_SENT);
}
