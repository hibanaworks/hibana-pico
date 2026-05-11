use core::mem::size_of;

use hibana::{Endpoint, substrate::program::RoleProgram};

use crate::{
    choreography::protocol::{EngineReq, EngineRet, WASIP1_STREAM_CHUNK_CAPACITY},
    kernel::app::{AppLeaseTable, AppStreamTable},
    kernel::budget::BudgetController,
    kernel::device::timer::TimerSleepTable,
    kernel::mgmt::{ImageSlotTable, ManagementRejectionTelemetry},
    kernel::network::{
        NET_DATAGRAM_PAYLOAD_CAPACITY, NET_STREAM_PAYLOAD_CAPACITY,
        NetworkObjectRejectionTelemetry, NetworkObjectTable,
    },
    kernel::remote::{RemoteObjectTable, RemoteRejectionTelemetry},
    kernel::resolver::{PicoInterruptResolver, ResolverRejectionTelemetry},
    kernel::swarm::{
        HostSwarmMedium, HostSwarmRoleTransport, NeighborTable, ReplayWindow, SWARM_AUTH_TAG_LEN,
        SWARM_FRAGMENT_CHUNK_CAPACITY, SWARM_FRAGMENT_HEADER_LEN, SWARM_FRAME_HEADER_LEN,
        SWARM_FRAME_MAX_WIRE_LEN, SWARM_FRAME_PAYLOAD_CAPACITY, SwarmDropTelemetry, SwarmFrame,
        SwarmReassemblyBuffer,
    },
    kernel::wasi::{
        MemoryLeaseRejectionTelemetry, MemoryLeaseTable, PicoFdRejectionTelemetry, PicoFdView,
        WASIP1_STATIC_ARG_BYTES_CAPACITY, WASIP1_STATIC_ARGS_CAPACITY,
        WASIP1_STATIC_ENV_BYTES_CAPACITY, WASIP1_STATIC_ENV_CAPACITY, Wasip1StaticArgEnv,
    },
    machine::rp2350::cyw43439::{QEMU_CYW43439_MAX_ROLES, QemuCyw43439Transport},
    port::host_queue::{FrameOwned, PAYLOAD_CAPACITY, QUEUE_CAPACITY, ROLE_CAPACITY},
};

pub const DEFAULT_TABLE_SLOTS: usize = 8;
pub const DEFAULT_IMAGE_SLOTS: usize = 2;
pub const DEFAULT_IMAGE_SLOT_BYTES: usize = 512;
pub const WASIP1_FULL_SUBSET_IMPORT_COUNT: usize = 46;
pub const PICO2W_SWARM_MIN_NODES: u8 = 2;
pub const PICO2W_SWARM_DEFAULT_NODES: u8 = QEMU_CYW43439_MAX_ROLES as u8;
pub const PICO2W_SWARM_DEFAULT_SENSOR_NODES: u8 = PICO2W_SWARM_DEFAULT_NODES.saturating_sub(1);
pub const PICO2W_SWARM_BASE_SAMPLE_VALUE: u32 = 0x0000_a5a5;
pub const PICO2W_SWARM_PING_PONG_NODES: usize = 2;
pub const PICO2W_SWARM_PING_PONG_MESSAGES: usize = 2;
pub const PICO2W_SWARM_REMOTE_FD_READ_MESSAGES: usize = 2;
pub const PICO2W_SWARM_REMOTE_ACTUATE_MESSAGES: usize = 2;
pub const PICO2W_SWARM_PACKET_LOSS_REDELIVERY_FRAMES: usize = 1;
pub const PICO2W_SWARM_JOIN_MESSAGES: usize = 4;
pub const PICO2W_SWARM_LEAVE_REVOKE_MESSAGES: usize = 4;

pub const fn pico2w_swarm_sample_value(node_id: u16) -> u32 {
    let offset = if node_id > PICO2W_SWARM_MIN_NODES as u16 {
        node_id - PICO2W_SWARM_MIN_NODES as u16
    } else {
        0
    };
    PICO2W_SWARM_BASE_SAMPLE_VALUE.wrapping_add(offset as u32)
}

pub const fn pico2w_swarm_expected_aggregate(node_count: u8) -> u32 {
    let mut sum = 0u32;
    let mut node = PICO2W_SWARM_MIN_NODES;
    while node <= node_count {
        sum = sum.wrapping_add(pico2w_swarm_sample_value(node as u16));
        node = node.saturating_add(1);
    }
    sum
}

pub const PICO2W_SWARM_DEFAULT_AGGREGATE: u32 =
    pico2w_swarm_expected_aggregate(PICO2W_SWARM_DEFAULT_NODES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SingleNodeMetrics {
    pub sio_role_capacity: usize,
    pub sio_queue_capacity: usize,
    pub sio_frame_payload_capacity: usize,
    pub sio_frame_size: usize,
    pub syscall_request_size: usize,
    pub syscall_response_size: usize,
    pub syscall_buffer_capacity: usize,
    pub timer_table_size: usize,
    pub interrupt_resolver_size: usize,
    pub interrupt_resolver_rejection_telemetry_size: usize,
    pub budget_controller_size: usize,
    pub memory_lease_table_size: usize,
    pub memory_lease_rejection_telemetry_size: usize,
    pub pico_fd_view_size: usize,
    pub pico_fd_rejection_telemetry_size: usize,
    pub app_stream_table_size: usize,
    pub app_lease_table_size: usize,
    pub endpoint_size: usize,
    pub role_program_size: usize,
    pub static_arg_env_size: usize,
    pub image_slot_table_size: usize,
    pub management_rejection_telemetry_size: usize,
}

impl SingleNodeMetrics {
    pub const fn collect() -> Self {
        Self {
            sio_role_capacity: ROLE_CAPACITY,
            sio_queue_capacity: QUEUE_CAPACITY,
            sio_frame_payload_capacity: PAYLOAD_CAPACITY,
            sio_frame_size: size_of::<FrameOwned>(),
            syscall_request_size: size_of::<EngineReq>(),
            syscall_response_size: size_of::<EngineRet>(),
            syscall_buffer_capacity: WASIP1_STREAM_CHUNK_CAPACITY,
            timer_table_size: size_of::<TimerSleepTable<DEFAULT_TABLE_SLOTS>>(),
            interrupt_resolver_size: size_of::<PicoInterruptResolver<4, 8, 4>>(),
            interrupt_resolver_rejection_telemetry_size: size_of::<ResolverRejectionTelemetry>(),
            budget_controller_size: size_of::<BudgetController>(),
            memory_lease_table_size: size_of::<MemoryLeaseTable<DEFAULT_TABLE_SLOTS>>(),
            memory_lease_rejection_telemetry_size: size_of::<MemoryLeaseRejectionTelemetry>(),
            pico_fd_view_size: size_of::<PicoFdView<DEFAULT_TABLE_SLOTS>>(),
            pico_fd_rejection_telemetry_size: size_of::<PicoFdRejectionTelemetry>(),
            app_stream_table_size: size_of::<AppStreamTable<DEFAULT_TABLE_SLOTS>>(),
            app_lease_table_size: size_of::<AppLeaseTable<DEFAULT_TABLE_SLOTS>>(),
            endpoint_size: size_of::<Endpoint<'static, 0>>(),
            role_program_size: size_of::<RoleProgram<0>>(),
            static_arg_env_size: size_of::<Wasip1StaticArgEnv<'static>>(),
            image_slot_table_size: size_of::<
                ImageSlotTable<DEFAULT_IMAGE_SLOTS, DEFAULT_IMAGE_SLOT_BYTES>,
            >(),
            management_rejection_telemetry_size: size_of::<ManagementRejectionTelemetry>(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwarmMetrics {
    pub wifi_frame_header_len: usize,
    pub wifi_frame_payload_capacity: usize,
    pub wifi_auth_tag_len: usize,
    pub wifi_frame_max_wire_len: usize,
    pub wifi_frame_size: usize,
    pub fragmentation_header_len: usize,
    pub fragmentation_chunk_capacity: usize,
    pub fragmentation_buffer_size: usize,
    pub qemu_cyw_max_roles: usize,
    pub qemu_cyw_transport_size: usize,
    pub host_swarm_medium_size: usize,
    pub host_swarm_role_transport_size: usize,
    pub neighbor_table_size: usize,
    pub remote_object_table_size: usize,
    pub remote_object_rejection_telemetry_size: usize,
    pub replay_window_size: usize,
    pub swarm_drop_telemetry_size: usize,
    pub wifi_ping_pong_nodes: usize,
    pub wifi_ping_pong_messages: usize,
    pub remote_fd_read_messages: usize,
    pub remote_actuator_command_messages: usize,
    pub packet_loss_redelivery_frames: usize,
    pub provisioning_join_messages: usize,
    pub leave_revoke_messages: usize,
    pub qemu_swarm_default_nodes: usize,
    pub qemu_swarm_default_sensor_nodes: usize,
    pub qemu_swarm_sample_count: usize,
    pub qemu_swarm_wasip1_fd_write_count: usize,
    pub qemu_swarm_aggregate_ack_count: usize,
    pub qemu_swarm_base_sample_value: u32,
    pub qemu_swarm_default_aggregate: u32,
}

impl SwarmMetrics {
    pub const fn collect() -> Self {
        Self {
            wifi_frame_header_len: SWARM_FRAME_HEADER_LEN,
            wifi_frame_payload_capacity: SWARM_FRAME_PAYLOAD_CAPACITY,
            wifi_auth_tag_len: SWARM_AUTH_TAG_LEN,
            wifi_frame_max_wire_len: SWARM_FRAME_MAX_WIRE_LEN,
            wifi_frame_size: size_of::<SwarmFrame>(),
            fragmentation_header_len: SWARM_FRAGMENT_HEADER_LEN,
            fragmentation_chunk_capacity: SWARM_FRAGMENT_CHUNK_CAPACITY,
            fragmentation_buffer_size: size_of::<SwarmReassemblyBuffer<256, 4>>(),
            qemu_cyw_max_roles: QEMU_CYW43439_MAX_ROLES,
            qemu_cyw_transport_size: size_of::<QemuCyw43439Transport>(),
            host_swarm_medium_size: size_of::<HostSwarmMedium<DEFAULT_TABLE_SLOTS>>(),
            host_swarm_role_transport_size: size_of::<
                HostSwarmRoleTransport<'static, DEFAULT_TABLE_SLOTS, QEMU_CYW43439_MAX_ROLES>,
            >(),
            neighbor_table_size: size_of::<NeighborTable<DEFAULT_TABLE_SLOTS>>(),
            remote_object_table_size: size_of::<RemoteObjectTable<DEFAULT_TABLE_SLOTS>>(),
            remote_object_rejection_telemetry_size: size_of::<RemoteRejectionTelemetry>(),
            replay_window_size: size_of::<ReplayWindow>(),
            swarm_drop_telemetry_size: size_of::<SwarmDropTelemetry>(),
            wifi_ping_pong_nodes: PICO2W_SWARM_PING_PONG_NODES,
            wifi_ping_pong_messages: PICO2W_SWARM_PING_PONG_MESSAGES,
            remote_fd_read_messages: PICO2W_SWARM_REMOTE_FD_READ_MESSAGES,
            remote_actuator_command_messages: PICO2W_SWARM_REMOTE_ACTUATE_MESSAGES,
            packet_loss_redelivery_frames: PICO2W_SWARM_PACKET_LOSS_REDELIVERY_FRAMES,
            provisioning_join_messages: PICO2W_SWARM_JOIN_MESSAGES,
            leave_revoke_messages: PICO2W_SWARM_LEAVE_REVOKE_MESSAGES,
            qemu_swarm_default_nodes: PICO2W_SWARM_DEFAULT_NODES as usize,
            qemu_swarm_default_sensor_nodes: PICO2W_SWARM_DEFAULT_SENSOR_NODES as usize,
            qemu_swarm_sample_count: PICO2W_SWARM_DEFAULT_SENSOR_NODES as usize,
            qemu_swarm_wasip1_fd_write_count: PICO2W_SWARM_DEFAULT_SENSOR_NODES as usize,
            qemu_swarm_aggregate_ack_count: PICO2W_SWARM_DEFAULT_SENSOR_NODES as usize,
            qemu_swarm_base_sample_value: PICO2W_SWARM_BASE_SAMPLE_VALUE,
            qemu_swarm_default_aggregate: PICO2W_SWARM_DEFAULT_AGGREGATE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoP2Metrics {
    pub wasip1_full_subset_import_count: usize,
    pub wasip1_static_args_capacity: usize,
    pub wasip1_static_env_capacity: usize,
    pub wasip1_static_arg_bytes_capacity: usize,
    pub wasip1_static_env_bytes_capacity: usize,
    pub network_object_table_size: usize,
    pub network_object_rejection_telemetry_size: usize,
    pub network_datagram_payload_capacity: usize,
    pub network_stream_payload_capacity: usize,
    pub component_model_loader_bytes: usize,
    pub wit_runtime_table_bytes: usize,
    pub p2_resource_table_bytes: usize,
}

impl NoP2Metrics {
    pub const fn collect() -> Self {
        Self {
            wasip1_full_subset_import_count: WASIP1_FULL_SUBSET_IMPORT_COUNT,
            wasip1_static_args_capacity: WASIP1_STATIC_ARGS_CAPACITY,
            wasip1_static_env_capacity: WASIP1_STATIC_ENV_CAPACITY,
            wasip1_static_arg_bytes_capacity: WASIP1_STATIC_ARG_BYTES_CAPACITY,
            wasip1_static_env_bytes_capacity: WASIP1_STATIC_ENV_BYTES_CAPACITY,
            network_object_table_size: size_of::<NetworkObjectTable<DEFAULT_TABLE_SLOTS>>(),
            network_object_rejection_telemetry_size: size_of::<NetworkObjectRejectionTelemetry>(),
            network_datagram_payload_capacity: NET_DATAGRAM_PAYLOAD_CAPACITY,
            network_stream_payload_capacity: NET_STREAM_PAYLOAD_CAPACITY,
            component_model_loader_bytes: 0,
            wit_runtime_table_bytes: 0,
            p2_resource_table_bytes: 0,
        }
    }
}

pub const SINGLE_NODE_METRICS: SingleNodeMetrics = SingleNodeMetrics::collect();
pub const SWARM_METRICS: SwarmMetrics = SwarmMetrics::collect();
pub const NO_P2_METRICS: NoP2Metrics = NoP2Metrics::collect();
