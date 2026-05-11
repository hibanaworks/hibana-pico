use crate::{
    choreography::protocol::{FdWrite, GpioSet},
    kernel::wasi::{ChoreoResourceKind, PicoFdError, PicoFdRights, PicoFdRoute, PicoFdView},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpioFdWriteError {
    Fd(PicoFdError),
    BadFd,
    BadPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpioFdWriteRoute {
    fds: &'static [u8],
    pins: &'static [u8],
    active_high: bool,
    route: PicoFdRoute,
}

impl GpioFdWriteRoute {
    pub const fn new(
        fds: &'static [u8],
        pins: &'static [u8],
        active_high: bool,
        route: PicoFdRoute,
    ) -> Self {
        Self {
            fds,
            pins,
            active_high,
            route,
        }
    }

    pub const fn route(self) -> PicoFdRoute {
        self.route
    }

    pub const fn active_high(self) -> bool {
        self.active_high
    }

    pub fn pin_for_fd(self, fd: u8) -> Option<u8> {
        let mut idx = 0usize;
        while idx < self.fds.len() && idx < self.pins.len() {
            if self.fds[idx] == fd {
                return Some(self.pins[idx]);
            }
            idx += 1;
        }
        None
    }
}

pub fn check_gpio_object_fd_write<const N: usize>(
    table: &PicoFdView<N>,
    write: &FdWrite,
    route: GpioFdWriteRoute,
) -> Result<GpioSet, GpioFdWriteError> {
    let pin = route
        .pin_for_fd(write.fd())
        .ok_or(GpioFdWriteError::BadFd)?;

    table
        .resolve_routed_current(
            write.fd(),
            PicoFdRights::Write,
            ChoreoResourceKind::Gpio,
            route.route(),
        )
        .map_err(GpioFdWriteError::Fd)?;

    let high = match write.as_bytes() {
        b"0" => !route.active_high(),
        b"1" => route.active_high(),
        _ => return Err(GpioFdWriteError::BadPayload),
    };
    Ok(GpioSet::new(pin, high))
}

#[cfg(test)]
mod tests {
    use super::{GpioFdWriteError, GpioFdWriteRoute, check_gpio_object_fd_write};
    use crate::{
        choreography::protocol::{FdWrite, LABEL_GPIO_SET},
        kernel::wasi::{ChoreoResourceKind, PicoFdError, PicoFdRights, PicoFdRoute, PicoFdView},
    };

    const TEST_FDS: [u8; 2] = [3, 4];
    const TEST_PINS: [u8; 2] = [22, 21];
    const TEST_ROUTE: PicoFdRoute = PicoFdRoute::new(0, 0, 3, LABEL_GPIO_SET, 0, 0);

    fn route() -> GpioFdWriteRoute {
        GpioFdWriteRoute::new(&TEST_FDS, &TEST_PINS, true, TEST_ROUTE)
    }

    fn table() -> PicoFdView<2> {
        let mut table = PicoFdView::new();
        for fd in TEST_FDS {
            table
                .apply_cap_grant(
                    fd,
                    PicoFdRights::Write,
                    ChoreoResourceKind::Gpio,
                    0,
                    TEST_ROUTE,
                )
                .expect("grant gpio fd");
        }
        table
    }

    #[test]
    fn resolves_digit_payload_to_gpio_set() {
        let table = table();
        let set =
            check_gpio_object_fd_write(&table, &FdWrite::new(4, b"1").expect("fd_write"), route())
                .expect("check gpio object fd");

        assert_eq!(set.pin(), 21);
        assert!(set.high());
    }

    #[test]
    fn rejects_fd_outside_route_map() {
        let table = table();

        assert_eq!(
            check_gpio_object_fd_write(&table, &FdWrite::new(5, b"1").expect("fd_write"), route()),
            Err(GpioFdWriteError::BadFd)
        );
    }

    #[test]
    fn rejects_wrong_control_route() {
        let mut table: PicoFdView<1> = PicoFdView::new();
        table
            .apply_local_cap_grant(3, PicoFdRights::Write, ChoreoResourceKind::Gpio, 1, 0, 0)
            .expect("grant unrouted gpio fd");

        assert_eq!(
            check_gpio_object_fd_write(&table, &FdWrite::new(3, b"1").expect("fd_write"), route()),
            Err(GpioFdWriteError::Fd(PicoFdError::BadRoute))
        );
    }

    #[test]
    fn rejects_non_digit_payload() {
        let table = table();

        assert_eq!(
            check_gpio_object_fd_write(&table, &FdWrite::new(3, b"on").expect("fd_write"), route()),
            Err(GpioFdWriteError::BadPayload)
        );
    }
}
