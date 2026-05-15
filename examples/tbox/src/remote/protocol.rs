use core::sync::atomic::{AtomicU8, Ordering};

use heapless::Vec;

use crate::modem;

use super::state::{RemoteSnapshot, snapshot};

pub const HEARTBEAT_PERIOD_SECS: u64 = 30;
pub const RECONNECT_DELAY_SECS: u64 = 360;
pub const MAX_PACKET_LEN: usize = 256;

pub const COMMAND_HEARTBEAT: u8 = 0x18;
pub const COMMAND_DEVICE_INFO: u8 = 0x1b;
pub const COMMAND_BATTERY_DETAIL: u8 = 0x2d;
pub const COMMAND_WEIGHING: u8 = 0x2e;
pub const COMMAND_WORK_HOUR: u8 = 0x2f;

const HEADER: [u8; 2] = [0xd2, 0xcf];
const PROTOCOL_KEY: u32 = 0x5cad_f34e;
const VERSION: u8 = 0x04;
const DEFAULT_DEVICE_ID: u32 = 0;

static PACKET_SEQUENCE: AtomicU8 = AtomicU8::new(0);

const DEVICE_ID_ENV: Option<&str> = option_env!("CONFIG_TBOX_DEVICE_ID");

pub type Packet = Vec<u8, MAX_PACKET_LEN>;

pub fn build_device_info_packet() -> Option<Packet> {
    let state = snapshot();
    let mut data: Vec<u8, 64> = Vec::new();
    push_all(&mut data, &[0x00, 0x01])?; // hardware version 00.01
    push_all(&mut data, &[0x00, 0x00, 0x01])?; // software version 00.00.01
    push_repeat(&mut data, 0xff, 10)?; // SIM unknown
    push_repeat(&mut data, 0xff, 6)?; // Bluetooth MAC unknown
    push_all(&mut data, &state.vin)?;
    push_repeat(&mut data, 0xff, 8)?; // VCU version unknown
    build_packet(COMMAND_DEVICE_INFO, &data)
}

pub fn build_heartbeat_packet() -> Option<Packet> {
    let state = snapshot();
    let mut data: Vec<u8, 192> = Vec::new();

    push_time_default(&mut data)?;
    data.push(state.soh_percent.unwrap_or(0)).ok()?;
    data.push(state.vehicle_state.unwrap_or(0xff)).ok()?;
    push_repeat(&mut data, 0xff, 4)?; // member card unsupported
    data.push(state.key_state.unwrap_or(0)).ok()?;
    data.push(state.shift.unwrap_or(0)).ok()?;
    data.push(0xff).ok()?; // window unsupported
    push_repeat(&mut data, 0xff, 12)?; // serial number unsupported
    push_repeat(&mut data, 0xff, 4)?; // reserved
    data.push(0xff).ok()?; // door unsupported
    data.push(state.speed_kph.unwrap_or(0xff)).ok()?;
    push_u24_be(&mut data, state.total_mileage_deci_km.filter(|value| *value <= 0x00ff_ffff))?;
    push_u16_be(&mut data, None)?; // low-voltage battery unsupported
    push_u16_be(&mut data, state.battery_voltage_deci_v)?;
    push_total_current(&mut data, state.total_current_deci_a)?;
    data.push(state.soc_percent.unwrap_or(0)).ok()?;
    push_u16_be(&mut data, None)?; // remaining range unsupported
    data.push(0xff).ok()?; // GPS CN unsupported
    data.push(0).ok()?; // GPS antenna default normal
    data.push(1).ok()?; // location default invalid
    push_u32_be(&mut data, None)?; // longitude unsupported
    push_u32_be(&mut data, None)?; // latitude unsupported
    data.push(0xff).ok()?; // A/C unsupported
    data.push(0xff).ok()?; // fault level unsupported
    data.push(0xff).ok()?; // battery pack current status unsupported
    data.push(0xff).ok()?; // battery alarm unsupported
    data.push(0xff).ok()?; // motor system temperature unsupported
    data.push(0xff).ok()?; // GPRS signal unsupported here
    data.push(state.handbrake.unwrap_or(0)).ok()?;
    data.push(0xff).ok()?; // lights unsupported
    data.push(0xff).ok()?; // trunk unsupported
    data.push(0xff).ok()?; // key plug state unsupported
    data.push(0xff).ok()?; // vehicle lock unsupported
    push_u16_be(&mut data, None)?; // accel X
    push_u16_be(&mut data, None)?; // accel Y
    push_u16_be(&mut data, None)?; // accel Z
    push_u16_be(&mut data, None)?; // gyro X
    push_u16_be(&mut data, None)?; // gyro Y
    push_u16_be(&mut data, None)?; // gyro Z
    data.push(0xff).ok()?; // heading unsupported
    push_u16_be(&mut data, None)?; // cabin temp unsupported
    push_u16_be(&mut data, None)?; // motor speed unsupported
    data.push(0xff).ok()?; // vehicle type unsupported
    data.push(0xff).ok()?; // vehicle subtype unsupported
    push_u16_be(&mut data, None)?; // load unsupported
    data.push(0xff).ok()?; // pitch unsupported
    push_u16_be(&mut data, None)?; // altitude unsupported
    push_u32_be(&mut data, None)?; // total fuel unsupported
    push_u24_be(&mut data, state.total_charge_deci_kwh)?;
    push_u24_be(&mut data, state.total_discharge_deci_kwh)?;
    push_u24_be(&mut data, state.total_plug_charge_deci_kwh)?;
    push_u24_be(&mut data, state.total_swap_charge_deci_kwh)?;
    push_u24_be(&mut data, state.total_regen_deci_kwh)?;
    push_battery_identity_header(&mut data, &state)?;
    push_all(&mut data, &state.battery_sn)?;
    push_all(&mut data, &state.vin)?;

    build_packet(COMMAND_HEARTBEAT, &data)
}

pub fn build_battery_detail_packet() -> Option<Packet> {
    let state = snapshot();
    let mut data: Vec<u8, 96> = Vec::new();

    push_time_default(&mut data)?;
    push_temp_c(&mut data, state.max_cell_temp_c)?;
    push_temp_c(&mut data, state.min_cell_temp_c)?;
    push_temp_c(&mut data, state.avg_cell_temp_c)?;
    data.push(state.max_temp_csc.unwrap_or(0xff)).ok()?;
    data.push(state.max_temp_probe.unwrap_or(0xff)).ok()?;
    data.push(state.min_temp_csc.unwrap_or(0xff)).ok()?;
    data.push(state.min_temp_probe.unwrap_or(0xff)).ok()?;
    data.push(state.max_cell_csc.unwrap_or(0xff)).ok()?;
    data.push(state.max_cell_index.unwrap_or(0xff)).ok()?;
    data.push(state.min_cell_csc.unwrap_or(0xff)).ok()?;
    data.push(state.min_cell_index.unwrap_or(0xff)).ok()?;
    data.push(0xff).ok()?; // reserved
    push_u16_be(&mut data, state.positive_insulation_kohm)?;
    push_u16_be(&mut data, state.negative_insulation_kohm)?;
    push_u16_be(&mut data, state.battery_voltage_deci_v)?;
    push_u16_be(&mut data, state.bus_voltage_deci_v)?;
    push_u16_be(&mut data, state.max_cell_voltage_mv)?;
    push_u16_be(&mut data, state.avg_cell_voltage_mv)?;
    push_u16_be(&mut data, state.min_cell_voltage_mv)?;
    push_all(&mut data, &state.bms_version)?;
    data.push(state.main_negative_relay_state.map(|value| value << 6).unwrap_or(0xff)).ok()?;
    push_repeat(&mut data, 0xff, 20)?; // B-BOX ID unknown
    push_repeat(&mut data, 0xff, 6)?; // reserved

    build_packet(COMMAND_BATTERY_DETAIL, &data)
}

pub fn build_weighing_packet() -> Option<Packet> {
    let mut data: Vec<u8, 16> = Vec::new();

    push_time_default(&mut data)?;
    data.push(0xff).ok()?; // weighing sensor unsupported
    push_repeat(&mut data, 0xff, 7)?; // reserved

    build_packet(COMMAND_WEIGHING, &data)
}

pub fn build_work_hour_packet() -> Option<Packet> {
    let mut data: Vec<u8, 32> = Vec::new();

    push_time_default(&mut data)?;
    push_u32_be(&mut data, None)?; // cumulative HV runtime unsupported
    data.push(0xff).ok()?; // current HV state unsupported
    push_repeat(&mut data, 0xff, 8)?; // HV on time unsupported
    push_repeat(&mut data, 0xff, 8)?; // HV off time unsupported
    data.push(0xff).ok()?; // work motor state unsupported
    push_repeat(&mut data, 0xff, 2)?; // reserved

    build_packet(COMMAND_WORK_HOUR, &data)
}

fn build_packet(command: u8, data: &[u8]) -> Option<Packet> {
    let mut packet = Packet::new();
    let sequence = next_sequence();
    push_all(&mut packet, &HEADER)?;
    let length = 1usize + 4 + 1 + 1 + data.len() + 2;
    if length > u16::MAX as usize {
        return None;
    }
    push_u16_raw_be(&mut packet, length as u16)?;
    packet.push(VERSION).ok()?;
    push_u32_raw_be(&mut packet, device_id())?;
    packet.push(sequence).ok()?;
    packet.push(command).ok()?;
    push_all(&mut packet, data)?;

    let mut crc_input = Packet::new();
    push_u32_raw_be(&mut crc_input, PROTOCOL_KEY)?;
    push_all(&mut crc_input, &packet)?;
    let crc = crc16_protocol(&crc_input);
    push_u16_raw_be(&mut packet, crc)?;
    Some(packet)
}

fn next_sequence() -> u8 {
    PACKET_SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

fn device_id() -> u32 {
    modem::device_id_from_imei()
        .or_else(|| env_u32(DEVICE_ID_ENV))
        .unwrap_or(DEFAULT_DEVICE_ID)
}

fn push_time_default<const N: usize>(data: &mut Vec<u8, N>) -> Option<()> {
    push_repeat(data, 0, 6)
}

fn push_total_current<const N: usize>(data: &mut Vec<u8, N>, current_deci_a: Option<i32>) -> Option<()> {
    let encoded = current_deci_a.and_then(|value| {
        let raw = value + 20_000;
        if (0..=u16::MAX as i32).contains(&raw) {
            Some(raw as u16)
        } else {
            None
        }
    });
    push_u16_be(data, encoded)
}

fn push_battery_identity_header<const N: usize>(data: &mut Vec<u8, N>, state: &RemoteSnapshot) -> Option<()> {
    let manufacturer = state.battery_manufacturer.unwrap_or(0).min(0x07ff);
    let raw = manufacturer;
    push_u16_raw_be(data, raw)
}

fn push_temp_c<const N: usize>(data: &mut Vec<u8, N>, temp_c: Option<i16>) -> Option<()> {
    let raw = temp_c.and_then(|value| {
        let encoded = value + 50;
        if (0..=254).contains(&encoded) {
            Some(encoded as u8)
        } else {
            None
        }
    });
    data.push(raw.unwrap_or(0xff)).ok()
}

fn push_u16_be<const N: usize>(data: &mut Vec<u8, N>, value: Option<u16>) -> Option<()> {
    push_u16_raw_be(data, value.unwrap_or(0xffff))
}

fn push_u24_be<const N: usize>(data: &mut Vec<u8, N>, value: Option<u32>) -> Option<()> {
    let raw = value.filter(|value| *value <= 0x00ff_ffff).unwrap_or(0x00ff_ffff);
    data.push((raw >> 16) as u8).ok()?;
    data.push((raw >> 8) as u8).ok()?;
    data.push(raw as u8).ok()
}

fn push_u32_be<const N: usize>(data: &mut Vec<u8, N>, value: Option<u32>) -> Option<()> {
    push_u32_raw_be(data, value.unwrap_or(0xffff_ffff))
}

fn push_u16_raw_be<const N: usize>(data: &mut Vec<u8, N>, value: u16) -> Option<()> {
    data.push((value >> 8) as u8).ok()?;
    data.push(value as u8).ok()
}

fn push_u32_raw_be<const N: usize>(data: &mut Vec<u8, N>, value: u32) -> Option<()> {
    data.push((value >> 24) as u8).ok()?;
    data.push((value >> 16) as u8).ok()?;
    data.push((value >> 8) as u8).ok()?;
    data.push(value as u8).ok()
}

fn push_repeat<const N: usize>(data: &mut Vec<u8, N>, value: u8, count: usize) -> Option<()> {
    for _ in 0..count {
        data.push(value).ok()?;
    }
    Some(())
}

fn push_all<const N: usize>(data: &mut Vec<u8, N>, bytes: &[u8]) -> Option<()> {
    data.extend_from_slice(bytes).ok()
}

const CRC_HI: [u8; 256] = [
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
];

const CRC_LO: [u8; 256] = [
    0x00, 0xc0, 0xc1, 0x01, 0xc3, 0x03, 0x02, 0xc2, 0xc6, 0x06, 0x07, 0xc7, 0x05, 0xc5, 0xc4, 0x04,
    0xcc, 0x0c, 0x0d, 0xcd, 0x0f, 0xcf, 0xce, 0x0e, 0x0a, 0xca, 0xcb, 0x0b, 0xc9, 0x09, 0x08, 0xc8,
    0xd8, 0x18, 0x19, 0xd9, 0x1b, 0xdb, 0xda, 0x1a, 0x1e, 0xde, 0xdf, 0x1f, 0xdd, 0x1d, 0x1c, 0xdc,
    0x14, 0xd4, 0xd5, 0x15, 0xd7, 0x17, 0x16, 0xd6, 0xd2, 0x12, 0x13, 0xd3, 0x11, 0xd1, 0xd0, 0x10,
    0xf0, 0x30, 0x31, 0xf1, 0x33, 0xf3, 0xf2, 0x32, 0x36, 0xf6, 0xf7, 0x37, 0xf5, 0x35, 0x34, 0xf4,
    0x3c, 0xfc, 0xfd, 0x3d, 0xff, 0x3f, 0x3e, 0xfe, 0xfa, 0x3a, 0x3b, 0xfb, 0x39, 0xf9, 0xf8, 0x38,
    0x28, 0xe8, 0xe9, 0x29, 0xeb, 0x2b, 0x2a, 0xea, 0xee, 0x2e, 0x2f, 0xef, 0x2d, 0xed, 0xec, 0x2c,
    0xe4, 0x24, 0x25, 0xe5, 0x27, 0xe7, 0xe6, 0x26, 0x22, 0xe2, 0xe3, 0x23, 0xe1, 0x21, 0x20, 0xe0,
    0xa0, 0x60, 0x61, 0xa1, 0x63, 0xa3, 0xa2, 0x62, 0x66, 0xa6, 0xa7, 0x67, 0xa5, 0x65, 0x64, 0xa4,
    0x6c, 0xac, 0xad, 0x6d, 0xaf, 0x6f, 0x6e, 0xae, 0xaa, 0x6a, 0x6b, 0xab, 0x69, 0xa9, 0xa8, 0x68,
    0x78, 0xb8, 0xb9, 0x79, 0xbb, 0x7b, 0x7a, 0xba, 0xbe, 0x7e, 0x7f, 0xbf, 0x7d, 0xbd, 0xbc, 0x7c,
    0xb4, 0x74, 0x75, 0xb5, 0x77, 0xb7, 0xb6, 0x76, 0x72, 0xb2, 0xb3, 0x73, 0xb1, 0x71, 0x70, 0xb0,
    0x50, 0x90, 0x91, 0x51, 0x93, 0x53, 0x52, 0x92, 0x96, 0x56, 0x57, 0x97, 0x55, 0x95, 0x94, 0x54,
    0x9c, 0x5c, 0x5d, 0x9d, 0x5f, 0x9f, 0x9e, 0x5e, 0x5a, 0x9a, 0x9b, 0x5b, 0x99, 0x59, 0x58, 0x98,
    0x88, 0x48, 0x49, 0x89, 0x4b, 0x8b, 0x8a, 0x4a, 0x4e, 0x8e, 0x8f, 0x4f, 0x8d, 0x4d, 0x4c, 0x8c,
    0x44, 0x84, 0x85, 0x45, 0x87, 0x47, 0x46, 0x86, 0x82, 0x42, 0x43, 0x83, 0x41, 0x81, 0x80, 0x40,
];

fn crc16_protocol(packet_without_crc: &[u8]) -> u16 {
    let mut crc_hi = 0xffu8;
    let mut crc_lo = 0xffu8;

    for &byte in packet_without_crc {
        let index = (crc_lo ^ byte) as usize;
        crc_lo = crc_hi ^ CRC_HI[index];
        crc_hi = CRC_LO[index];
    }

    (u16::from(crc_hi) << 8) | u16::from(crc_lo)
}

fn env_u32(value: Option<&str>) -> Option<u32> {
    let text = value?;
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        parse_radix_u32(hex.as_bytes(), 16)
    } else {
        parse_radix_u32(text.as_bytes(), 10)
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
