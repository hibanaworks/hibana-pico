use hibana_pico::{
    choreography::protocol::MgmtImageActivate,
    kernel::{
        choreofs::{ChoreoFsError, ChoreoFsStore},
        mgmt::{ActivationBoundary, ImageSlotError, ImageSlotTable, TopologyLifecycle},
        network::{NetworkControl, NetworkError, NetworkObjectTable, NetworkRights, NetworkRoute},
        remote::{
            RemoteControl, RemoteError, RemoteObjectTable, RemoteResource, RemoteRights,
            RemoteRoute,
        },
        state::{StateRestoreFact, StateSnapshotFact},
        swarm::{NodeId, SwarmCredential},
        transaction::{ObjectTransaction, ObjectTransactionDecision},
    },
};

const COORDINATOR: NodeId = NodeId::new(1);
const SENSOR: NodeId = NodeId::new(2);
const CREDENTIAL: SwarmCredential = SwarmCredential::new(0x4849_4241);
const SESSION_GENERATION: u16 = 7;

#[test]
fn object_transaction_fact_distinguishes_commit_and_abort() {
    let commit = ObjectTransaction::commit(SESSION_GENERATION);
    assert_eq!(commit.decision(), ObjectTransactionDecision::Commit);
    assert!(commit.is_commit());

    let abort = ObjectTransaction::abort(SESSION_GENERATION);
    assert_eq!(abort.decision(), ObjectTransactionDecision::Abort);
    assert!(!abort.is_commit());
}

#[test]
fn remote_and_network_cap_grants_require_tx_commit() {
    let mut remote = RemoteObjectTable::<4>::new();
    let remote_control = RemoteControl::cap_grant_remote(
        COORDINATOR,
        CREDENTIAL,
        RemoteRoute::new(SENSOR, 1, 2, 3, SESSION_GENERATION),
        RemoteRights::Read,
        RemoteResource::Sensor,
    );
    assert_eq!(
        remote.apply_control_in_tx(
            remote_control,
            COORDINATOR,
            CREDENTIAL,
            SESSION_GENERATION,
            ObjectTransaction::abort(SESSION_GENERATION),
        ),
        Err(RemoteError::PolicyDenied)
    );
    let materialized = remote
        .apply_control_in_tx(
            remote_control,
            COORDINATOR,
            CREDENTIAL,
            SESSION_GENERATION,
            ObjectTransaction::commit(SESSION_GENERATION),
        )
        .expect("commit materializes remote object");
    assert_eq!(materialized.target_node(), SENSOR);

    let mut network = NetworkObjectTable::<4>::new();
    let network_control = NetworkControl::cap_grant_datagram(
        COORDINATOR,
        CREDENTIAL,
        NetworkRoute::new(SENSOR, 4, 5, SESSION_GENERATION),
        NetworkRights::Send,
    );
    assert_eq!(
        network.apply_control_in_tx(
            network_control,
            COORDINATOR,
            CREDENTIAL,
            SESSION_GENERATION,
            ObjectTransaction::abort(SESSION_GENERATION),
        ),
        Err(NetworkError::PolicyDenied)
    );
    let materialized = network
        .apply_control_in_tx(
            network_control,
            COORDINATOR,
            CREDENTIAL,
            SESSION_GENERATION,
            ObjectTransaction::commit(SESSION_GENERATION),
        )
        .expect("commit materializes network object");
    assert_eq!(materialized.target_node(), SENSOR);
}

#[test]
fn choreofs_object_updates_require_tx_commit() {
    let mut store = ChoreoFsStore::<4, 32, 32>::new();
    assert_eq!(
        store.install_static_blob_in_tx(b"config/name", b"hibana", ObjectTransaction::abort(1),),
        Err(ChoreoFsError::PermissionDenied)
    );
    let object_id = store
        .install_static_blob_in_tx(b"config/name", b"hibana", ObjectTransaction::commit(1))
        .expect("commit installs object");
    assert_eq!(object_id, 0);
}

#[test]
fn topology_lifecycle_commit_is_required_before_activation() {
    let mut table = ImageSlotTable::<1, 16>::new();
    let activate = MgmtImageActivate::new(0, 9);
    let boundary = ActivationBoundary::single_node(true, true, 9);
    let topology = TopologyLifecycle::new(9)
        .begin(9)
        .expect("begin topology")
        .ack(9)
        .expect("ack topology");
    let grant = hibana_pico::kernel::mgmt::MgmtControl::install_grant(
        COORDINATOR,
        CREDENTIAL,
        SESSION_GENERATION,
        0,
        9,
    );

    assert_eq!(
        table.activate_with_topology_control(
            grant,
            COORDINATOR,
            CREDENTIAL,
            SESSION_GENERATION,
            activate,
            boundary,
            topology,
        ),
        Err(ImageSlotError::NeedFence)
    );
}

#[test]
fn state_snapshot_restore_facts_are_fresh_activation_boundaries() {
    let snapshot = StateSnapshotFact::new(3, 9);
    let restore = StateRestoreFact::from_snapshot(snapshot, 10);
    assert_eq!(restore.snapshot_generation(), 3);
    assert_eq!(restore.target_memory_epoch(), 10);

    let mut store = ChoreoFsStore::<4, 32, 32>::new();
    assert_eq!(
        store.install_state_snapshot_in_tx(
            b"state/runtime",
            b"snapshot",
            ObjectTransaction::abort(1),
        ),
        Err(ChoreoFsError::PermissionDenied)
    );
    let object_id = store
        .install_state_snapshot_in_tx(b"state/runtime", b"snapshot", ObjectTransaction::commit(1))
        .expect("commit installs state snapshot");
    assert_eq!(object_id, 0);
}
