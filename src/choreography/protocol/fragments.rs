macro_rules! seq_chain {
    ($head:expr, $($tail:expr),+ $(,)?) => {
        ::hibana::g::seq($head, $crate::choreography::protocol::seq_chain!($($tail),+))
    };
    ($last:expr $(,)?) => {
        $last
    };
}

macro_rules! local_fd_write_gpio_cycle {
    () => {
        $crate::choreography::protocol::seq_chain!(
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_BORROW_READ },
                    $crate::choreography::protocol::MemBorrow,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                $crate::choreography::protocol::MemReadGrantControl,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_FD_WRITE },
                    $crate::choreography::protocol::EngineReq,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<2>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_GPIO_SET },
                    $crate::choreography::protocol::GpioSet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<2>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_GPIO_SET_DONE },
                    $crate::choreography::protocol::GpioSet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_FD_WRITE_RET },
                    $crate::choreography::protocol::EngineRet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_RELEASE },
                    $crate::choreography::protocol::MemRelease,
                >,
                1,
            >(),
        )
    };
}

macro_rules! local_path_open_cycle {
    () => {
        $crate::choreography::protocol::seq_chain!(
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_BORROW_READ },
                    $crate::choreography::protocol::MemBorrow,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                $crate::choreography::protocol::MemReadGrantControl,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_PATH_OPEN },
                    $crate::choreography::protocol::EngineReq,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                $crate::choreography::protocol::ChoreoFsOpenAdmitRouteMsg,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_PATH_OPEN_RET },
                    $crate::choreography::protocol::EngineRet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_RELEASE },
                    $crate::choreography::protocol::MemRelease,
                >,
                1,
            >(),
        )
    };
}

macro_rules! local_path_open_reject_cycle {
    () => {
        $crate::choreography::protocol::seq_chain!(
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_BORROW_READ },
                    $crate::choreography::protocol::MemBorrow,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                $crate::choreography::protocol::MemReadGrantControl,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_PATH_OPEN },
                    $crate::choreography::protocol::EngineReq,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                $crate::choreography::protocol::ChoreoFsOpenRejectRouteMsg,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_PATH_OPEN_RET },
                    $crate::choreography::protocol::EngineRet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_MEM_RELEASE },
                    $crate::choreography::protocol::MemRelease,
                >,
                1,
            >(),
        )
    };
}

macro_rules! local_poll_timer_cycle {
    () => {
        $crate::choreography::protocol::seq_chain!(
            ::hibana::g::send::<
                ::hibana::g::Role<1>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_POLL_ONEOFF },
                    $crate::choreography::protocol::EngineReq,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<3>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_TIMER_SLEEP_UNTIL },
                    $crate::choreography::protocol::TimerSleepUntil,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<3>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_TIMER_SLEEP_DONE },
                    $crate::choreography::protocol::TimerSleepDone,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<1>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_WASI_POLL_ONEOFF_RET },
                    $crate::choreography::protocol::EngineRet,
                >,
                1,
            >(),
        )
    };
}

macro_rules! local_gpio_set_cycle {
    () => {
        $crate::choreography::protocol::seq_chain!(
            ::hibana::g::send::<
                ::hibana::g::Role<0>,
                ::hibana::g::Role<2>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_GPIO_SET },
                    $crate::choreography::protocol::GpioSet,
                >,
                1,
            >(),
            ::hibana::g::send::<
                ::hibana::g::Role<2>,
                ::hibana::g::Role<0>,
                ::hibana::g::Msg<
                    { $crate::choreography::protocol::LABEL_GPIO_SET_DONE },
                    $crate::choreography::protocol::GpioSet,
                >,
                1,
            >(),
        )
    };
}

pub(crate) use local_fd_write_gpio_cycle;
pub(crate) use local_gpio_set_cycle;
pub(crate) use local_path_open_cycle;
pub(crate) use local_path_open_reject_cycle;
pub(crate) use local_poll_timer_cycle;
pub(crate) use seq_chain;
