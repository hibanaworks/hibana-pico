use crate::{
    kernel::{
        choreofs::{ChoreoFsError, pico_rights_from_wasip1_base},
        guest_ledger::{GuestFd, GuestLedger},
        wasi::{ChoreoResourceKind, PicoFdRights, PicoFdRoute, PicoFdView, PicoFdViewEntry},
    },
    projects::baker_link_led::manifest::{
        BAKER_LINK_CHOREOFS_PREOPEN_FD, BAKER_LINK_CHOREOFS_PREOPEN_LANE,
        BAKER_LINK_CHOREOFS_PREOPEN_ROUTE_LABEL, BAKER_LINK_LED_FD, BAKER_LINK_LED_FDS,
        BAKER_LINK_LED_LANE, BAKER_LINK_LED_POLICY_SLOT, BAKER_LINK_LED_RESOURCE_PATHS,
        BAKER_LINK_LED_ROUTE_LABEL, BAKER_LINK_LED_SESSION_GENERATION, BAKER_LINK_LED_TARGET_NODE,
        BAKER_LINK_LED_TARGET_ROLE, BakerLinkChoreoFsOpen, BakerLinkLedResourceStore,
        baker_link_led_resource_store, resolve_baker_link_choreofs_object,
    },
};

pub fn grant_baker_link_choreofs_preopen<
    const FDS: usize,
    const LEASES: usize,
    const PENDING: usize,
>(
    ledger: &mut GuestLedger<FDS, LEASES, PENDING>,
) -> Result<(), ChoreoFsError> {
    ledger.apply_fd_cap_grant(
        BAKER_LINK_CHOREOFS_PREOPEN_FD,
        PicoFdRights::Read,
        ChoreoResourceKind::PreopenRoot,
        BAKER_LINK_CHOREOFS_PREOPEN_LANE,
        BAKER_LINK_CHOREOFS_PREOPEN_ROUTE_LABEL,
        0,
        0,
        0,
        0,
        0,
    )?;
    Ok(())
}

pub fn baker_link_choreofs_ledger<const FDS: usize, const LEASES: usize, const PENDING: usize>(
    _store: &BakerLinkLedResourceStore,
    memory_len: u32,
    memory_epoch: u32,
) -> Result<GuestLedger<FDS, LEASES, PENDING>, ChoreoFsError> {
    let mut ledger = GuestLedger::pico_min(memory_len, memory_epoch);
    grant_baker_link_choreofs_preopen(&mut ledger)?;
    Ok(ledger)
}

pub fn resolve_baker_link_choreofs_path<
    const FDS: usize,
    const LEASES: usize,
    const PENDING: usize,
>(
    store: &BakerLinkLedResourceStore,
    ledger: &GuestLedger<FDS, LEASES, PENDING>,
    path: &[u8],
    rights_base: u64,
) -> Result<BakerLinkChoreoFsOpen, ChoreoFsError> {
    ledger.resolve_fd(
        BAKER_LINK_CHOREOFS_PREOPEN_FD,
        PicoFdRights::Read,
        ChoreoResourceKind::PreopenRoot,
    )?;
    let rights = pico_rights_from_wasip1_base(rights_base);
    resolve_baker_link_choreofs_object(store, path, rights)
}

pub fn mint_baker_link_choreofs_fd<const FDS: usize, const LEASES: usize, const PENDING: usize>(
    ledger: &mut GuestLedger<FDS, LEASES, PENDING>,
    opened: BakerLinkChoreoFsOpen,
) -> Result<GuestFd, ChoreoFsError> {
    Ok(ledger.apply_fd_cap_mint(
        opened.fd(),
        opened.rights(),
        opened.resource(),
        opened.lane(),
        opened.route_label(),
        opened.object_id(),
        opened.target_node(),
        opened.target_role(),
        opened.session_generation(),
        opened.object_generation(),
        opened.policy_slot(),
    )?)
}

pub fn grant_baker_link_led_fd<const N: usize>(
    table: &mut PicoFdView<N>,
) -> Result<PicoFdViewEntry, ChoreoFsError> {
    let store = baker_link_led_resource_store()?;
    mint_baker_link_led_fd_for(table, &store, BAKER_LINK_LED_FD)
}

pub fn grant_baker_link_led_sequence_fds<const N: usize>(
    table: &mut PicoFdView<N>,
) -> Result<(), ChoreoFsError> {
    let store = baker_link_led_resource_store()?;
    for fd in BAKER_LINK_LED_FDS {
        mint_baker_link_led_fd_for(table, &store, fd)?;
    }
    Ok(())
}

fn mint_baker_link_led_fd_for<const N: usize>(
    table: &mut PicoFdView<N>,
    store: &BakerLinkLedResourceStore,
    fd: u8,
) -> Result<PicoFdViewEntry, ChoreoFsError> {
    let index = baker_link_led_index_for_fd(fd).ok_or(ChoreoFsError::NotFound)?;
    let opened = store.open(BAKER_LINK_LED_RESOURCE_PATHS[index], PicoFdRights::Write)?;
    let route = PicoFdRoute::new(
        BAKER_LINK_LED_TARGET_NODE,
        BAKER_LINK_LED_TARGET_ROLE,
        BAKER_LINK_LED_LANE,
        BAKER_LINK_LED_ROUTE_LABEL,
        BAKER_LINK_LED_SESSION_GENERATION,
        BAKER_LINK_LED_POLICY_SLOT,
    );
    Ok(table.apply_cap_mint(
        fd,
        PicoFdRights::Write,
        opened.resource(),
        opened.object_id(),
        opened.generation(),
        route,
    )?)
}

pub fn grant_baker_link_led_sequence_ledger<
    const FDS: usize,
    const LEASES: usize,
    const PENDING: usize,
>(
    ledger: &mut GuestLedger<FDS, LEASES, PENDING>,
) -> Result<(), ChoreoFsError> {
    let store = baker_link_led_resource_store()?;
    for (index, fd) in BAKER_LINK_LED_FDS.into_iter().enumerate() {
        let opened = store.open(BAKER_LINK_LED_RESOURCE_PATHS[index], PicoFdRights::Write)?;
        ledger.apply_fd_cap_mint(
            fd,
            PicoFdRights::Write,
            opened.resource(),
            BAKER_LINK_LED_LANE,
            BAKER_LINK_LED_ROUTE_LABEL,
            opened.object_id(),
            BAKER_LINK_LED_TARGET_NODE,
            BAKER_LINK_LED_TARGET_ROLE,
            BAKER_LINK_LED_SESSION_GENERATION,
            opened.generation(),
            BAKER_LINK_LED_POLICY_SLOT,
        )?;
    }
    Ok(())
}

pub fn baker_link_pico_min_ledger<const LEASES: usize, const PENDING: usize>(
    memory_len: u32,
    memory_epoch: u32,
) -> Result<GuestLedger<3, LEASES, PENDING>, ChoreoFsError> {
    let mut ledger = GuestLedger::pico_min(memory_len, memory_epoch);
    grant_baker_link_led_sequence_ledger(&mut ledger)?;
    Ok(ledger)
}

fn baker_link_led_index_for_fd(fd: u8) -> Option<usize> {
    BAKER_LINK_LED_FDS
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| (*candidate == fd).then_some(index))
}

#[cfg(test)]
mod tests {
    use super::{baker_link_pico_min_ledger, grant_baker_link_led_sequence_fds};
    use crate::{
        kernel::wasi::{ChoreoResourceKind, PicoFdRights, PicoFdView, PicoFdViewSource},
        projects::baker_link_led::manifest::{
            BAKER_LINK_LED_FDS, BAKER_LINK_LED_SESSION_GENERATION,
        },
    };

    #[test]
    fn sequence_led_fds_are_project_materialized_from_baker_objects() {
        let mut fds: PicoFdView<3> = PicoFdView::new();
        grant_baker_link_led_sequence_fds(&mut fds).expect("grant led sequence fds");

        for fd in BAKER_LINK_LED_FDS {
            let view = fds
                .resolve_current(fd, PicoFdRights::Write, ChoreoResourceKind::Gpio)
                .expect("resolve minted led fd");
            assert_eq!(view.source(), PicoFdViewSource::Mint);
        }
    }

    #[test]
    fn pico_min_ledger_materializes_baker_led_fds_in_project_layer() {
        let ledger = baker_link_pico_min_ledger::<1, 1>(4096, 1).expect("create Baker ledger");
        for fd in BAKER_LINK_LED_FDS {
            let view = ledger
                .resolve_fd(fd, PicoFdRights::Write, ChoreoResourceKind::Gpio)
                .expect("resolve Baker LED fd");
            assert_eq!(view.source(), PicoFdViewSource::Mint);
            assert_eq!(view.wait_or_subscription_id(), u16::from(fd - 3));
            assert_eq!(
                view.route().session_generation(),
                BAKER_LINK_LED_SESSION_GENERATION
            );
            assert_ne!(view.choreo_object_generation(), 0);
        }
    }
}
