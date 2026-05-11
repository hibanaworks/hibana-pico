//! Baker Link Dev Rev1 LED proof choreography.
//!
//! This project-local module is intentionally separate from
//! `choreography::local`: Baker Link is the public RP2040 hardware proof, not
//! the generic syscall/device protocol vocabulary. The roles are:
//!
//! - Role 0: Kernel
//! - Role 1: Engine / WASI P1 app
//! - Role 2: GPIO device
//! - Role 3: Timer device

use hibana::{
    g,
    g::{Msg, Role},
    substrate::{
        cap::{
            GenericCapToken,
            advanced::{LoopBreakKind, LoopContinueKind},
        },
        program::{RoleProgram, project},
    },
};

use crate::choreography::protocol::{
    BudgetRunMsg, EngineAbortAckControl, EngineAbortBeginControl, EngineAbortFenceControl,
    EngineAbortMsg, EngineAbortRouteControl, EngineNormalRouteControl, EngineReq,
    LABEL_WASI_PROC_EXIT, local_fd_write_gpio_cycle, local_gpio_set_cycle, local_path_open_cycle,
    local_path_open_reject_cycle, local_poll_timer_cycle, seq_chain,
};

pub const LABEL_BAKER_TRAFFIC_LOOP_CONTINUE: u8 = 120;
pub const LABEL_BAKER_TRAFFIC_LOOP_BREAK: u8 = 121;
pub const POLICY_BAKER_TRAFFIC_LOOP: u16 = 120;
pub const POLICY_BAKER_ENGINE_ABORT_ROUTE: u16 = 121;

pub type BakerTrafficLoopContinueControl =
    Msg<LABEL_BAKER_TRAFFIC_LOOP_CONTINUE, GenericCapToken<LoopContinueKind>, LoopContinueKind>;
pub type BakerTrafficLoopBreakControl =
    Msg<LABEL_BAKER_TRAFFIC_LOOP_BREAK, GenericCapToken<LoopBreakKind>, LoopBreakKind>;
macro_rules! fd_write_two_cycles_program {
    () => {
        g::seq(local_fd_write_gpio_cycle!(), local_fd_write_gpio_cycle!())
    };
}

macro_rules! abort_safe_terminal_program {
    () => {{
        let abort_arm = seq_chain!(
            g::send::<Role<1>, Role<1>, EngineAbortRouteControl, 1>()
                .policy::<POLICY_BAKER_ENGINE_ABORT_ROUTE>(),
            g::send::<Role<1>, Role<0>, EngineAbortMsg, 1>(),
            g::send::<Role<1>, Role<0>, EngineAbortBeginControl, 1>(),
            g::send::<Role<0>, Role<1>, EngineAbortFenceControl, 1>(),
            local_gpio_set_cycle!(),
            local_gpio_set_cycle!(),
            local_gpio_set_cycle!(),
            g::send::<Role<0>, Role<1>, EngineAbortAckControl, 1>(),
        );
        let normal_arm = seq_chain!(
            g::send::<Role<1>, Role<1>, EngineNormalRouteControl, 1>()
                .policy::<POLICY_BAKER_ENGINE_ABORT_ROUTE>(),
            g::send::<Role<1>, Role<0>, Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
        );
        g::route(abort_arm, normal_arm)
    }};
}

macro_rules! abort_safe_linear_program {
    () => {
        seq_chain!(
            g::send::<Role<1>, Role<0>, EngineAbortMsg, 1>(),
            g::send::<Role<1>, Role<0>, EngineAbortBeginControl, 1>(),
            g::send::<Role<0>, Role<1>, EngineAbortFenceControl, 1>(),
            local_gpio_set_cycle!(),
            local_gpio_set_cycle!(),
            local_gpio_set_cycle!(),
            g::send::<Role<0>, Role<1>, EngineAbortAckControl, 1>(),
        )
    };
}

macro_rules! recoverable_abort_program {
    () => {
        seq_chain!(
            g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
            abort_safe_linear_program!(),
            g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
            g::send::<Role<1>, Role<0>, Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
        )
    };
}

macro_rules! traffic_light_program {
    () => {{
        let continue_arm = g::send::<Role<1>, Role<1>, BakerTrafficLoopContinueControl, 1>()
            .policy::<POLICY_BAKER_TRAFFIC_LOOP>();
        let break_arm = g::send::<Role<1>, Role<1>, BakerTrafficLoopBreakControl, 1>()
            .policy::<POLICY_BAKER_TRAFFIC_LOOP>();
        g::seq(
            g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
            g::route(
                g::seq(
                    continue_arm,
                    g::seq(local_fd_write_gpio_cycle!(), local_poll_timer_cycle!()),
                ),
                g::seq(
                    break_arm,
                    g::send::<Role<1>, Role<0>, Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
                ),
            ),
        )
    }};
}

macro_rules! choreofs_traffic_light_program {
    () => {{
        let continue_arm = g::send::<Role<1>, Role<1>, BakerTrafficLoopContinueControl, 1>()
            .policy::<POLICY_BAKER_TRAFFIC_LOOP>();
        let break_arm = g::send::<Role<1>, Role<1>, BakerTrafficLoopBreakControl, 1>()
            .policy::<POLICY_BAKER_TRAFFIC_LOOP>();
        seq_chain!(
            g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
            local_path_open_cycle!(),
            local_path_open_cycle!(),
            local_path_open_cycle!(),
            g::route(
                g::seq(
                    continue_arm,
                    g::seq(local_fd_write_gpio_cycle!(), local_poll_timer_cycle!())
                ),
                g::seq(
                    break_arm,
                    g::send::<Role<1>, Role<0>, Msg<LABEL_WASI_PROC_EXIT, EngineReq>, 1>(),
                ),
            ),
        )
    }};
}

macro_rules! choreofs_bad_path_program {
    () => {
        seq_chain!(
            g::send::<Role<0>, Role<1>, BudgetRunMsg, 1>(),
            local_path_open_reject_cycle!(),
        )
    };
}

pub const FD_WRITE_KERNEL_PROGRAM: RoleProgram<0> = project(&fd_write_two_cycles_program!());
pub const FD_WRITE_ENGINE_PROGRAM: RoleProgram<1> = project(&fd_write_two_cycles_program!());
pub const FD_WRITE_GPIO_PROGRAM: RoleProgram<2> = project(&fd_write_two_cycles_program!());

pub const TRAFFIC_LIGHT_KERNEL_PROGRAM: RoleProgram<0> = project(&traffic_light_program!());
pub const TRAFFIC_LIGHT_ENGINE_PROGRAM: RoleProgram<1> = project(&traffic_light_program!());
pub const TRAFFIC_LIGHT_GPIO_PROGRAM: RoleProgram<2> = project(&traffic_light_program!());
pub const TRAFFIC_LIGHT_TIMER_PROGRAM: RoleProgram<3> = project(&traffic_light_program!());

pub const CHOREOFS_TRAFFIC_LIGHT_KERNEL_PROGRAM: RoleProgram<0> =
    project(&choreofs_traffic_light_program!());
pub const CHOREOFS_TRAFFIC_LIGHT_ENGINE_PROGRAM: RoleProgram<1> =
    project(&choreofs_traffic_light_program!());
pub const CHOREOFS_TRAFFIC_LIGHT_GPIO_PROGRAM: RoleProgram<2> =
    project(&choreofs_traffic_light_program!());
pub const CHOREOFS_TRAFFIC_LIGHT_TIMER_PROGRAM: RoleProgram<3> =
    project(&choreofs_traffic_light_program!());
pub const CHOREOFS_BAD_PATH_KERNEL_PROGRAM: RoleProgram<0> = project(&choreofs_bad_path_program!());
pub const CHOREOFS_BAD_PATH_ENGINE_PROGRAM: RoleProgram<1> = project(&choreofs_bad_path_program!());
pub const CHOREOFS_BAD_PATH_GPIO_PROGRAM: RoleProgram<2> = project(&choreofs_bad_path_program!());
pub const CHOREOFS_BAD_PATH_TIMER_PROGRAM: RoleProgram<3> = project(&choreofs_bad_path_program!());
pub const ABORT_SAFE_KERNEL_PROGRAM: RoleProgram<0> = project(&abort_safe_terminal_program!());
pub const ABORT_SAFE_ENGINE_PROGRAM: RoleProgram<1> = project(&abort_safe_terminal_program!());
pub const ABORT_SAFE_GPIO_PROGRAM: RoleProgram<2> = project(&abort_safe_terminal_program!());
pub const ABORT_SAFE_LINEAR_KERNEL_PROGRAM: RoleProgram<0> = project(&abort_safe_linear_program!());
pub const ABORT_SAFE_LINEAR_ENGINE_PROGRAM: RoleProgram<1> = project(&abort_safe_linear_program!());
pub const ABORT_SAFE_LINEAR_GPIO_PROGRAM: RoleProgram<2> = project(&abort_safe_linear_program!());
pub const RECOVERABLE_ABORT_KERNEL_PROGRAM: RoleProgram<0> = project(&recoverable_abort_program!());
pub const RECOVERABLE_ABORT_ENGINE_PROGRAM: RoleProgram<1> = project(&recoverable_abort_program!());
pub const RECOVERABLE_ABORT_GPIO_PROGRAM: RoleProgram<2> = project(&recoverable_abort_program!());

/// Baker Link LED fd_write proof. The guest writes ASCII `1` then `0` to fd 3;
/// each write is gated by a read lease and acknowledged by the GPIO device role
/// before Kernel returns to Engine.
pub fn fd_write_two_cycles_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    (
        FD_WRITE_KERNEL_PROGRAM,
        FD_WRITE_ENGINE_PROGRAM,
        FD_WRITE_GPIO_PROGRAM,
    )
}

/// Baker Link LED traffic-light proof. Kernel sends one `BudgetRunMsg`; after
/// that the Engine-owned hibana loop route decides whether the WASI app has
/// another fd_write/poll body (`LoopContinue`) or has returned (`LoopBreak +
/// proc_exit`). Every wait is a WASI `poll_oneoff` admitted by the Timer
/// resolver.
pub fn traffic_light_roles() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
) {
    (
        TRAFFIC_LIGHT_KERNEL_PROGRAM,
        TRAFFIC_LIGHT_ENGINE_PROGRAM,
        TRAFFIC_LIGHT_GPIO_PROGRAM,
        TRAFFIC_LIGHT_TIMER_PROGRAM,
    )
}

/// Baker Link ChoreoFS LED proof. The guest first opens LED resource paths
/// through WASI `path_open`, so the GPIO fds are minted from ChoreoFS object
/// identities before the same fd_write/poll loop begins.
pub fn choreofs_traffic_light_roles() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
) {
    (
        CHOREOFS_TRAFFIC_LIGHT_KERNEL_PROGRAM,
        CHOREOFS_TRAFFIC_LIGHT_ENGINE_PROGRAM,
        CHOREOFS_TRAFFIC_LIGHT_GPIO_PROGRAM,
        CHOREOFS_TRAFFIC_LIGHT_TIMER_PROGRAM,
    )
}

/// Baker Link ChoreoFS bad-path proof. The only legal protocol body is one
/// `path_open` transaction after `BudgetRun`; the returned errno is the typed
/// terminal result for this negative proof.
pub fn choreofs_bad_path_roles() -> (
    RoleProgram<0>,
    RoleProgram<1>,
    RoleProgram<2>,
    RoleProgram<3>,
) {
    (
        CHOREOFS_BAD_PATH_KERNEL_PROGRAM,
        CHOREOFS_BAD_PATH_ENGINE_PROGRAM,
        CHOREOFS_BAD_PATH_GPIO_PROGRAM,
        CHOREOFS_BAD_PATH_TIMER_PROGRAM,
    )
}

/// Baker Link abort terminal proof. This is the terminal fragment used by
/// Engine-owned abort branches: Engine begins abort, Kernel fences local
/// authority, then the existing GPIO choreography applies the board safe state
/// before Kernel acknowledges the abort.
pub fn abort_safe_terminal_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    (
        ABORT_SAFE_KERNEL_PROGRAM,
        ABORT_SAFE_ENGINE_PROGRAM,
        ABORT_SAFE_GPIO_PROGRAM,
    )
}

/// Baker Link abort terminal fragment without the preceding Abort | Normal
/// route. Hardware uses this after the Engine-owned abort route has been
/// selected by construction; host tests keep the full route proof above.
pub fn abort_safe_linear_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    (
        ABORT_SAFE_LINEAR_KERNEL_PROGRAM,
        ABORT_SAFE_LINEAR_ENGINE_PROGRAM,
        ABORT_SAFE_LINEAR_GPIO_PROGRAM,
    )
}

/// Baker Link recoverable fail-safe proof. The lifecycle owner holds a reusable
/// `ActivationAuthority<Many>` in the lower-layer hibana capability system. This
/// projected proof materializes concrete `Activation<One>` runs as
/// generation-distinct `BudgetRunMsg`s: the first run aborts, `Fence` makes its
/// authority stale, and a second `BudgetRunMsg` starts a fresh activation.
pub fn recoverable_abort_roles() -> (RoleProgram<0>, RoleProgram<1>, RoleProgram<2>) {
    (
        RECOVERABLE_ABORT_KERNEL_PROGRAM,
        RECOVERABLE_ABORT_ENGINE_PROGRAM,
        RECOVERABLE_ABORT_GPIO_PROGRAM,
    )
}
