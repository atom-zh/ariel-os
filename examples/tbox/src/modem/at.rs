use core::fmt;
use core::sync::atomic::Ordering;

use ariel_os::{
    log::Debug2Format,
    time::{Duration, Timer, with_timeout},
};
use embedded_io_async_07 as embedded_io_async_new;
use embedded_io_async_new::{Read, Write};
use heapless::String;

use super::config::{
    MODEM_APN, MODEM_ATTACH_TIMEOUT_MS, MODEM_BAUD_SETTLE_MS, MODEM_BOOT_PROBE_START_DELAY_MS,
    MODEM_BOOT_WAIT_MAX_MS, MODEM_DEFAULT_APN, MODEM_DRAIN_TIMEOUT_MS, MODEM_FIXED_BAUD,
    MODEM_NETWORK_POLL_INTERVAL_SECS, MODEM_NETWORK_READY_TIMEOUT_SECS, MODEM_PROBE_TIMEOUT_MS,
    PPP_PASSWORD, PPP_USERNAME, UART_LINE_BUFFER,
};
use super::ppp::{
    DIAL_CANDIDATE_COUNT, PPP_DIAL_ACTIVE_INDEX, PPP_DIAL_BASE_INDEX, advance_dial_strategy,
    dial_candidate_by_index,
};
use super::uart::DmaUart;
use super::set_device_id_from_imei;

pub(super) async fn prepare_modem_with_autobaud(
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

    info!("waiting 5000ms before AT boot probe (max boot wait: 20000ms)");
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
            attempt, baudrate, remaining_boot_probe_ms
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
            Ok(Err(_)) => warn!(
                "no AT response on attempt #{} at fixed {}bps",
                attempt, baudrate
            ),
            Err(_) => warn!(
                "AT probe timed out on attempt #{} at fixed {}bps",
                attempt, baudrate
            ),
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
        Ok(Ok(())) => info!("modem UART fixed rate set to 921600bps"),
        Ok(Err(_)) => warn!("AT+IPR=921600 failed, continuing with current link"),
        Err(_) => warn!("AT+IPR=921600 timed out, continuing with current link"),
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
            Ok(Ok(())) => info!("APN auth configured via AT+CGAUTH (username/password from env)"),
            Ok(Err(_)) => warn!("AT+CGAUTH returned non-OK, continuing without explicit APN auth"),
            Err(_) => warn!("AT+CGAUTH timed out, continuing without explicit APN auth"),
        }
    }

    {
        let apn = MODEM_APN.unwrap_or(MODEM_DEFAULT_APN);
        let user = PPP_USERNAME.unwrap_or("");
        let pass = PPP_PASSWORD.unwrap_or("");
        let auth = if user.is_empty() && pass.is_empty() {
            0
        } else {
            1
        };
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
            Ok(Ok(())) => info!(
                "Quectel PDP profile configured via AT+QICSGP (cid=1, auth={})",
                auth
            ),
            Ok(Err(_)) => warn!("AT+QICSGP returned non-OK, continuing with CGDCONT/CGAUTH path"),
            Err(_) => warn!("AT+QICSGP timed out, continuing with CGDCONT/CGAUTH path"),
        }
    }

    query_and_log_modem_identity(uart).await;

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
        Ok(Err(_)) => warn!("AT+CGATT returned non-OK, still trying PPP dial"),
        Err(_) => warn!("AT+CGATT timed out, still trying PPP dial"),
    }

    match with_timeout(
        Duration::from_millis(MODEM_ATTACH_TIMEOUT_MS),
        send_expect(uart, "AT+CGACT=1,1", "OK"),
    )
    .await
    {
        Ok(Ok(())) => info!("PDP context activated: CID 1"),
        Ok(Err(_)) => warn!("AT+CGACT=1,1 returned non-OK, still trying PPP dial"),
        Err(_) => warn!("AT+CGACT=1,1 timed out, still trying PPP dial"),
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
            info!(
                "skipping duplicate PPP dial command #{}: {}",
                dial_index, dial
            );
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
            Ok(Err(_)) => warn!("PPP dial command failed: {}", dial),
            Err(_) => warn!("PPP dial timed out for command: {}", dial),
        }
    }

    advance_dial_strategy("all PPP dial commands failed before CONNECT");

    Err(ModemError::UnexpectedResponse)
}

async fn query_and_log_modem_identity(uart: &mut DmaUart<'_>) {
    match query_digits_with_fallback(uart, "IMEI", "AT+CGSN", None, "AT+GSN", None).await {
        Some(imei) => {
            info!("modem IMEI: {}", imei.as_str());
            match set_device_id_from_imei(imei.as_str()) {
                Some(device_id) => info!(
                    "remote device identifier set from IMEI last 8 digits: {:08}",
                    device_id
                ),
                None => warn!("failed to derive remote device identifier from IMEI"),
            }
        }
        None => warn!("failed to read modem IMEI before PPP dial"),
    }

    match query_digits_with_fallback(
        uart,
        "ICCID",
        "AT+QCCID",
        Some("+QCCID:"),
        "AT+CCID",
        Some("+CCID:"),
    )
    .await
    {
        Some(iccid) => info!("modem ICCID: {}", iccid.as_str()),
        None => warn!("failed to read modem ICCID before PPP dial"),
    }
}

async fn query_digits_with_fallback(
    uart: &mut DmaUart<'_>,
    name: &str,
    primary_cmd: &str,
    primary_prefix: Option<&str>,
    fallback_cmd: &str,
    fallback_prefix: Option<&str>,
) -> Option<String<32>> {
    match query_digit_identity(uart, primary_cmd, primary_prefix).await {
        Some(value) => Some(value),
        None => {
            warn!(
                "{} query failed with {}, trying {}",
                name, primary_cmd, fallback_cmd
            );
            query_digit_identity(uart, fallback_cmd, fallback_prefix).await
        }
    }
}

async fn query_digit_identity(
    uart: &mut DmaUart<'_>,
    cmd: &str,
    prefix: Option<&str>,
) -> Option<String<32>> {
    match with_timeout(
        Duration::from_millis(MODEM_PROBE_TIMEOUT_MS),
        query_digit_identity_inner(uart, cmd, prefix),
    )
    .await
    {
        Ok(Ok(value)) => Some(value),
        Ok(Err(_)) | Err(_) => None,
    }
}

async fn query_digit_identity_inner(
    uart: &mut DmaUart<'_>,
    cmd: &str,
    prefix: Option<&str>,
) -> Result<String<32>, ModemError<embedded_io_async_new::ErrorKind>> {
    let mut value: String<32> = String::new();

    send_at_parse(uart, cmd.as_bytes(), AtSuccess::Ok, |text| {
        let Some(raw) = identity_payload(text, prefix) else {
            return Ok(());
        };

        let mut candidate: String<32> = String::new();
        copy_ascii_digits(&mut candidate, raw);
        if candidate.len() >= 8 {
            value.clear();
            let _ = value.push_str(candidate.as_str());
        }

        Ok(())
    })
    .await?;

    if value.is_empty() {
        Err(ModemError::UnexpectedResponse)
    } else {
        Ok(value)
    }
}

fn identity_payload<'a>(text: &'a str, prefix: Option<&str>) -> Option<&'a str> {
    if text == "OK" || is_at_error_line(text) {
        return None;
    }

    match prefix {
        Some(prefix) => text.strip_prefix(prefix).map(str::trim),
        None => Some(text.trim()),
    }
}

fn copy_ascii_digits(out: &mut String<32>, text: &str) {
    for byte in text.bytes() {
        if byte.is_ascii_digit() {
            let _ = out.push(byte as char);
        }
    }
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
        let reg_ok =
            is_registered(creg_stat) || is_registered(cgreg_stat) || is_registered(cereg_stat);
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
        match with_timeout(
            Duration::from_millis(MODEM_DRAIN_TIMEOUT_MS),
            uart.read(&mut byte),
        )
        .await
        {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
            Ok(Ok(_)) => info!("discarding stale UART byte before AT probe"),
        }
    }
}

fn write_apn_command(
    buf: &mut [u8],
    apn: &str,
) -> Result<usize, ModemError<core::convert::Infallible>> {
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

async fn send_expect<RW>(
    uart: &mut RW,
    command: &str,
    expected: &str,
) -> Result<(), ModemError<RW::Error>>
where
    RW: Read + Write,
{
    send_expect_bytes(uart, command.as_bytes(), expected).await
}

async fn send_expect_bytes<RW>(
    uart: &mut RW,
    command: &[u8],
    expected: &str,
) -> Result<(), ModemError<RW::Error>>
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

pub(super) enum ModemError<E> {
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
