use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static DEVICE_ID_FROM_IMEI: AtomicU32 = AtomicU32::new(0);
static DEVICE_ID_FROM_IMEI_SET: AtomicBool = AtomicBool::new(false);

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
