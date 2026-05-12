use core::sync::atomic::AtomicBool;

pub(super) static PPP_RX_CONTROL_SEEN: AtomicBool = AtomicBool::new(false);