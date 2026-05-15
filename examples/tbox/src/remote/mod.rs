//! TCP cloud protocol for the swap-terminal IoT platform.
//!
//! Source document: `doc/my/换电终端至物联平台通信协议V1.2.docx`.
//! The transport endpoint remains configured in `tcp_client.rs`; this module owns
//! the application payload format and the CAN-derived telemetry snapshot.

mod protocol;
mod state;

pub use protocol::{
    HEARTBEAT_PERIOD_SECS, RECONNECT_DELAY_SECS, build_battery_detail_packet,
    build_device_info_packet, build_heartbeat_packet, build_weighing_packet,
    build_work_hour_packet,
};
pub use state::update_from_vehicle_message;
