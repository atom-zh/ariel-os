use core::fmt;
use core::fmt::Write as _;
use core::str::FromStr as _;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use ariel_os::{
    log::*,
    time::{Duration, Timer, with_timeout},
};
use embassy_futures::select::{Either, select};
use embassy_futures::join::join;
use embassy_stm32::{mode::Async, usart::Uart as EmbassyUart};
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4, tcp::TcpSocket};
use embassy_net_ppp::{Config as PppConfig, Ipv4Status, State as PppState};
use embedded_io_async_07 as embedded_io_async_new;
use embedded_io_async_new::{BufRead, Read, Write};
use heapless::String;
use static_cell::StaticCell;

const SOCKETS: usize = 4;
const PPP_RX_QUEUE: usize = 4;
const PPP_TX_QUEUE: usize = 4;
const UART_LINE_BUFFER: usize = 128;
const PPP_SEED: u64 = 0x5442_4f58_5050_5001;
const SERIAL_LOG_PREVIEW: usize = 48;
const DEFAULT_SOCKET_HOST: &str = "47.103.151.216";
const DEFAULT_SOCKET_PORT: u16 = 8089;
const TCP_CONNECT_TIMEOUT_SECS: u64 = 30;
const TCP_IO_TIMEOUT_SECS: u64 = 10;
const TCP_SEND_INTERVAL_MS: u64 = 800;
const TCP_READ_PROBE_TIMEOUT_MS: u64 = 80;
const TCP_WRITE_RETRY_MAX: u8 = 3;
const TCP_WRITE_RETRY_BACKOFF_SECS: u64 = 1;
const TCP_LONG_MODE_RESET_THRESHOLD: u8 = 3;
const TCP_SHORT_MODE_HOLD_SENDS: u32 = 20;
const MODEM_PROBE_TIMEOUT_MS: u64 = 3_000;
const MODEM_BAUD_SETTLE_MS: u64 = 200;
const MODEM_DRAIN_TIMEOUT_MS: u64 = 100;
const MODEM_ATTACH_TIMEOUT_MS: u64 = 10_000;
const MODEM_NETWORK_READY_TIMEOUT_SECS: u64 = 60;
const MODEM_NETWORK_POLL_INTERVAL_SECS: u64 = 3;
const UART_STASH_SIZE: usize = 256;
const MODEM_BOOT_PROBE_START_DELAY_MS: u64 = 5_000;
const MODEM_BOOT_WAIT_MAX_MS: u64 = 20_000;
const PPP_UP_TIMEOUT_SECS: u64 = 90;
const PPP_WAIT_PROGRESS_INTERVAL_SECS: u64 = 5;
const PPP_RX_CONTROL_TIMEOUT_SECS: u64 = 12;
const MODEM_FIXED_BAUD: u32 = 921_600;
const UART_RX_LINE_LOG_BUFFER: usize = 160;
const PPP_PARSE_BUFFER: usize = 192;
const PPP_CTRL_LOG_SAMPLES_MAX: u8 = 24;

pub enum RunOutcome {
    RetryPowerCycle,
}

static NET_RESOURCES: StaticCell<StackResources<SOCKETS>> = StaticCell::new();
static PPP_STATE: StaticCell<PppState<PPP_RX_QUEUE, PPP_TX_QUEUE>> = StaticCell::new();
static TCP_RX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
static TCP_TX_BUF: StaticCell<[u8; 512]> = StaticCell::new();

static mut NET_RESOURCES_PTR: Option<*mut StackResources<SOCKETS>> = None;
static mut PPP_STATE_PTR: Option<*mut PppState<PPP_RX_QUEUE, PPP_TX_QUEUE>> = None;
static mut TCP_RX_BUF_PTR: Option<*mut [u8; 512]> = None;
static mut TCP_TX_BUF_PTR: Option<*mut [u8; 512]> = None;
static PPP_RX_CONTROL_SEEN: AtomicBool = AtomicBool::new(false);
static PPP_DIAL_BASE_INDEX: AtomicU8 = AtomicU8::new(0);
static PPP_DIAL_ACTIVE_INDEX: AtomicU8 = AtomicU8::new(0);

const MODEM_APN: Option<&str> = option_env!("CONFIG_MODEM_APN");
const MODEM_DEFAULT_APN: &str = "CMNET";
const MODEM_DIAL: &str = match option_env!("CONFIG_MODEM_DIAL") {
    Some(v) => v,
    None => "ATD*99***1#",
};
const SOCKET_HOST: Option<&str> = option_env!("CONFIG_TBOX_SOCKET_HOST");
const SOCKET_PORT: Option<&str> = option_env!("CONFIG_TBOX_SOCKET_PORT");
const PPP_USERNAME: Option<&str> = option_env!("CONFIG_PPP_USERNAME");
const PPP_PASSWORD: Option<&str> = option_env!("CONFIG_PPP_PASSWORD");

const DIAL_CANDIDATE_COUNT: u8 = 3;

fn dial_candidate_by_index(index: u8) -> &'static str {
    match index % DIAL_CANDIDATE_COUNT {
        // 0 => "AT+CGDATA=\"PPP\",1",
        // 1 => MODEM_DIAL,
        0 => "ATD*99#",
        1 => "ATD*99***1#",
        _ => "ATD*98*1#",
    }
}

fn advance_dial_strategy(reason: &str) {
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

pub struct DmaUart<'d> {
    inner: EmbassyUart<'d, Async>,
    stash: [u8; UART_STASH_SIZE],
    stash_len: usize,
    stash_pos: usize,
    log_io: bool,
    rx_line: String<UART_RX_LINE_LOG_BUFFER>,
    ppp_tx_started: bool,
    ppp_rx_started: bool,
    ppp_tx_log_samples: u8,
    ppp_rx_log_samples: u8,
    ppp_ctrl_log_samples: u8,
    ppp_phase: PppPhase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PppPhase {
    Idle,
    Lcp,
    Auth,
    Ipcp,
}

impl<'d> DmaUart<'d> {
    pub fn new(inner: EmbassyUart<'d, Async>) -> Self {
        Self {
            inner,
            stash: [0; UART_STASH_SIZE],
            stash_len: 0,
            stash_pos: 0,
            log_io: true,
            rx_line: String::new(),
            ppp_tx_started: false,
            ppp_rx_started: false,
            ppp_tx_log_samples: 0,
            ppp_rx_log_samples: 0,
            ppp_ctrl_log_samples: 0,
            ppp_phase: PppPhase::Idle,
        }
    }

    pub fn set_io_logging(&mut self, enabled: bool) {
        if !enabled {
            self.flush_rx_line();
            self.ppp_tx_started = false;
            self.ppp_rx_started = false;
            self.ppp_tx_log_samples = 0;
            self.ppp_rx_log_samples = 0;
            self.ppp_ctrl_log_samples = 0;
            self.ppp_phase = PppPhase::Lcp;
            PPP_RX_CONTROL_SEEN.store(false, Ordering::Relaxed);
            info!("PPP phase: LCP negotiation started");
        }
        self.log_io = enabled;
    }

    fn set_ppp_phase(&mut self, phase: PppPhase) {
        if self.ppp_phase == phase {
            return;
        }
        self.ppp_phase = phase;
        match phase {
            PppPhase::Idle => {}
            PppPhase::Lcp => info!("PPP phase: LCP negotiation"),
            PppPhase::Auth => info!("PPP phase: AUTH negotiation"),
            PppPhase::Ipcp => info!("PPP phase: IPCP negotiation"),
        }
    }

    fn contains_protocol(buf: &[u8], proto: &[u8; 2]) -> bool {
        buf.windows(2).any(|w| w == proto)
            || buf.windows(4)
                .any(|w| w == [0x7d, proto[0] ^ 0x20, 0x7d, proto[1] ^ 0x20])
    }

    fn observe_ppp_phase_from_bytes(&mut self, buf: &[u8]) {
        if Self::contains_protocol(buf, &[0xC0, 0x21]) {
            self.set_ppp_phase(PppPhase::Lcp);
            return;
        }

        if Self::contains_protocol(buf, &[0xC0, 0x23]) || Self::contains_protocol(buf, &[0xC2, 0x23]) {
            self.set_ppp_phase(PppPhase::Auth);
            return;
        }

        if Self::contains_protocol(buf, &[0x80, 0x21]) {
            self.set_ppp_phase(PppPhase::Ipcp);
        }
    }

    fn decode_ppp_bytes(input: &[u8], out: &mut [u8]) -> usize {
        let mut i = 0usize;
        let mut used = 0usize;

        while i < input.len() && used < out.len() {
            let b = input[i];
            i += 1;

            if b == 0x7E {
                continue;
            }

            if b == 0x7D {
                if i >= input.len() {
                    break;
                }
                let esc = input[i] ^ 0x20;
                i += 1;
                out[used] = esc;
                used += 1;
                continue;
            }

            out[used] = b;
            used += 1;
        }

        used
    }

    fn ppp_code_name(protocol: [u8; 2], code: u8) -> &'static str {
        match protocol {
            [0xC0, 0x21] | [0x80, 0x21] => match code {
                1 => "Configure-Request",
                2 => "Configure-Ack",
                3 => "Configure-Nak",
                4 => "Configure-Reject",
                5 => "Terminate-Request",
                6 => "Terminate-Ack",
                7 => "Code-Reject",
                8 => "Protocol-Reject",
                9 => "Echo-Request",
                10 => "Echo-Reply",
                11 => "Discard-Request",
                _ => "Unknown",
            },
            [0xC0, 0x23] => match code {
                1 => "Authenticate-Request",
                2 => "Authenticate-Ack",
                3 => "Authenticate-Nak",
                _ => "Unknown",
            },
            [0xC2, 0x23] => match code {
                1 => "Challenge",
                2 => "Response",
                3 => "Success",
                4 => "Failure",
                _ => "Unknown",
            },
            _ => "Unknown",
        }
    }

    fn ppp_proto_name(protocol: [u8; 2]) -> &'static str {
        match protocol {
            [0xC0, 0x21] => "LCP",
            [0xC0, 0x23] => "PAP",
            [0xC2, 0x23] => "CHAP",
            [0x80, 0x21] => "IPCP",
            _ => "PPP",
        }
    }

    fn observe_ppp_control_from_bytes(&mut self, buf: &[u8], is_rx: bool) {
        if self.ppp_ctrl_log_samples >= PPP_CTRL_LOG_SAMPLES_MAX {
            return;
        }

        let mut decoded = [0u8; PPP_PARSE_BUFFER];
        let used = Self::decode_ppp_bytes(buf, &mut decoded);
        if used < 6 {
            return;
        }

        for i in 0..(used.saturating_sub(5)) {
            let protocol = [decoded[i], decoded[i + 1]];
            let known = matches!(protocol, [0xC0, 0x21] | [0xC0, 0x23] | [0xC2, 0x23] | [0x80, 0x21]);
            if !known {
                continue;
            }

            let code = decoded[i + 2];
            let id = decoded[i + 3];
            let plen = u16::from_be_bytes([decoded[i + 4], decoded[i + 5]]);

            if is_rx {
                PPP_RX_CONTROL_SEEN.store(true, Ordering::Relaxed);
            }

            self.ppp_ctrl_log_samples = self.ppp_ctrl_log_samples.saturating_add(1);
            info!(
                "PPP {} {} {} id={} len={}",
                if is_rx { "RX" } else { "TX" },
                Self::ppp_proto_name(protocol),
                Self::ppp_code_name(protocol, code),
                id,
                plen
            );

            if self.ppp_ctrl_log_samples >= PPP_CTRL_LOG_SAMPLES_MAX {
                return;
            }
        }
    }

    fn log_rx_chunk_lines(&mut self, chunk: &[u8]) {
        for &byte in chunk {
            match byte {
                b'\r' | b'\n' => self.flush_rx_line(),
                0x20..=0x7e => {
                    if self.rx_line.push(byte as char).is_err() {
                        self.flush_rx_line();
                        let _ = self.rx_line.push(byte as char);
                    }
                }
                _ => {
                    if self.rx_line.push('.').is_err() {
                        self.flush_rx_line();
                        let _ = self.rx_line.push('.');
                    }
                }
            }
        }
    }

    fn flush_rx_line(&mut self) {
        if !self.rx_line.is_empty() {
            info!("UART3 RX: \"{}\"", self.rx_line.as_str());
            self.rx_line.clear();
        }
    }

    fn available(&self) -> &[u8] {
        &self.stash[self.stash_pos..self.stash_len]
    }

    fn clear_stash(&mut self) {
        self.stash_len = 0;
        self.stash_pos = 0;
    }

    async fn refill_stash(&mut self) -> Result<(), embedded_io_async_new::ErrorKind> {
        if self.stash_pos < self.stash_len {
            return Ok(());
        }

        self.clear_stash();
        if self.log_io {
            let mut retries = 0u8;
            loop {
                match self.inner.read_until_idle(&mut self.stash).await {
                    Ok(len) => {
                        if len == 0 {
                            Timer::after_millis(2).await;
                            continue;
                        }
                        self.stash_len = len;
                        return Ok(());
                    }
                    Err(_) => {
                        retries = retries.saturating_add(1);
                        if retries == 8 {
                            warn!("UART3 RX transient errors, continuing retries");
                        }
                        Timer::after_millis(5).await;
                    }
                }
            }
        } else {
            let mut retries = 0u8;
            loop {
                match self.inner.read_until_idle(&mut self.stash).await {
                    Ok(len) => {
                        if len == 0 {
                            Timer::after_millis(1).await;
                            continue;
                        }
                        self.stash_len = len;
                        return Ok(());
                    }
                    Err(_) => {
                        retries = retries.saturating_add(1);
                        if retries == 8 {
                            warn!("UART3 RX transient errors in PPP mode, continuing retries");
                        }
                        Timer::after_millis(5).await;
                    }
                }
            }
        }
    }

    pub fn set_baudrate(&mut self, baudrate: u32) -> Result<(), embassy_stm32::usart::ConfigError> {
        self.clear_stash();
        self.inner.set_baudrate(baudrate)
    }
}

impl embedded_io_async_new::ErrorType for DmaUart<'_> {
    type Error = embedded_io_async_new::ErrorKind;
}

impl embedded_io_async_new::Read for DmaUart<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        self.refill_stash().await?;
        let available = self.available();
        let len = available.len().min(buf.len());
        buf[..len].copy_from_slice(&available[..len]);
        self.consume(len);
        if self.log_io {
            self.log_rx_chunk_lines(&buf[..len]);
        } else {
            self.observe_ppp_phase_from_bytes(&buf[..len]);
            self.observe_ppp_control_from_bytes(&buf[..len], true);
            if !self.ppp_rx_started {
                self.ppp_rx_started = true;
                info!("PPP RX data stream detected");
            }
            if self.ppp_rx_log_samples < 4 {
                self.ppp_rx_log_samples += 1;
                log_serial_bytes("UART3 PPP RX", &buf[..len]);
            }
        }
        Ok(len)
    }
}

impl embedded_io_async_new::BufRead for DmaUart<'_> {
    async fn fill_buf(&mut self) -> Result<&[u8], Self::Error> {
        self.refill_stash().await?;
        let should_mark_rx = !self.log_io && !self.ppp_rx_started && !self.available().is_empty();
        if should_mark_rx {
            let preview_len = self.available().len().min(SERIAL_LOG_PREVIEW);
            let mut preview = [0u8; SERIAL_LOG_PREVIEW];
            preview[..preview_len].copy_from_slice(&self.available()[..preview_len]);
            self.observe_ppp_phase_from_bytes(&preview[..preview_len]);
            self.observe_ppp_control_from_bytes(&preview[..preview_len], true);
            self.ppp_rx_started = true;
            info!("PPP RX data stream detected");
            if self.ppp_rx_log_samples < 4 {
                self.ppp_rx_log_samples += 1;
                log_serial_bytes("UART3 PPP RX", &preview[..preview_len]);
            }
        }
        Ok(self.available())
    }

    fn consume(&mut self, amt: usize) {
        self.stash_pos = (self.stash_pos + amt).min(self.stash_len);
        if self.stash_pos >= self.stash_len {
            self.clear_stash();
        }
    }
}

impl embedded_io_async_new::Write for DmaUart<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.log_io {
            log_serial_bytes("UART3 TX", buf);
        } else {
            self.observe_ppp_phase_from_bytes(buf);
            self.observe_ppp_control_from_bytes(buf, false);
            if !self.ppp_tx_started {
                self.ppp_tx_started = true;
                info!("PPP TX data stream started");
            }
            if self.ppp_tx_log_samples < 4 {
                self.ppp_tx_log_samples += 1;
                log_serial_bytes("UART3 PPP TX", buf);
            }
        }
        self.inner
            .write(buf)
            .await
            .map_err(|_| embedded_io_async_new::ErrorKind::Other)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner
            .flush()
            .await
            .map_err(|_| embedded_io_async_new::ErrorKind::Other)
    }
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
                    socket_demo(stack).await;
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

fn tcp_rx_buf() -> &'static mut [u8; 512] {
    #[expect(unsafe_code, reason = "cache StaticCell allocation across modem retries")]
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
    #[expect(unsafe_code, reason = "cache StaticCell allocation across modem retries")]
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

async fn prepare_modem_with_autobaud(
    uart: &mut DmaUart<'_>,
) -> Result<u32, ModemError<embedded_io_async_new::ErrorKind>> {
    let baudrate = MODEM_FIXED_BAUD;

    if uart.set_baudrate(baudrate).is_err() {
        warn!("failed to switch modem UART to fixed {}bps", baudrate);
        return Err(ModemError::UnexpectedResponse);
    }

    info!("probing modem at fixed {}bps", baudrate);
    Timer::after_millis(MODEM_BAUD_SETTLE_MS).await;
    drain_pending_input(uart).await;

    info!(
        "waiting 5000ms before AT boot probe (max boot wait: 20000ms)"
    );
    Timer::after_millis(MODEM_BOOT_PROBE_START_DELAY_MS).await;

    let mut at_ok = false;
    let mut attempt = 0u32;
    let mut remaining_boot_probe_ms =
        MODEM_BOOT_WAIT_MAX_MS.saturating_sub(MODEM_BOOT_PROBE_START_DELAY_MS);

    while remaining_boot_probe_ms > 0 {
        attempt += 1;
        let probe_timeout_ms = remaining_boot_probe_ms.min(MODEM_PROBE_TIMEOUT_MS);

        info!(
            "AT probe attempt #{} at fixed {}bps (budget {}ms)",
            attempt,
            baudrate,
            remaining_boot_probe_ms
        );

        match with_timeout(
            Duration::from_millis(probe_timeout_ms),
            send_expect(uart, "AT", "OK"),
        )
        .await
        {
            Ok(Ok(())) => {
                at_ok = true;
                break;
            }
            Ok(Err(_)) => {
                warn!("no AT response on attempt #{} at fixed {}bps", attempt, baudrate);
            }
            Err(_) => {
                warn!("AT probe timed out on attempt #{} at fixed {}bps", attempt, baudrate);
            }
        }

        remaining_boot_probe_ms = remaining_boot_probe_ms.saturating_sub(probe_timeout_ms);
        if remaining_boot_probe_ms == 0 {
            break;
        }

        let settle_ms = remaining_boot_probe_ms.min(MODEM_BAUD_SETTLE_MS);
        Timer::after_millis(settle_ms).await;
        remaining_boot_probe_ms = remaining_boot_probe_ms.saturating_sub(settle_ms);
        drain_pending_input(uart).await;
    }

    if !at_ok {
        warn!("no usable AT response at fixed {}bps", baudrate);
        return Err(ModemError::UnexpectedResponse);
    }

    info!("modem responded at fixed {}bps", baudrate);

    match with_timeout(
        Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
        send_expect(uart, "AT+IPR=921600;&W", "OK"),
    )
    .await
    {
        Ok(Ok(())) => {
            info!("modem UART fixed rate set to 921600bps");
        }
        Ok(Err(_)) => {
            warn!("AT+IPR=921600 failed, continuing with current link");
        }
        Err(_) => {
            warn!("AT+IPR=921600 timed out, continuing with current link");
        }
    }

    match with_timeout(
        Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
        send_expect(uart, "ATE0", "OK"),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(_) => {
            warn!("ATE0 timed out at fixed {}bps", baudrate);
            return Err(ModemError::UnexpectedResponse);
        }
    }

    match with_timeout(
        Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
        send_expect(uart, "AT+IFC=0,0", "OK"),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(_) => {
            warn!("AT+IFC timed out at fixed {}bps", baudrate);
            return Err(ModemError::UnexpectedResponse);
        }
    }

    {
        let apn = MODEM_APN.unwrap_or(MODEM_DEFAULT_APN);
        let mut cmd = [0u8; 96];
        let len = match write_apn_command(&mut cmd, apn) {
            Ok(len) => len,
            Err(ModemError::LineTooLong) => return Err(ModemError::LineTooLong),
            Err(ModemError::Io(_))
            | Err(ModemError::Eof)
            | Err(ModemError::UnexpectedResponse)
            | Err(ModemError::Utf8) => unreachable!(),
        };

        match with_timeout(
            Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
            send_expect_bytes(uart, &cmd[..len], "OK"),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                warn!("APN setup timed out at fixed {}bps", baudrate);
                return Err(ModemError::UnexpectedResponse);
            }
        }
    }

    if PPP_USERNAME.is_some() || PPP_PASSWORD.is_some() {
        let user = PPP_USERNAME.unwrap_or("");
        let pass = PPP_PASSWORD.unwrap_or("");
        let mut cmd = [0u8; 128];
        let len = match write_cgauth_command(&mut cmd, 1, user, pass) {
            Ok(len) => len,
            Err(ModemError::LineTooLong) => return Err(ModemError::LineTooLong),
            Err(ModemError::Io(_))
            | Err(ModemError::Eof)
            | Err(ModemError::UnexpectedResponse)
            | Err(ModemError::Utf8) => unreachable!(),
        };

        match with_timeout(
            Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
            send_expect_bytes(uart, &cmd[..len], "OK"),
        )
        .await
        {
            Ok(Ok(())) => {
                info!("APN auth configured via AT+CGAUTH (username/password from env)");
            }
            Ok(Err(_)) => {
                warn!("AT+CGAUTH returned non-OK, continuing without explicit APN auth");
            }
            Err(_) => {
                warn!("AT+CGAUTH timed out, continuing without explicit APN auth");
            }
        }
    }

    {
        let apn = MODEM_APN.unwrap_or(MODEM_DEFAULT_APN);
        let user = PPP_USERNAME.unwrap_or("");
        let pass = PPP_PASSWORD.unwrap_or("");
        let auth = if user.is_empty() && pass.is_empty() { 0 } else { 1 };
        let mut cmd = [0u8; 160];
        let len = match write_qicsgp_command(&mut cmd, 1, apn, user, pass, auth) {
            Ok(len) => len,
            Err(ModemError::LineTooLong) => return Err(ModemError::LineTooLong),
            Err(ModemError::Io(_))
            | Err(ModemError::Eof)
            | Err(ModemError::UnexpectedResponse)
            | Err(ModemError::Utf8) => unreachable!(),
        };

        match with_timeout(
            Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
            send_expect_bytes(uart, &cmd[..len], "OK"),
        )
        .await
        {
            Ok(Ok(())) => {
                info!(
                    "Quectel PDP profile configured via AT+QICSGP (cid=1, auth={})",
                    auth
                );
            }
            Ok(Err(_)) => {
                warn!("AT+QICSGP returned non-OK, continuing with CGDCONT/CGAUTH path");
            }
            Err(_) => {
                warn!("AT+QICSGP timed out, continuing with CGDCONT/CGAUTH path");
            }
        }
    }

    run_optional_at_diag(uart, "AT+CPIN?").await;
    run_optional_at_diag(uart, "AT+CSQ").await;
    run_optional_at_diag(uart, "AT+COPS?").await;
    run_optional_at_diag(uart, "AT+CREG?").await;
    run_optional_at_diag(uart, "AT+CGREG?").await;
    run_optional_at_diag(uart, "AT+CEREG?").await;
    run_optional_at_diag(uart, "AT+CGATT?").await;
    run_optional_at_diag(uart, "AT+CGACT?").await;
    run_optional_at_diag(uart, "AT+CGPADDR=1").await;

    if !wait_for_network_ready(uart).await {
        warn!(
            "cellular network not ready within {}s, skip PPP dial this cycle",
            MODEM_NETWORK_READY_TIMEOUT_SECS
        );
        return Err(ModemError::UnexpectedResponse);
    }

    match with_timeout(
        Duration::from_millis(MODEM_ATTACH_TIMEOUT_MS),
        send_expect(uart, "AT+CGATT=1", "OK"),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            warn!("AT+CGATT returned non-OK, still trying PPP dial");
        }
        Err(_) => {
            warn!("AT+CGATT timed out, still trying PPP dial");
        }
    }

    match with_timeout(
        Duration::from_millis(MODEM_ATTACH_TIMEOUT_MS),
        send_expect(uart, "AT+CGACT=1,1", "OK"),
    )
    .await
    {
        Ok(Ok(())) => {
            info!("PDP context activated: CID 1");
        }
        Ok(Err(_)) => {
            warn!("AT+CGACT=1,1 returned non-OK, still trying PPP dial");
        }
        Err(_) => {
            warn!("AT+CGACT=1,1 timed out, still trying PPP dial");
        }
    }

    run_optional_at_diag(uart, "AT+CGACT?").await;
    run_optional_at_diag(uart, "AT+CGPADDR=1").await;

    let start = PPP_DIAL_BASE_INDEX.load(Ordering::Relaxed) % DIAL_CANDIDATE_COUNT;
    info!(
        "PPP dial strategy start index={} command=\"{}\"",
        start,
        dial_candidate_by_index(start)
    );

    let mut attempted: [Option<&'static str>; DIAL_CANDIDATE_COUNT as usize] =
        [None; DIAL_CANDIDATE_COUNT as usize];
    let mut attempted_count = 0usize;

    for offset in 0..DIAL_CANDIDATE_COUNT {
        let dial_index = (start + offset) % DIAL_CANDIDATE_COUNT;
        let dial = dial_candidate_by_index(dial_index);

        if attempted
            .iter()
            .take(attempted_count)
            .any(|v| v.is_some_and(|existing| existing == dial))
        {
            info!("skipping duplicate PPP dial command #{}: {}", dial_index, dial);
            continue;
        }

        attempted[attempted_count] = Some(dial);
        attempted_count += 1;

        info!("trying PPP dial command #{}: {}", dial_index, dial);
        match with_timeout(
            Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
            send_expect(uart, dial, "CONNECT"),
        )
        .await
        {
            Ok(Ok(())) => {
                PPP_DIAL_ACTIVE_INDEX.store(dial_index, Ordering::Relaxed);
                return Ok(baudrate);
            }
            Ok(Err(_)) => {
                warn!("PPP dial command failed: {}", dial);
            }
            Err(_) => {
                warn!("PPP dial timed out for command: {}", dial);
            }
        }
    }

    advance_dial_strategy("all PPP dial commands failed before CONNECT");

    Err(ModemError::UnexpectedResponse)
}

async fn run_optional_at_diag(uart: &mut DmaUart<'_>, cmd: &str) {
    info!("PPP precheck cmd: {}", cmd);
    match with_timeout(
        Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
        send_expect(uart, cmd, "OK"),
    )
    .await
    {
        Ok(Ok(())) => info!("PPP precheck ok: {}", cmd),
        Ok(Err(_)) => warn!("PPP precheck failed: {}", cmd),
        Err(_) => warn!("PPP precheck timeout: {}", cmd),
    }
}

async fn wait_for_network_ready(uart: &mut DmaUart<'_>) -> bool {
    let mut waited = 0u64;

    loop {
        let csq = query_value(uart, "AT+CSQ", "+CSQ:").await;
        let creg = query_value(uart, "AT+CREG?", "+CREG:").await;
        let cgreg = query_value(uart, "AT+CGREG?", "+CGREG:").await;
        let cereg = query_value(uart, "AT+CEREG?", "+CEREG:").await;
        let cgatt = query_value(uart, "AT+CGATT?", "+CGATT:").await;

        let csq_rssi = csq.as_deref().and_then(parse_csq_rssi);
        let creg_stat = creg.as_deref().and_then(parse_reg_stat);
        let cgreg_stat = cgreg.as_deref().and_then(parse_reg_stat);
        let cereg_stat = cereg.as_deref().and_then(parse_reg_stat);
        let cgatt_stat = cgatt.as_deref().and_then(parse_single_u8);

        info!(
            "net ready poll {}s/{}s: CSQ={:?} CREG={:?} CGREG={:?} CEREG={:?} CGATT={:?}",
            waited,
            MODEM_NETWORK_READY_TIMEOUT_SECS,
            Debug2Format(&csq_rssi),
            Debug2Format(&creg_stat),
            Debug2Format(&cgreg_stat),
            Debug2Format(&cereg_stat),
            Debug2Format(&cgatt_stat)
        );

        let signal_ok = csq_rssi.map(|v| v != 99).unwrap_or(false);
        let reg_ok = is_registered(creg_stat) || is_registered(cgreg_stat) || is_registered(cereg_stat);
        let attach_ok = matches!(cgatt_stat, Some(1));

        if signal_ok && reg_ok && attach_ok {
            info!("cellular network is ready for PPP dial");
            return true;
        }

        if waited >= MODEM_NETWORK_READY_TIMEOUT_SECS {
            return false;
        }

        Timer::after_secs(MODEM_NETWORK_POLL_INTERVAL_SECS).await;
        waited = (waited + MODEM_NETWORK_POLL_INTERVAL_SECS).min(MODEM_NETWORK_READY_TIMEOUT_SECS);
    }
}

async fn query_value(uart: &mut DmaUart<'_>, cmd: &str, prefix: &str) -> Option<String<48>> {
    match query_value_inner(uart, cmd, prefix).await {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

async fn query_value_inner(
    uart: &mut DmaUart<'_>,
    cmd: &str,
    prefix: &str,
) -> Result<String<48>, ModemError<embedded_io_async_new::ErrorKind>> {
    let mut value: String<48> = String::new();

    send_at_parse(uart, cmd.as_bytes(), AtSuccess::Ok, |text| {
        if text.starts_with(prefix) {
            let raw = text[prefix.len()..].trim();
            value.clear();
            let _ = value.push_str(raw);
        }
        Ok(())
    })
    .await?;

    Ok(value)
}

fn parse_csq_rssi(value: &str) -> Option<u8> {
    let first = value.split(',').next()?.trim();
    first.parse::<u8>().ok()
}

fn parse_reg_stat(value: &str) -> Option<u8> {
    value.split(',').last()?.trim().parse::<u8>().ok()
}

fn parse_single_u8(value: &str) -> Option<u8> {
    value.trim().parse::<u8>().ok()
}

fn is_registered(stat: Option<u8>) -> bool {
    matches!(stat, Some(1) | Some(5))
}

async fn drain_pending_input(uart: &mut DmaUart<'_>) {
    let mut byte = [0u8; 1];

    loop {
        match with_timeout(Duration::from_millis(MODEM_DRAIN_TIMEOUT_MS), uart.read(&mut byte)).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
            Ok(Ok(_)) => {
                info!("discarding stale UART byte before AT probe");
            }
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

async fn socket_demo(stack: Stack<'static>) {
    let host = SOCKET_HOST.unwrap_or(DEFAULT_SOCKET_HOST);

    let Ok(host_addr) = Ipv4Address::from_str(host) else {
        warn!("invalid CONFIG_TBOX_SOCKET_HOST: {}", host);
        idle_forever().await
    };

    let port = SOCKET_PORT
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_SOCKET_PORT);
    let [ha, hb, hc, hd] = host_addr.octets();

    info!("server target: {}:{}", host, port);
    info!("server target parsed as {}.{}.{}.{}:{}", ha, hb, hc, hd, port);

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
        socket.set_timeout(Some(ariel_os::time::Duration::from_secs(TCP_CONNECT_TIMEOUT_SECS)));

        info!(
            "TCP connect attempt #{}: starting handshake to {}:{} over PPP",
            connect_attempt,
            host,
            port,
        );
        match socket.connect((host_addr, port)).await {
            Ok(()) => {
                info!(
                    "TCP connect attempt #{} succeeded: connected to {}:{}",
                    connect_attempt,
                    host,
                    port,
                );
                socket.set_timeout(Some(ariel_os::time::Duration::from_secs(TCP_IO_TIMEOUT_SECS)));

                if short_mode_remaining > 0 {
                    info!(
                        "TCP adaptive mode: short-connection fallback active (remaining sends: {})",
                        short_mode_remaining
                    );
                } else {
                    info!("TCP adaptive mode: long-connection preferred");
                }

                let mut sent_this_connection: u32 = 0;
                let mut closed_by_reset = false;
                let mut closed_by_send_failure = false;

                loop {
                    let current_id = message_counter.saturating_add(1);

                    let mut payload: String<96> = String::new();
                    let _ = writeln!(&mut payload, "hello word {}", current_id);

                    info!(
                        "TCP send #{}: sending {} bytes to {}:{}",
                        current_id,
                        payload.len(),
                        host,
                        port,
                    );
                    log_serial_bytes("UART3 PPP TX", payload.as_bytes());

                    let mut write_ok = false;
                    for write_try in 1..=TCP_WRITE_RETRY_MAX {
                        match socket.write_all(payload.as_bytes()).await {
                            Ok(()) => {
                                write_ok = true;
                                if write_try > 1 {
                                    info!(
                                        "TCP send #{} succeeded on retry {}/{}",
                                        current_id,
                                        write_try,
                                        TCP_WRITE_RETRY_MAX
                                    );
                                }
                                break;
                            }
                            Err(err) => {
                                warn!(
                                    "TCP send #{} write attempt {}/{} failed: {:?}",
                                    current_id,
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

                    if !write_ok {
                        warn!(
                            "TCP send #{} failed before commit (last committed #{}), reconnecting",
                            current_id,
                            message_counter,
                        );
                        closed_by_send_failure = true;
                        break;
                    }

                    message_counter = current_id;
                    sent_this_connection = sent_this_connection.saturating_add(1);

                    info!("TCP send #{} completed", current_id);

                    if short_mode_remaining > 0 {
                        short_mode_remaining = short_mode_remaining.saturating_sub(1);
                        info!(
                            "TCP short-mode: closing connection after send #{} (remaining sends: {})",
                            current_id,
                            short_mode_remaining
                        );
                        break;
                    }

                    match with_timeout(
                        Duration::from_millis(TCP_READ_PROBE_TIMEOUT_MS),
                        socket.read(&mut socket_read_buf),
                    )
                    .await
                    {
                        Ok(Ok(0)) => {
                            warn!("TCP peer closed connection after send #{}", current_id);
                            closed_by_reset = true;
                            break;
                        }
                        Ok(Ok(len)) => {
                            info!("TCP receive after send #{}: {} bytes", current_id, len);
                            log_serial_bytes("UART3 PPP RX", &socket_read_buf[..len]);
                        }
                        Ok(Err(err)) => {
                            let mut err_text: String<64> = String::new();
                            let _ = write!(&mut err_text, "{:?}", Debug2Format(&err));

                            if err_text.contains("ConnectionReset")
                                || err_text.contains("ConnectionAborted")
                                || err_text.contains("ConnectionClosed")
                            {
                                warn!(
                                    "TCP connection lost after send #{} ({}), reconnecting",
                                    current_id,
                                    err_text.as_str()
                                );
                                closed_by_reset = true;
                                break;
                            }
                        }
                        Err(_) => {
                            // No incoming data within probe window; continue periodic send loop.
                        }
                    }

                    Timer::after_millis(TCP_SEND_INTERVAL_MS).await;
                }

                if sent_this_connection <= 1 && (closed_by_reset || closed_by_send_failure) {
                    early_reset_streak = early_reset_streak.saturating_add(1);
                    warn!(
                        "TCP early-close streak: {}/{}",
                        early_reset_streak,
                        TCP_LONG_MODE_RESET_THRESHOLD
                    );
                } else if sent_this_connection >= 2 {
                    early_reset_streak = 0;
                }

                if short_mode_remaining == 0 && early_reset_streak >= TCP_LONG_MODE_RESET_THRESHOLD {
                    short_mode_remaining = TCP_SHORT_MODE_HOLD_SENDS;
                    early_reset_streak = 0;
                    warn!(
                        "TCP switching to short-connection fallback for next {} sends (long mode remains preferred and will auto-resume)",
                        TCP_SHORT_MODE_HOLD_SENDS
                    );
                }

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

        Timer::after_secs(5).await;
    }
}

fn write_apn_command(buf: &mut [u8], apn: &str) -> Result<usize, ModemError<core::convert::Infallible>> {
    let prefix = b"AT+CGDCONT=1,\"IP\",\"";
    let suffix = b"\"";
    let required = prefix.len() + apn.len() + suffix.len();

    if buf.len() < required {
        return Err(ModemError::LineTooLong);
    }

    let mut index = 0;
    buf[index..index + prefix.len()].copy_from_slice(prefix);
    index += prefix.len();
    buf[index..index + apn.len()].copy_from_slice(apn.as_bytes());
    index += apn.len();
    buf[index..index + suffix.len()].copy_from_slice(suffix);
    index += suffix.len();
    Ok(index)
}

async fn send_expect<RW>(uart: &mut RW, command: &str, expected: &str) -> Result<(), ModemError<RW::Error>>
where
    RW: Read + Write,
{
    send_expect_bytes(uart, command.as_bytes(), expected).await
}

async fn send_expect_bytes<RW>(uart: &mut RW, command: &[u8], expected: &str) -> Result<(), ModemError<RW::Error>>
where
    RW: Read + Write,
{
    send_at_parse(uart, command, AtSuccess::Exact(expected), |_| Ok(())).await
}

enum AtSuccess<'a> {
    Ok,
    Exact(&'a str),
}

fn is_at_error_line(text: &str) -> bool {
    text == "ERROR" || text.starts_with("+CME ERROR") || text.starts_with("+CMS ERROR")
}

async fn send_at_parse<RW, F>(
    uart: &mut RW,
    command: &[u8],
    success: AtSuccess<'_>,
    mut parse_line: F,
) -> Result<(), ModemError<RW::Error>>
where
    RW: Read + Write,
    F: FnMut(&str) -> Result<(), ModemError<RW::Error>>,
{
    uart.write_all(command).await.map_err(ModemError::Io)?;
    uart.write_all(b"\r").await.map_err(ModemError::Io)?;
    uart.flush().await.map_err(ModemError::Io)?;

    let mut line = [0u8; UART_LINE_BUFFER];
    loop {
        let len = read_line(uart, &mut line).await?;
        if len == 0 {
            continue;
        }

        let text = core::str::from_utf8(&line[..len]).map_err(|_| ModemError::Utf8)?;
        parse_line(text)?;

        let matched = match success {
            AtSuccess::Ok => text == "OK",
            AtSuccess::Exact(expected) => text == expected,
        };
        if matched {
            return Ok(());
        }

        if is_at_error_line(text) {
            return Err(ModemError::UnexpectedResponse);
        }
    }
}

async fn read_line<RW>(uart: &mut RW, out: &mut [u8]) -> Result<usize, ModemError<RW::Error>>
where
    RW: Read + Write,
{
    let mut used = 0;
    let mut saw_cr = false;

    loop {
        let byte = read_byte(uart).await?;
        match byte {
            b'\n' => return Ok(used),
            b'\r' => {
                if used > 0 || saw_cr {
                    return Ok(used);
                }
                saw_cr = true;
            }
            _ => {
                saw_cr = false;
                if used >= out.len() {
                    return Err(ModemError::LineTooLong);
                }
                out[used] = byte;
                used += 1;
            }
        }
    }
}

async fn read_byte<RW>(uart: &mut RW) -> Result<u8, ModemError<RW::Error>>
where
    RW: Read + Write,
{
    let mut byte = [0u8; 1];

    loop {
        match uart.read(&mut byte).await.map_err(ModemError::Io)? {
            0 => return Err(ModemError::Eof),
            _ => return Ok(byte[0]),
        }
    }
}

async fn idle_forever() -> ! {
    loop {
        Timer::after_secs(60).await;
    }
}

fn log_serial_bytes(prefix: &str, bytes: &[u8]) {
    let preview_len = bytes.len().min(SERIAL_LOG_PREVIEW);

    let mut ascii: String<96> = String::new();
    for &byte in &bytes[..preview_len] {
        let ch = match byte {
            0x20..=0x7E => byte as char,
            b'\r' => '␍',
            b'\n' => '␊',
            _ => '.',
        };
        let _ = ascii.push(ch);
    }

    if bytes.len() > preview_len {
        info!("{}: \"{}...\"", prefix, ascii.as_str());
    } else {
        info!("{}: \"{}\"", prefix, ascii.as_str());
    }

    if prefix.contains("PPP") {
        let mut hex: String<192> = String::new();
        for (i, &byte) in bytes[..preview_len].iter().enumerate() {
            if i > 0 {
                let _ = hex.push(' ');
            }
            let _ = write!(hex, "{:02X}", byte);
        }

        if bytes.len() > preview_len {
            info!("{} HEX: {} ...", prefix, hex.as_str());
        } else {
            info!("{} HEX: {}", prefix, hex.as_str());
        }
    }
}

enum ModemError<E> {
    Io(E),
    Eof,
    LineTooLong,
    UnexpectedResponse,
    Utf8,
}

impl<E: fmt::Debug> fmt::Debug for ModemError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => f.debug_tuple("Io").field(err).finish(),
            Self::Eof => f.write_str("Eof"),
            Self::LineTooLong => f.write_str("LineTooLong"),
            Self::UnexpectedResponse => f.write_str("UnexpectedResponse"),
            Self::Utf8 => f.write_str("Utf8"),
        }
    }
}

fn write_cgauth_command(
    buf: &mut [u8],
    cid: u8,
    username: &str,
    password: &str,
) -> Result<usize, ModemError<core::convert::Infallible>> {
    let mut index = 0usize;

    let prefix = b"AT+CGAUTH=";
    if buf.len() < prefix.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + prefix.len()].copy_from_slice(prefix);
    index += prefix.len();

    let cid_str = match cid {
        0..=9 => [b'0' + cid],
        _ => return Err(ModemError::LineTooLong),
    };
    if buf.len() < index + cid_str.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + cid_str.len()].copy_from_slice(&cid_str);
    index += cid_str.len();

    let middle = b",1,\"";
    if buf.len() < index + middle.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + middle.len()].copy_from_slice(middle);
    index += middle.len();

    if buf.len() < index + username.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + username.len()].copy_from_slice(username.as_bytes());
    index += username.len();

    let sep = b"\",\"";
    if buf.len() < index + sep.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + sep.len()].copy_from_slice(sep);
    index += sep.len();

    if buf.len() < index + password.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + password.len()].copy_from_slice(password.as_bytes());
    index += password.len();

    let suffix = b"\"";
    if buf.len() < index + suffix.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + suffix.len()].copy_from_slice(suffix);
    index += suffix.len();

    Ok(index)
}

fn write_qicsgp_command(
    buf: &mut [u8],
    cid: u8,
    apn: &str,
    username: &str,
    password: &str,
    auth: u8,
) -> Result<usize, ModemError<core::convert::Infallible>> {
    if cid > 9 || auth > 3 {
        return Err(ModemError::LineTooLong);
    }

    let mut index = 0usize;

    let prefix = b"AT+QICSGP=";
    if buf.len() < index + prefix.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + prefix.len()].copy_from_slice(prefix);
    index += prefix.len();

    let head = [b'0' + cid, b',', b'1', b',', b'\"'];
    if buf.len() < index + head.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + head.len()].copy_from_slice(&head);
    index += head.len();

    if buf.len() < index + apn.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + apn.len()].copy_from_slice(apn.as_bytes());
    index += apn.len();

    let sep1 = b"\",\"";
    if buf.len() < index + sep1.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + sep1.len()].copy_from_slice(sep1);
    index += sep1.len();

    if buf.len() < index + username.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + username.len()].copy_from_slice(username.as_bytes());
    index += username.len();

    let sep2 = b"\",\"";
    if buf.len() < index + sep2.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + sep2.len()].copy_from_slice(sep2);
    index += sep2.len();

    if buf.len() < index + password.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + password.len()].copy_from_slice(password.as_bytes());
    index += password.len();

    let tail = [b'\"', b',', b'0' + auth];
    if buf.len() < index + tail.len() {
        return Err(ModemError::LineTooLong);
    }
    buf[index..index + tail.len()].copy_from_slice(&tail);
    index += tail.len();

    Ok(index)
}