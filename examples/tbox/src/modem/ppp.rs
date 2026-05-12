use core::sync::atomic::{AtomicU8, Ordering};

use ariel_os::{
    log::*,
    time::{Duration, with_timeout},
};
use embassy_futures::join::join;
use embassy_futures::select::{Either, select};
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4};
use embassy_net_ppp::{Config as PppConfig, Ipv4Status, State as PppState};
use static_cell::StaticCell;

use super::at::prepare_modem_with_autobaud;
use super::config::PPP_PASSWORD;
use super::config::PPP_USERNAME;
use super::state::PPP_RX_CONTROL_SEEN;
use super::uart::DmaUart;

const SOCKETS: usize = 4;
const PPP_RX_QUEUE: usize = 4;
const PPP_TX_QUEUE: usize = 4;
const PPP_SEED: u64 = 0x5442_4f58_5050_5001;
const PPP_UP_TIMEOUT_SECS: u64 = 90;
const PPP_WAIT_PROGRESS_INTERVAL_SECS: u64 = 5;
const PPP_RX_CONTROL_TIMEOUT_SECS: u64 = 12;

pub(super) const DIAL_CANDIDATE_COUNT: u8 = 3;

static NET_RESOURCES: StaticCell<StackResources<SOCKETS>> = StaticCell::new();
static PPP_STATE: StaticCell<PppState<PPP_RX_QUEUE, PPP_TX_QUEUE>> = StaticCell::new();

static mut NET_RESOURCES_PTR: Option<*mut StackResources<SOCKETS>> = None;
static mut PPP_STATE_PTR: Option<*mut PppState<PPP_RX_QUEUE, PPP_TX_QUEUE>> = None;
pub(super) static PPP_DIAL_BASE_INDEX: AtomicU8 = AtomicU8::new(0);
pub(super) static PPP_DIAL_ACTIVE_INDEX: AtomicU8 = AtomicU8::new(0);

pub enum RunOutcome {
    RetryPowerCycle,
}

pub(super) fn dial_candidate_by_index(index: u8) -> &'static str {
    match index % DIAL_CANDIDATE_COUNT {
        0 => "ATD*99#",
        1 => "ATD*99***1#",
        _ => "ATD*98*1#",
    }
}

pub(super) fn advance_dial_strategy(reason: &str) {
    let current = PPP_DIAL_BASE_INDEX.load(Ordering::Relaxed) % DIAL_CANDIDATE_COUNT;
    let next = (current + 1) % DIAL_CANDIDATE_COUNT;
    PPP_DIAL_BASE_INDEX.store(next, Ordering::Relaxed);
    info!(
        "PPP dial strategy advanced: reason=\"{}\", next=\"{}\"",
        reason,
        dial_candidate_by_index(next)
    );
}

fn advance_dial_strategy_from_active(reason: &str) {
    let active = PPP_DIAL_ACTIVE_INDEX.load(Ordering::Relaxed) % DIAL_CANDIDATE_COUNT;
    let next = (active + 1) % DIAL_CANDIDATE_COUNT;
    PPP_DIAL_BASE_INDEX.store(next, Ordering::Relaxed);
    info!(
        "PPP dial strategy advanced from active: reason=\"{}\", active=\"{}\", next=\"{}\"",
        reason,
        dial_candidate_by_index(active),
        dial_candidate_by_index(next)
    );
}

pub async fn run(uart: &mut DmaUart<'_>) -> RunOutcome {
    uart.set_io_logging(true);

    let active_baudrate;
    match prepare_modem_with_autobaud(uart).await {
        Ok(baudrate) => active_baudrate = baudrate,
        Err(err) => {
            let _ = err;
            warn!("modem bring-up failed");
            return RunOutcome::RetryPowerCycle;
        }
    }

    info!("modem entered PPP data mode at {}bps", active_baudrate);
    uart.set_io_logging(false);

    let (device, mut ppp_runner) = embassy_net_ppp::new(ppp_state());
    let (stack, mut net_runner) = embassy_net::new(
        device,
        embassy_net::Config::default(),
        net_resources(),
        PPP_SEED,
    );

    let ppp_cfg = PppConfig {
        username: PPP_USERNAME.unwrap_or("").as_bytes(),
        password: PPP_PASSWORD.unwrap_or("").as_bytes(),
    };

    info!(
        "PPP auth config: username={}, password={}",
        if ppp_cfg.username.is_empty() {
            "\"<empty>\""
        } else {
            "\"<set>\""
        },
        if ppp_cfg.password.is_empty() {
            "\"<empty>\""
        } else {
            "\"<set>\""
        }
    );

    let ppp_and_net = async move {
        info!("PPP runner starting (LCP/auth/IPCP negotiation)");
        join(
            async move { net_runner.run().await },
            async move {
                let Err(err) = ppp_runner.run(uart, ppp_cfg, |status| on_ipv4_up(stack, status)).await;
                warn!("PPP session ended: {:?}", Debug2Format(&err));
            },
        )
        .await;

        RunOutcome::RetryPowerCycle
    };

    let app = async move {
        info!("waiting for PPP network configuration...");
        let mut waited_secs = 0u64;

        loop {
            let wait_slice = (PPP_UP_TIMEOUT_SECS - waited_secs).min(PPP_WAIT_PROGRESS_INTERVAL_SECS);
            match with_timeout(Duration::from_secs(wait_slice), stack.wait_config_up()).await {
                Ok(()) => {
                    info!("PPP is up, TCP/UDP socket API is ready");
                    info!("PPP phase: network request stage");
                    info!("PPP IPCP done, sending first PPP payload request");
                    crate::tcp_client::run(stack).await;
                    return RunOutcome::RetryPowerCycle;
                }
                Err(_) => {
                    waited_secs += wait_slice;
                    info!(
                        "PPP negotiation/auth still in progress: {}s/{}s",
                        waited_secs,
                        PPP_UP_TIMEOUT_SECS
                    );
                    if waited_secs >= PPP_RX_CONTROL_TIMEOUT_SECS
                        && !PPP_RX_CONTROL_SEEN.load(Ordering::Relaxed)
                    {
                        let active = PPP_DIAL_ACTIVE_INDEX.load(Ordering::Relaxed);
                        let active_cmd = dial_candidate_by_index(active);
                        warn!(
                            "PPP has no inbound LCP/AUTH/IPCP frames within {}s after CONNECT (dial=\"{}\"), retrying modem cycle",
                            PPP_RX_CONTROL_TIMEOUT_SECS,
                            active_cmd
                        );
                        advance_dial_strategy_from_active("no inbound PPP control frame after CONNECT");
                        return RunOutcome::RetryPowerCycle;
                    }
                    if waited_secs >= PPP_UP_TIMEOUT_SECS {
                        let active = PPP_DIAL_ACTIVE_INDEX.load(Ordering::Relaxed);
                        let active_cmd = dial_candidate_by_index(active);
                        warn!(
                            "PPP did not come up within {}s (dial=\"{}\"), requesting modem power cycle",
                            PPP_UP_TIMEOUT_SECS,
                            active_cmd
                        );
                        advance_dial_strategy_from_active("PPP up timeout");
                        return RunOutcome::RetryPowerCycle;
                    }
                }
            }
        }
    };

    match select(ppp_and_net, app).await {
        Either::First(outcome) => outcome,
        Either::Second(outcome) => outcome,
    }
}

fn net_resources() -> &'static mut StackResources<SOCKETS> {
    #[expect(unsafe_code, reason = "cache StaticCell allocation across modem retries")]
    unsafe {
        if let Some(ptr) = NET_RESOURCES_PTR {
            &mut *ptr
        } else {
            let value = NET_RESOURCES.init(StackResources::new());
            NET_RESOURCES_PTR = Some(value as *mut _);
            value
        }
    }
}

fn ppp_state() -> &'static mut PppState<PPP_RX_QUEUE, PPP_TX_QUEUE> {
    #[expect(unsafe_code, reason = "cache StaticCell allocation across modem retries")]
    unsafe {
        if let Some(ptr) = PPP_STATE_PTR {
            &mut *ptr
        } else {
            let value = PPP_STATE.init(PppState::new());
            PPP_STATE_PTR = Some(value as *mut _);
            value
        }
    }
}

fn on_ipv4_up(stack: Stack<'static>, status: Ipv4Status) {
    if let Some(address) = status.address {
        let [a, b, c, d] = address.octets();
        info!("PPP IPv4 local: {}.{}.{}.{}", a, b, c, d);
    }
    if let Some(peer) = status.peer_address {
        let [a, b, c, d] = peer.octets();
        info!("PPP IPv4 peer: {}.{}.{}.{}", a, b, c, d);
    }
    let config = ipv4_status_to_config(status);
    info!("PPP IPv4 up");
    stack.set_config_v4(config.ipv4);
}

fn ipv4_status_to_config(status: Ipv4Status) -> embassy_net::Config {
    let mut config = embassy_net::Config::default();

    if let Some(address) = status.address {
        let [a, b, c, d] = address.octets();
        let our_addr = Ipv4Address::new(a, b, c, d);
        let gateway = status.peer_address.map(|peer| {
            let [a, b, c, d] = peer.octets();
            Ipv4Address::new(a, b, c, d)
        });

        config.ipv4 = embassy_net::ConfigV4::Static(StaticConfigV4 {
            address: Ipv4Cidr::new(our_addr, 32),
            dns_servers: Default::default(),
            gateway,
        });
    }

    config
}