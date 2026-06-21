#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use hibana::{
    g,
    runtime::resolver::{DecisionArm, ResolverError, ResolverRef},
};
use hibana_pico::appkit;
use rp2w_firmware::{Rp2wCapsuleFacts, Rp2wPlacement};

const LABEL_RESPONSE_MESSAGE: u8 = 138;
const LABEL_TIMER_EXPIRED_MESSAGE: u8 = 139;
const LABEL_TIMER_ROUTE_DONE: u8 = 140;
const LABEL_TIMER_ROUTE_ACK: u8 = 141;
const LABEL_EPF_POLICY_IMAGE: u8 = 142;
const LABEL_EPF_NO_IMAGE: u8 = 144;
const EPF_TIMER_ROUTE_POLICY: u16 = 57;
const EPF_IMAGE_LOAD_POLICY: u16 = 58;
const EPF_POLICY_IMAGE_BYTES: usize = 64;
const RESULT_EPF_POLICY_TIMER_OK: u32 = 0x4849_4550;
const ENDPOINT_ERROR_CODE: u32 = 0x5745_0f00;
static EPF_RESOLVER_STATE: () = ();

type ResponseReady = g::Msg<LABEL_RESPONSE_MESSAGE, u8>;
type TimerExpired = g::Msg<LABEL_TIMER_EXPIRED_MESSAGE, u8>;
type TimerRouteDone = g::Msg<LABEL_TIMER_ROUTE_DONE, u8>;
type TimerRouteAck = g::Msg<LABEL_TIMER_ROUTE_ACK, u8>;
type EpfPolicyImage = g::Msg<LABEL_EPF_POLICY_IMAGE, [u8; EPF_POLICY_IMAGE_BYTES]>;
type EpfNoImageNotice = g::Msg<LABEL_EPF_NO_IMAGE, u8>;

struct Rp2wEpfPolicyTimer;
struct Rp2wEpfPolicyTimerLocal;

#[derive(Clone, Copy, Debug)]
enum Rp2wEpfPolicyTimerError {
    Endpoint(hibana::EndpointError),
    Resolver(hibana::runtime::resolver::ResolverError),
    RuntimeViolation,
}

impl From<hibana::EndpointError> for Rp2wEpfPolicyTimerError {
    fn from(error: hibana::EndpointError) -> Self {
        rp2w_firmware::record_choreofs_engine_status(0x4550_e000 | ENDPOINT_ERROR_CODE);
        Self::Endpoint(error)
    }
}

impl From<hibana::runtime::resolver::ResolverError> for Rp2wEpfPolicyTimerError {
    fn from(error: hibana::runtime::resolver::ResolverError) -> Self {
        rp2w_firmware::record_choreofs_engine_status(0x4550_f000);
        Self::Resolver(error)
    }
}

fn timer_irq_fact_resolver(_: &()) -> Result<DecisionArm, ResolverError> {
    if rp2w_firmware::rp2w_timer_route_irq_observed() {
        Ok(DecisionArm::Right)
    } else {
        Ok(DecisionArm::Left)
    }
}

fn epf_image_ready_resolver(_: &()) -> Result<DecisionArm, ResolverError> {
    if rp2w_firmware::rp2w_epf_policy_image_ready() {
        Ok(DecisionArm::Left)
    } else {
        Ok(DecisionArm::Right)
    }
}

fn timer_epf_policy_resolver(_: &()) -> Result<DecisionArm, ResolverError> {
    let _ = rp2w_firmware::run_epf_timer_irq_fact();
    rp2w_firmware::epf_policy_resolver::<EPF_TIMER_ROUTE_POLICY>(ResolverRef::decision_state(
        &EPF_RESOLVER_STATE,
        timer_irq_fact_resolver,
    ))
    .decide()
}

fn try_load_uart_policy_image(policy_image: &mut [u8; EPF_POLICY_IMAGE_BYTES]) -> bool {
    if !rp2w_firmware::rp2w_wait_epf_uart_image_ready(1) {
        return false;
    }
    if !rp2w_firmware::read_epf_uart_image(policy_image) {
        return false;
    }
    if !rp2w_firmware::load_epf_uart_choreography_image(policy_image) {
        return false;
    }
    rp2w_firmware::rp2w_epf_uart_clear_image_ready();
    true
}

impl appkit::Capsule for Rp2wEpfPolicyTimer {
    type Placement = Rp2wPlacement;
    type Local = Rp2wEpfPolicyTimerLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::seq(
            g::route(
                g::send::<0, 1, EpfPolicyImage>(),
                g::send::<0, 1, EpfNoImageNotice>(),
            )
            .resolve::<EPF_IMAGE_LOAD_POLICY>(),
            g::seq(
                g::route(
                    g::send::<1, 0, ResponseReady>(),
                    g::send::<1, 0, TimerExpired>(),
                )
                .resolve::<EPF_TIMER_ROUTE_POLICY>(),
                g::seq(
                    g::send::<0, 1, TimerRouteDone>(),
                    g::send::<1, 0, TimerRouteAck>(),
                ),
            ),
        )
    }

    fn register_resolvers<'cfg, R>(registry: &mut R)
    where
        R: appkit::ResolverRegistry<'cfg, Self>,
    {
        rp2w_firmware::record_choreofs_engine_status(0x4550_0200);
        let resolver = ResolverRef::decision_state(&EPF_RESOLVER_STATE, timer_epf_policy_resolver);
        let image_resolver =
            ResolverRef::decision_state(&EPF_RESOLVER_STATE, epf_image_ready_resolver);
        registry.resolver::<EPF_IMAGE_LOAD_POLICY, 0>(image_resolver);
        registry.resolver::<EPF_IMAGE_LOAD_POLICY, 1>(image_resolver);
        registry.resolver::<EPF_TIMER_ROUTE_POLICY, 0>(resolver);
        registry.resolver::<EPF_TIMER_ROUTE_POLICY, 1>(resolver);
        rp2w_firmware::record_choreofs_engine_status(0x4550_0201);
    }

    fn observe(tap: &mut hibana::runtime::tap::TapPort<'_>) {
        rp2w_firmware::poll_epf_diagnostic(tap);
    }
}

impl Rp2wCapsuleFacts for Rp2wEpfPolicyTimer {
    const SUCCESS_RESULT: u32 = RESULT_EPF_POLICY_TIMER_OK;

    fn run_engine_image() {
        rp2w_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<Rp2wEpfPolicyTimer> for Rp2wEpfPolicyTimerLocal {
    type Error = Rp2wEpfPolicyTimerError;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        mut ctx: appkit::EngineCtx<'endpoint, 'guest, Rp2wEpfPolicyTimer, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                rp2w_firmware::record_choreofs_engine_status(0x4550_010f);
                let branch = ctx.endpoint().offer().await?;
                if branch.label() != LABEL_EPF_POLICY_IMAGE {
                    let _ = branch.recv::<EpfNoImageNotice>().await?;
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                let image = branch.recv::<EpfPolicyImage>().await?;
                if !rp2w_firmware::load_epf_choreography_image(&image) {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_engine_status(0x4550_010e);
                while !rp2w_firmware::epf_policy_loaded(EPF_TIMER_ROUTE_POLICY) {
                    rp2w_firmware::poll_epf_image_load();
                    rp2w_firmware::rp2w_poll_delay(1);
                }
                rp2w_firmware::record_choreofs_engine_status(0x4550_0110);

                while !rp2w_firmware::rp2w_timer_route_resolver_ready(500) {
                    rp2w_firmware::rp2w_poll_delay(1);
                }
                if !rp2w_firmware::rp2w_timer_route_irq_observed() {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                if !rp2w_firmware::run_epf_timer_irq_fact() {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_engine_status(0x4550_0111);

                ctx.endpoint().send::<ResponseReady>(&7).await?;
                rp2w_firmware::record_choreofs_engine_status(0x4550_0112);

                let done = ctx.endpoint().recv::<TimerRouteDone>().await?;
                if done != 7 {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_engine_status(0x4550_0113);

                ctx.endpoint().send::<TimerRouteAck>(&7).await?;
                rp2w_firmware::record_choreofs_engine_status(0x4550_0114);

                rp2w_firmware::mark_runtime_ready();
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn driver<'a, const ROLE: u8>(
        mut ctx: appkit::DriverCtx<'a, Rp2wEpfPolicyTimer, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 0 {
                rp2w_firmware::record_choreofs_driver_trace(0x4550_000f);
                rp2w_firmware::rp2w_epf_uart0_init();
                let mut policy_image = [0u8; EPF_POLICY_IMAGE_BYTES];
                let mut policy_image_loaded = false;
                let mut waits = 0usize;
                while !rp2w_firmware::rp2w_epf_policy_image_ready() {
                    if try_load_uart_policy_image(&mut policy_image) {
                        policy_image_loaded = true;
                        break;
                    }
                    if waits >= 5_000 {
                        ctx.endpoint().send::<EpfNoImageNotice>(&0).await?;
                        return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                    }
                    rp2w_firmware::rp2w_poll_delay(1);
                    waits += 1;
                }

                if !policy_image_loaded && !rp2w_firmware::read_epf_policy_image(&mut policy_image)
                {
                    ctx.endpoint().send::<EpfNoImageNotice>(&0).await?;
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }

                ctx.endpoint().send::<EpfPolicyImage>(&policy_image).await?;
                if !policy_image_loaded
                    && !rp2w_firmware::load_epf_choreography_image(&policy_image)
                {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_driver_trace(0x4550_0010);
                if !rp2w_firmware::rp2w_wait_timer_route_irq_observed(500) {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                if !rp2w_firmware::run_epf_timer_irq_fact() {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_driver_trace(0x4550_0011);

                let branch = ctx.endpoint().offer().await?;
                rp2w_firmware::record_choreofs_driver_trace(
                    0x4550_1000 | u32::from(branch.label()),
                );
                let response = branch.recv::<ResponseReady>().await?;
                if response != 7 {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_driver_trace(0x4550_0012);

                ctx.endpoint().send::<TimerRouteDone>(&7).await?;
                rp2w_firmware::record_choreofs_driver_trace(0x4550_0013);

                let ack = ctx.endpoint().recv::<TimerRouteAck>().await?;
                if ack != 7 {
                    return Err(Rp2wEpfPolicyTimerError::RuntimeViolation);
                }
                rp2w_firmware::record_choreofs_driver_trace(0x4550_0014);

                rp2w_firmware::mark_runtime_ready();
                rp2w_firmware::mark_success(
                    <Rp2wEpfPolicyTimer as Rp2wCapsuleFacts>::SUCCESS_RESULT,
                );
                return ctx.pending().await;
            }
            ctx.pending().await
        }
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, Rp2wEpfPolicyTimer, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    rp2w_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn rp2w_selected_run() -> ! {
    rp2w_firmware::run::<Rp2wEpfPolicyTimer>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    rp2w_firmware::run::<Rp2wEpfPolicyTimer>()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    rp2w_firmware::run::<Rp2wEpfPolicyTimer>()
}
