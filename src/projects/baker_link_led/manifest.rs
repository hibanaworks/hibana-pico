use crate::{
    choreography::protocol::{GpioSet, LABEL_GPIO_SET},
    kernel::{
        choreofs::{ChoreoFsError, ChoreoFsStore},
        fd_object::GpioFdWriteRoute,
        wasi::{ChoreoResourceKind, PicoFdRights, PicoFdRoute},
    },
    machine::rp2040::baker_link::{BAKER_LINK_USER_LED_ACTIVE_HIGH, BAKER_LINK_USER_LED_PINS},
};

pub const BAKER_LINK_LED_FD: u8 = 3;
pub const BAKER_LINK_CHOREOFS_PREOPEN_FD: u8 = 9;
pub const BAKER_LINK_CHOREOFS_PREOPEN_LANE: u8 = 7;
pub const BAKER_LINK_CHOREOFS_PREOPEN_ROUTE_LABEL: u8 = 0;
pub const BAKER_LINK_CHOREOFS_OBJECT_LANE: u8 = 8;
pub const BAKER_LINK_CHOREOFS_OBJECT_ROUTE_LABEL: u8 = 1;
pub const BAKER_LINK_LED_PIN: u8 = BAKER_LINK_USER_LED_PINS[0];
pub const BAKER_LINK_LED_FDS: [u8; 3] = [3, 4, 5];
pub const BAKER_LINK_LED_PINS: [u8; 3] = BAKER_LINK_USER_LED_PINS;
pub const BAKER_LINK_LED_RESOURCE_PATHS: [&[u8]; 3] =
    [b"device/led/green", b"device/led/orange", b"device/led/red"];
pub const BAKER_LINK_WRONG_OBJECT_PATH: &[u8] = b"device/not-gpio";
pub const BAKER_LINK_LED_ACTIVE_HIGH: bool = BAKER_LINK_USER_LED_ACTIVE_HIGH;
pub const BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS: usize = 7;
pub const BAKER_LINK_TRAFFIC_GREEN_DELAY_TICKS: u32 = 250;
pub const BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS: u32 = 50;
pub const BAKER_LINK_TRAFFIC_RED_DELAY_TICKS: u32 = 250;
pub const BAKER_LINK_LED_LANE: u8 = 3;
pub const BAKER_LINK_LED_ROUTE_LABEL: u8 = LABEL_GPIO_SET;
pub const BAKER_LINK_LED_TARGET_NODE: u8 = 0;
pub const BAKER_LINK_LED_TARGET_ROLE: u16 = 0;
pub const BAKER_LINK_LED_SESSION_GENERATION: u16 = 0;
pub const BAKER_LINK_LED_POLICY_SLOT: u8 = 0;

pub type BakerLinkLedResourceStore = ChoreoFsStore<4, 24, 0>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakerLinkTrafficStep {
    fd: u8,
    high: bool,
    delay_ticks: u32,
}

impl BakerLinkTrafficStep {
    pub const fn new(fd: u8, high: bool, delay_ticks: u32) -> Self {
        Self {
            fd,
            high,
            delay_ticks,
        }
    }

    pub const fn fd(self) -> u8 {
        self.fd
    }

    pub const fn high(self) -> bool {
        self.high
    }

    pub const fn delay_ticks(self) -> u32 {
        self.delay_ticks
    }
}

pub const BAKER_LINK_TRAFFIC_LIGHT_PATTERN: [BakerLinkTrafficStep;
    BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS] = [
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[0],
        true,
        BAKER_LINK_TRAFFIC_GREEN_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[1],
        true,
        BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[1],
        false,
        BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[1],
        true,
        BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[1],
        false,
        BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[1],
        true,
        BAKER_LINK_TRAFFIC_ORANGE_DELAY_TICKS,
    ),
    BakerLinkTrafficStep::new(
        BAKER_LINK_LED_FDS[2],
        true,
        BAKER_LINK_TRAFFIC_RED_DELAY_TICKS,
    ),
];

pub const fn baker_link_traffic_light_step(step: usize) -> BakerLinkTrafficStep {
    BAKER_LINK_TRAFFIC_LIGHT_PATTERN[step % BAKER_LINK_TRAFFIC_LIGHT_PATTERN_STEPS]
}

pub const fn baker_link_led_route() -> PicoFdRoute {
    PicoFdRoute::new(
        BAKER_LINK_LED_TARGET_NODE,
        BAKER_LINK_LED_TARGET_ROLE,
        BAKER_LINK_LED_LANE,
        BAKER_LINK_LED_ROUTE_LABEL,
        BAKER_LINK_LED_SESSION_GENERATION,
        BAKER_LINK_LED_POLICY_SLOT,
    )
}

pub const fn baker_link_led_fd_write_route() -> GpioFdWriteRoute {
    GpioFdWriteRoute::new(
        &BAKER_LINK_LED_FDS,
        &BAKER_LINK_LED_PINS,
        BAKER_LINK_LED_ACTIVE_HIGH,
        baker_link_led_route(),
    )
}

pub fn baker_link_led_resource_store() -> Result<BakerLinkLedResourceStore, ChoreoFsError> {
    let mut store = ChoreoFsStore::new();
    for path in BAKER_LINK_LED_RESOURCE_PATHS {
        store.install_gpio_device(path)?;
    }
    store.install_config_cell(BAKER_LINK_WRONG_OBJECT_PATH, &[])?;
    Ok(store)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakerLinkChoreoFsOpen {
    fd: u8,
    rights: PicoFdRights,
    resource: ChoreoResourceKind,
    lane: u8,
    route_label: u8,
    object_id: u16,
    target_node: u8,
    target_role: u16,
    session_generation: u16,
    object_generation: u16,
    policy_slot: u8,
}

impl BakerLinkChoreoFsOpen {
    pub const fn fd(self) -> u8 {
        self.fd
    }

    pub const fn rights(self) -> PicoFdRights {
        self.rights
    }

    pub const fn resource(self) -> ChoreoResourceKind {
        self.resource
    }

    pub const fn lane(self) -> u8 {
        self.lane
    }

    pub const fn route_label(self) -> u8 {
        self.route_label
    }

    pub const fn object_id(self) -> u16 {
        self.object_id
    }

    pub const fn target_node(self) -> u8 {
        self.target_node
    }

    pub const fn target_role(self) -> u16 {
        self.target_role
    }

    pub const fn session_generation(self) -> u16 {
        self.session_generation
    }

    pub const fn object_generation(self) -> u16 {
        self.object_generation
    }

    pub const fn policy_slot(self) -> u8 {
        self.policy_slot
    }
}

pub fn resolve_baker_link_choreofs_object(
    store: &BakerLinkLedResourceStore,
    path: &[u8],
    rights: PicoFdRights,
) -> Result<BakerLinkChoreoFsOpen, ChoreoFsError> {
    let selector = baker_link_choreofs_selector(path);
    let new_fd = baker_link_choreofs_fd_for_selector(selector)?;
    let opened = store.open(selector, rights)?;
    let (lane, route_label, target_role, policy_slot) = match opened.resource() {
        ChoreoResourceKind::Gpio => (
            BAKER_LINK_LED_LANE,
            BAKER_LINK_LED_ROUTE_LABEL,
            BAKER_LINK_LED_TARGET_ROLE,
            BAKER_LINK_LED_POLICY_SLOT,
        ),
        _ => (
            BAKER_LINK_CHOREOFS_OBJECT_LANE,
            BAKER_LINK_CHOREOFS_OBJECT_ROUTE_LABEL,
            opened.object_id(),
            0,
        ),
    };
    Ok(BakerLinkChoreoFsOpen {
        fd: new_fd,
        rights,
        resource: opened.resource(),
        lane,
        route_label,
        object_id: opened.object_id(),
        target_node: BAKER_LINK_LED_TARGET_NODE,
        target_role,
        session_generation: BAKER_LINK_LED_SESSION_GENERATION,
        object_generation: opened.generation(),
        policy_slot,
    })
}

fn baker_link_choreofs_selector(path: &[u8]) -> &[u8] {
    match path.split_first() {
        Some((b'/', rest)) => rest,
        _ => path,
    }
}

fn baker_link_choreofs_fd_for_selector(path: &[u8]) -> Result<u8, ChoreoFsError> {
    for (index, candidate) in BAKER_LINK_LED_RESOURCE_PATHS.iter().enumerate() {
        if *candidate == path {
            return Ok(BAKER_LINK_LED_FDS[index]);
        }
    }
    if path == BAKER_LINK_WRONG_OBJECT_PATH {
        return Ok(BAKER_LINK_LED_FD);
    }
    Err(ChoreoFsError::NotFound)
}

#[cfg(test)]
fn baker_link_led_index_for_fd(fd: u8) -> Option<usize> {
    BAKER_LINK_LED_FDS
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| (*candidate == fd).then_some(index))
}

pub fn baker_link_led_pin_for_fd(fd: u8) -> Option<u8> {
    baker_link_led_fd_write_route().pin_for_fd(fd)
}

pub fn apply_baker_link_led_bank_set(mut write_pin: impl FnMut(u8, bool), set: GpioSet) {
    if set.high() == BAKER_LINK_LED_ACTIVE_HIGH && BAKER_LINK_LED_PINS.contains(&set.pin()) {
        for pin in BAKER_LINK_LED_PINS {
            write_pin(pin, !BAKER_LINK_LED_ACTIVE_HIGH);
        }
    }
    write_pin(set.pin(), set.high());
}

#[cfg(test)]
mod tests {
    use super::{
        BAKER_LINK_LED_ACTIVE_HIGH, BAKER_LINK_LED_FD, BAKER_LINK_LED_PIN, BAKER_LINK_LED_PINS,
        BAKER_LINK_LED_RESOURCE_PATHS, BAKER_LINK_LED_SESSION_GENERATION,
        apply_baker_link_led_bank_set, baker_link_led_fd_write_route,
        baker_link_led_resource_store,
    };
    use crate::{
        choreography::protocol::FdWrite,
        kernel::{
            fd_object::{GpioFdWriteError, check_gpio_object_fd_write},
            wasi::{ChoreoResourceKind, PicoFdRights, PicoFdRoute, PicoFdView, PicoFdViewSource},
        },
    };

    fn grant_led_fd<const N: usize>(fds: &mut PicoFdView<N>, fd: u8) {
        let store = baker_link_led_resource_store().expect("create Baker LED resource store");
        let index = super::baker_link_led_index_for_fd(fd).expect("Baker LED fd");
        let opened = store
            .open(BAKER_LINK_LED_RESOURCE_PATHS[index], PicoFdRights::Write)
            .expect("open Baker LED resource");
        let route = PicoFdRoute::new(
            super::BAKER_LINK_LED_TARGET_NODE,
            super::BAKER_LINK_LED_TARGET_ROLE,
            super::BAKER_LINK_LED_LANE,
            super::BAKER_LINK_LED_ROUTE_LABEL,
            BAKER_LINK_LED_SESSION_GENERATION,
            super::BAKER_LINK_LED_POLICY_SLOT,
        );
        fds.apply_cap_mint(
            fd,
            PicoFdRights::Write,
            opened.resource(),
            opened.object_id(),
            opened.generation(),
            route,
        )
        .expect("mint Baker LED fd view");
    }

    fn grant_led_sequence_fds<const N: usize>(fds: &mut PicoFdView<N>) {
        for fd in super::BAKER_LINK_LED_FDS {
            grant_led_fd(fds, fd);
        }
    }

    #[test]
    fn digit_one_selects_baker_link_led_active_level() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        grant_led_fd(&mut fds, BAKER_LINK_LED_FD);
        let write = FdWrite::new(BAKER_LINK_LED_FD, b"1").expect("fd_write payload");

        let set = check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route())
            .expect("resolve Baker Link LED write");
        assert_eq!(set.pin(), BAKER_LINK_LED_PIN);
        assert_eq!(set.high(), BAKER_LINK_LED_ACTIVE_HIGH);
    }

    #[test]
    fn digit_zero_sets_baker_link_led_inactive_level() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        grant_led_fd(&mut fds, BAKER_LINK_LED_FD);
        let write = FdWrite::new(BAKER_LINK_LED_FD, b"0").expect("fd_write payload");

        let set = check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route())
            .expect("resolve Baker Link LED write");
        assert_eq!(set.pin(), BAKER_LINK_LED_PIN);
        assert_eq!(set.high(), !BAKER_LINK_LED_ACTIVE_HIGH);
    }

    #[test]
    fn sequence_fds_select_each_baker_link_led_pin() {
        let mut fds: PicoFdView<3> = PicoFdView::new();
        grant_led_sequence_fds(&mut fds);

        for (fd, pin) in super::BAKER_LINK_LED_FDS
            .into_iter()
            .zip(BAKER_LINK_LED_PINS)
        {
            let write = FdWrite::new(fd, b"1").expect("fd_write payload");
            let set = check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route())
                .expect("resolve Baker Link LED sequence write");
            assert_eq!(set.pin(), pin);
            assert_eq!(set.high(), BAKER_LINK_LED_ACTIVE_HIGH);
        }
    }

    #[test]
    fn baker_led_manifest_paths_are_choreofs_device_objects() {
        let store = baker_link_led_resource_store().expect("create Baker LED resource store");
        for path in BAKER_LINK_LED_RESOURCE_PATHS {
            let object = store
                .open(path, PicoFdRights::Write)
                .expect("open LED path");
            assert_eq!(object.resource(), ChoreoResourceKind::Gpio);
        }
    }

    #[test]
    fn bank_set_keeps_led_sequence_one_hot() {
        let mut fds: PicoFdView<3> = PicoFdView::new();
        grant_led_sequence_fds(&mut fds);
        let mut levels = [!BAKER_LINK_LED_ACTIVE_HIGH; 32];

        for fd in super::BAKER_LINK_LED_FDS {
            let write = FdWrite::new(fd, b"1").expect("fd_write payload");
            let set = check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route())
                .expect("resolve Baker Link LED sequence write");
            apply_baker_link_led_bank_set(|pin, high| levels[pin as usize] = high, set);
            let active_count = BAKER_LINK_LED_PINS
                .iter()
                .filter(|pin| levels[**pin as usize] == BAKER_LINK_LED_ACTIVE_HIGH)
                .count();
            assert_eq!(active_count, 1);
            assert_eq!(levels[set.pin() as usize], BAKER_LINK_LED_ACTIVE_HIGH);
        }
    }

    #[test]
    fn non_digit_payload_rejects() {
        let mut fds: PicoFdView<1> = PicoFdView::new();
        grant_led_fd(&mut fds, BAKER_LINK_LED_FD);
        let write = FdWrite::new(BAKER_LINK_LED_FD, b"on").expect("fd_write payload");

        assert_eq!(
            check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route()),
            Err(GpioFdWriteError::BadPayload)
        );
    }

    #[test]
    fn stdout_fd_does_not_route_to_led() {
        let fds: PicoFdView<1> = PicoFdView::new();
        let write = FdWrite::new(1, b"1").expect("fd_write payload");

        assert_eq!(
            check_gpio_object_fd_write(&fds, &write, baker_link_led_fd_write_route()),
            Err(GpioFdWriteError::BadFd)
        );
    }

    #[test]
    fn sequence_led_fds_are_minted_from_project_manifest_objects() {
        let mut fds: PicoFdView<3> = PicoFdView::new();
        grant_led_sequence_fds(&mut fds);

        for fd in super::BAKER_LINK_LED_FDS {
            let view = fds
                .resolve_current(fd, PicoFdRights::Write, ChoreoResourceKind::Gpio)
                .expect("resolve minted led fd");
            assert_eq!(view.source(), PicoFdViewSource::Mint);
            assert_ne!(view.choreo_object_generation(), 0);
        }
    }
}
