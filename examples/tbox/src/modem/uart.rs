use core::fmt::Write as _;
use core::sync::atomic::Ordering;

use ariel_os::{
    log::*,
    time::Timer,
};
use embassy_stm32::{mode::Async, usart::Uart as EmbassyUart};
use embedded_io_async_07 as embedded_io_async_new;
use embedded_io_async_new::BufRead;
use heapless::String;

use super::config::{PPP_CTRL_LOG_SAMPLES_MAX, PPP_PARSE_BUFFER, SERIAL_LOG_PREVIEW, UART_RX_LINE_LOG_BUFFER, UART_STASH_SIZE};
use super::state::PPP_RX_CONTROL_SEEN;

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

    pub fn set_baudrate(&mut self, baudrate: u32) -> Result<(), embassy_stm32::usart::ConfigError> {
        self.clear_stash();
        self.inner.set_baudrate(baudrate)
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

pub(crate) fn log_serial_bytes(prefix: &str, bytes: &[u8]) {
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