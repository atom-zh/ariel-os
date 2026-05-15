use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ariel_os::time::Instant;
use critical_section::Mutex;

static DEVICE_ID_FROM_IMEI: AtomicU32 = AtomicU32::new(0);
static DEVICE_ID_FROM_IMEI_SET: AtomicBool = AtomicBool::new(false);
static REPORT_TIME: Mutex<RefCell<Option<ReportTimeBase>>> = Mutex::new(RefCell::new(None));

#[derive(Clone, Copy)]
struct ReportTimeBase {
    epoch_secs: u32,
    tick_millis: u64,
}

pub(crate) fn set_device_id_from_imei(imei: &str) -> Option<u32> {
    let mut last_digits = [0u8; 8];
    let mut digit_count = 0usize;

    for byte in imei.bytes() {
        if !byte.is_ascii_digit() {
            continue;
        }

        if digit_count < last_digits.len() {
            last_digits[digit_count] = byte;
        } else {
            last_digits.copy_within(1.., 0);
            last_digits[last_digits.len() - 1] = byte;
        }
        digit_count += 1;
    }

    if digit_count < last_digits.len() {
        return None;
    }

    let device_id = parse_radix_u32(&last_digits, 10)?;
    DEVICE_ID_FROM_IMEI.store(device_id, Ordering::Relaxed);
    DEVICE_ID_FROM_IMEI_SET.store(true, Ordering::Relaxed);
    Some(device_id)
}

pub(crate) fn device_id_from_imei() -> Option<u32> {
    if DEVICE_ID_FROM_IMEI_SET.load(Ordering::Relaxed) {
        Some(DEVICE_ID_FROM_IMEI.load(Ordering::Relaxed))
    } else {
        None
    }
}

pub(crate) fn set_report_time(time: [u8; 6]) {
    let Some(epoch_secs) = report_time_to_epoch_secs(time) else {
        return;
    };

    critical_section::with(|cs| {
        *REPORT_TIME.borrow(cs).borrow_mut() = Some(ReportTimeBase {
            epoch_secs,
            tick_millis: Instant::now().as_millis(),
        });
    });
}

pub(crate) fn sync_report_time_if_drift_exceeds(time: [u8; 6], max_drift_secs: u32) -> Option<i32> {
    let remote_epoch = report_time_to_epoch_secs(time)?;
    let local_epoch = current_epoch_secs();
    let drift = local_epoch
        .map(|local| remote_epoch as i32 - local as i32)
        .unwrap_or(0);

    if local_epoch.is_none() || drift.unsigned_abs() > max_drift_secs {
        set_report_time(time);
    }

    Some(drift)
}

pub(crate) fn report_time() -> Option<[u8; 6]> {
    epoch_secs_to_report_time(current_epoch_secs()?)
}

fn current_epoch_secs() -> Option<u32> {
    let base = critical_section::with(|cs| *REPORT_TIME.borrow(cs).borrow())?;
    let elapsed_secs = Instant::now()
        .as_millis()
        .saturating_sub(base.tick_millis)
        / 1000;
    Some(base.epoch_secs.saturating_add(elapsed_secs.min(u64::from(u32::MAX)) as u32))
}

fn report_time_to_epoch_secs(time: [u8; 6]) -> Option<u32> {
    let year = 2000u32.checked_add(u32::from(time[0]))?;
    let month = u32::from(time[1]);
    let day = u32::from(time[2]);
    let hour = u32::from(time[3]);
    let minute = u32::from(time[4]);
    let second = u32::from(time[5]);

    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let mut days = 0u32;
    let mut y = 2000u32;
    while y < year {
        days = days.checked_add(if is_leap_year(y) { 366 } else { 365 })?;
        y += 1;
    }

    let mut m = 1u32;
    while m < month {
        days = days.checked_add(days_in_month(year, m))?;
        m += 1;
    }
    days = days.checked_add(day - 1)?;

    days.checked_mul(86_400)?
        .checked_add(hour.checked_mul(3_600)?)?
        .checked_add(minute.checked_mul(60)?)?
        .checked_add(second)
}

fn epoch_secs_to_report_time(epoch_secs: u32) -> Option<[u8; 6]> {
    let mut days = epoch_secs / 86_400;
    let seconds_of_day = epoch_secs % 86_400;

    let mut year = 2000u32;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
        if year > 2099 {
            return None;
        }
    }

    let mut month = 1u32;
    loop {
        let month_days = days_in_month(year, month);
        if days < month_days {
            break;
        }
        days -= month_days;
        month += 1;
    }

    Some([
        (year - 2000) as u8,
        month as u8,
        (days + 1) as u8,
        (seconds_of_day / 3_600) as u8,
        ((seconds_of_day % 3_600) / 60) as u8,
        (seconds_of_day % 60) as u8,
    ])
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400)
}

fn parse_radix_u32(bytes: &[u8], radix: u32) -> Option<u32> {
    let mut value = 0u32;
    for &byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a') + 10,
            b'A'..=b'F' => u32::from(byte - b'A') + 10,
            _ => return None,
        };
        if digit >= radix {
            return None;
        }
        value = value.checked_mul(radix)?.checked_add(digit)?;
    }
    Some(value)
}
