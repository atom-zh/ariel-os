mod at;
mod config;
mod identity;
mod ipcp;
mod ppp;
mod state;
mod uart;

pub use ppp::{RunOutcome, run};
pub use uart::DmaUart;
pub(crate) use identity::{
	device_id_from_imei, report_time, set_device_id_from_imei, set_report_time,
	sync_report_time_if_drift_exceeds,
};
pub(crate) use uart::log_serial_bytes;
