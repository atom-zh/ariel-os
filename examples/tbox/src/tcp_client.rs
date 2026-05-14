use core::fmt::Write as _;
use core::str::FromStr as _;

use ariel_os::{
    log::Debug2Format,
    time::{Duration, Timer, with_timeout},
};
use embassy_net::{Ipv4Address, Stack, tcp::TcpSocket};
use embedded_io_async_07::Write;
use heapless::String;
use static_cell::StaticCell;

use crate::modem::log_serial_bytes;
use crate::remote;

const DEFAULT_SOCKET_HOST: &str = "tbox.uatiothub.rhecube.com";
const DEFAULT_SOCKET_PORT: u16 = 8089;
const TCP_CONNECT_TIMEOUT_SECS: u64 = 30;
const TCP_IO_TIMEOUT_SECS: u64 = 10;
const TCP_SEND_INTERVAL_SECS: u64 = remote::HEARTBEAT_PERIOD_SECS;
const TCP_READ_PROBE_TIMEOUT_MS: u64 = 80;
const TCP_WRITE_RETRY_MAX: u8 = 3;
const TCP_WRITE_RETRY_BACKOFF_SECS: u64 = 1;
const TCP_LONG_MODE_RESET_THRESHOLD: u8 = 3;
const TCP_SHORT_MODE_HOLD_SENDS: u32 = 20;
const TCP_RECONNECT_DELAY_SECS: u64 = remote::RECONNECT_DELAY_SECS;

static TCP_RX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
static TCP_TX_BUF: StaticCell<[u8; 512]> = StaticCell::new();

static mut TCP_RX_BUF_PTR: Option<*mut [u8; 512]> = None;
static mut TCP_TX_BUF_PTR: Option<*mut [u8; 512]> = None;

const SOCKET_HOST: Option<&str> = option_env!("CONFIG_TBOX_SOCKET_HOST");
const SOCKET_PORT: Option<&str> = option_env!("CONFIG_TBOX_SOCKET_PORT");

struct ConnectionOutcome {
    sent_this_connection: u32,
    closed_by_reset: bool,
    closed_by_send_failure: bool,
}

fn resolve_target() -> Option<(&'static str, Ipv4Address, u16)> {
    let host = SOCKET_HOST.unwrap_or(DEFAULT_SOCKET_HOST);

    let Ok(host_addr) = Ipv4Address::from_str(host) else {
        warn!("invalid CONFIG_TBOX_SOCKET_HOST: {}", host);
        return None;
    };

    let port = SOCKET_PORT
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_SOCKET_PORT);

    Some((host, host_addr, port))
}

fn log_target(host: &str, host_addr: Ipv4Address, port: u16) {
    let [ha, hb, hc, hd] = host_addr.octets();
    info!("server target: {}:{}", host, port);
    info!(
        "server target parsed as {}.{}.{}.{}:{}",
        ha, hb, hc, hd, port
    );
}

fn log_adaptive_mode(short_mode_remaining: u32) {
    if short_mode_remaining > 0 {
        info!(
            "TCP adaptive mode: short-connection fallback active (remaining sends: {})",
            short_mode_remaining
        );
    } else {
        info!("TCP adaptive mode: long-connection preferred");
    }
}

fn update_adaptive_state(
    outcome: &ConnectionOutcome,
    early_reset_streak: &mut u8,
    short_mode_remaining: &mut u32,
) {
    if outcome.sent_this_connection <= 1
        && (outcome.closed_by_reset || outcome.closed_by_send_failure)
    {
        *early_reset_streak = early_reset_streak.saturating_add(1);
        warn!(
            "TCP early-close streak: {}/{}",
            *early_reset_streak, TCP_LONG_MODE_RESET_THRESHOLD
        );
    } else if outcome.sent_this_connection >= 2 {
        *early_reset_streak = 0;
    }

    if *short_mode_remaining == 0 && *early_reset_streak >= TCP_LONG_MODE_RESET_THRESHOLD {
        *short_mode_remaining = TCP_SHORT_MODE_HOLD_SENDS;
        *early_reset_streak = 0;
        warn!(
            "TCP switching to short-connection fallback for next {} sends (long mode remains preferred and will auto-resume)",
            TCP_SHORT_MODE_HOLD_SENDS
        );
    }
}

async fn run_connection(
    socket: &mut TcpSocket<'_>,
    host: &str,
    port: u16,
    socket_read_buf: &mut [u8; 256],
    message_counter: &mut u64,
    short_mode_remaining: &mut u32,
) -> ConnectionOutcome {
    let mut outcome = ConnectionOutcome {
        sent_this_connection: 0,
        closed_by_reset: false,
        closed_by_send_failure: false,
    };
    let mut sent_device_info = false;
    let mut heartbeat_ticks: u32 = 0;

    loop {
        if !sent_device_info {
            let Some(payload) = remote::build_device_info_packet(next_sequence(message_counter)) else {
                warn!("remote device-info packet build failed");
                outcome.closed_by_send_failure = true;
                break;
            };
            if !send_payload(socket, host, port, message_counter, "device-info", payload.as_slice()).await {
                outcome.closed_by_send_failure = true;
                break;
            }
            outcome.sent_this_connection = outcome.sent_this_connection.saturating_add(1);
            sent_device_info = true;
        }

        let Some(heartbeat) = remote::build_heartbeat_packet(next_sequence(message_counter)) else {
            warn!("remote heartbeat packet build failed");
            outcome.closed_by_send_failure = true;
            break;
        };
        if !send_payload(socket, host, port, message_counter, "heartbeat", heartbeat.as_slice()).await {
            outcome.closed_by_send_failure = true;
            break;
        }
        outcome.sent_this_connection = outcome.sent_this_connection.saturating_add(1);
        heartbeat_ticks = heartbeat_ticks.saturating_add(1);

        if heartbeat_ticks % 2 == 0 {
            let Some(detail) = remote::build_battery_detail_packet(next_sequence(message_counter)) else {
                warn!("remote battery-detail packet build failed");
                outcome.closed_by_send_failure = true;
                break;
            };
            if !send_payload(socket, host, port, message_counter, "battery-detail", detail.as_slice()).await {
                outcome.closed_by_send_failure = true;
                break;
            }
            outcome.sent_this_connection = outcome.sent_this_connection.saturating_add(1);
        }

        if *short_mode_remaining > 0 {
            *short_mode_remaining = short_mode_remaining.saturating_sub(1);
            info!(
                "TCP short-mode: closing connection after send #{} (remaining sends: {})",
                *message_counter, *short_mode_remaining
            );
            break;
        }

        match with_timeout(
            Duration::from_millis(TCP_READ_PROBE_TIMEOUT_MS),
            socket.read(socket_read_buf),
        )
        .await
        {
            Ok(Ok(0)) => {
                warn!("TCP peer closed connection after send #{}", *message_counter);
                outcome.closed_by_reset = true;
                break;
            }
            Ok(Ok(len)) => {
                info!("TCP receive after send #{}: {} bytes", *message_counter, len);
                log_serial_bytes("UART3 PPP RX", &socket_read_buf[..len]);
            }
            Ok(Err(err)) => {
                let mut err_text: String<64> = String::new();
                let _ = write!(&mut err_text, "{:?}", err);

                if err_text.contains("ConnectionReset")
                    || err_text.contains("ConnectionAborted")
                    || err_text.contains("ConnectionClosed")
                {
                    warn!(
                        "TCP connection lost after send #{} ({}), reconnecting",
                        *message_counter,
                        err_text.as_str()
                    );
                    outcome.closed_by_reset = true;
                    break;
                }
            }
            Err(_) => {}
        }

        Timer::after_secs(TCP_SEND_INTERVAL_SECS).await;
    }

    outcome
}

async fn send_payload(
    socket: &mut TcpSocket<'_>,
    host: &str,
    port: u16,
    message_counter: &mut u64,
    label: &str,
    payload: &[u8],
) -> bool {
    let current_id = message_counter.saturating_add(1);

    info!(
        "TCP send #{} {}: sending {} bytes to {}:{}",
        current_id,
        label,
        payload.len(),
        host,
        port,
    );
    log_serial_bytes("UART3 PPP TX", payload);

    for write_try in 1..=TCP_WRITE_RETRY_MAX {
        match socket.write_all(payload).await {
            Ok(()) => {
                if write_try > 1 {
                    info!(
                        "TCP send #{} succeeded on retry {}/{}",
                        current_id, write_try, TCP_WRITE_RETRY_MAX
                    );
                }
                *message_counter = current_id;
                info!("TCP send #{} {} completed", current_id, label);
                return true;
            }
            Err(err) => {
                warn!(
                    "TCP send #{} {} write attempt {}/{} failed: {:?}",
                    current_id,
                    label,
                    write_try,
                    TCP_WRITE_RETRY_MAX,
                    Debug2Format(&err)
                );
                if write_try < TCP_WRITE_RETRY_MAX {
                    Timer::after_secs(TCP_WRITE_RETRY_BACKOFF_SECS).await;
                }
            }
        }
    }

    warn!(
        "TCP send #{} {} failed before commit (last committed #{}), reconnecting",
        current_id, label, *message_counter,
    );
    false
}

fn next_sequence(message_counter: &u64) -> u8 {
    message_counter.wrapping_add(1) as u8
}

fn tcp_rx_buf() -> &'static mut [u8; 512] {
    #[expect(
        unsafe_code,
        reason = "cache StaticCell allocation across modem retries"
    )]
    unsafe {
        if let Some(ptr) = TCP_RX_BUF_PTR {
            &mut *ptr
        } else {
            let value = TCP_RX_BUF.init([0u8; 512]);
            TCP_RX_BUF_PTR = Some(value as *mut _);
            value
        }
    }
}

fn tcp_tx_buf() -> &'static mut [u8; 512] {
    #[expect(
        unsafe_code,
        reason = "cache StaticCell allocation across modem retries"
    )]
    unsafe {
        if let Some(ptr) = TCP_TX_BUF_PTR {
            &mut *ptr
        } else {
            let value = TCP_TX_BUF.init([0u8; 512]);
            TCP_TX_BUF_PTR = Some(value as *mut _);
            value
        }
    }
}

pub async fn run(stack: Stack<'static>) {
    let Some((host, host_addr, port)) = resolve_target() else {
        loop {
            Timer::after_secs(60).await;
        }
    };
    log_target(host, host_addr, port);

    let rx_buffer = tcp_rx_buf();
    let tx_buffer = tcp_tx_buf();
    let mut socket_read_buf = [0u8; 256];
    let mut connect_attempt: u32 = 0;
    let mut message_counter: u64 = 0;
    let mut early_reset_streak: u8 = 0;
    let mut short_mode_remaining: u32 = 0;

    loop {
        connect_attempt += 1;
        let mut socket = TcpSocket::new(stack, &mut rx_buffer[..], &mut tx_buffer[..]);
        socket.set_timeout(Some(Duration::from_secs(TCP_CONNECT_TIMEOUT_SECS)));

        info!(
            "TCP connect attempt #{}: starting handshake to {}:{} over PPP",
            connect_attempt, host, port,
        );
        match socket.connect((host_addr, port)).await {
            Ok(()) => {
                info!(
                    "TCP connect attempt #{} succeeded: connected to {}:{}",
                    connect_attempt, host, port,
                );
                socket.set_timeout(Some(Duration::from_secs(TCP_IO_TIMEOUT_SECS)));

                log_adaptive_mode(short_mode_remaining);

                let outcome = run_connection(
                    &mut socket,
                    host,
                    port,
                    &mut socket_read_buf,
                    &mut message_counter,
                    &mut short_mode_remaining,
                )
                .await;

                update_adaptive_state(&outcome, &mut early_reset_streak, &mut short_mode_remaining);

                info!("TCP connection closed, preparing to reconnect");
                let _ = socket.close();
            }
            Err(err) => {
                warn!(
                    "TCP connect attempt #{} failed for {}:{} ({:?}), retrying",
                    connect_attempt,
                    host,
                    port,
                    Debug2Format(&err)
                );
            }
        }

        Timer::after_secs(TCP_RECONNECT_DELAY_SECS).await;
    }
}
