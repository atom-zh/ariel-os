use heapless::Vec;

use super::state::{RemoteSnapshot, snapshot};

pub const HEARTBEAT_PERIOD_SECS: u64 = 30;
pub const RECONNECT_DELAY_SECS: u64 = 360;
pub const MAX_PACKET_LEN: usize = 256;

pub const COMMAND_HEARTBEAT: u8 = 0x18;
pub const COMMAND_DEVICE_INFO: u8 = 0x1b;
pub const COMMAND_BATTERY_DETAIL: u8 = 0x2d;

const HEADER: [u8; 2] = [0xd2, 0xcf];
const VERSION: u8 = 0x05;
const DEFAULT_DEVICE_ID: u32 = 0;
const DEFAULT_CRC_KEY: u32 = 0;

const DEVICE_ID_ENV: Option<&str> = option_env!("CONFIG_TBOX_DEVICE_ID");
const CRC_KEY_ENV: Option<&str> = option_env!("CONFIG_TBOX_CRC_KEY");

pub type Packet = Vec<u8, MAX_PACKET_LEN>;

pub fn build_device_info_packet(sequence: u8) -> Option<Packet> {
    let state = snapshot();
    let mut data: Vec<u8, 64> = Vec::new();
    push_all(&mut data, &[0x00, 0x01])?; // hardware version 00.01
    push_all(&mut data, &[0x00, 0x00, 0x01])?; // software version 00.00.01
    push_repeat(&mut data, 0xff, 10)?; // SIM unknown
    push_repeat(&mut data, 0xff, 6)?; // Bluetooth MAC unknown
    push_all(&mut data, &state.vin)?;
    push_repeat(&mut data, 0xff, 8)?; // VCU version unknown
    build_packet(COMMAND_DEVICE_INFO, sequence, &data)
}

pub fn build_heartbeat_packet(sequence: u8) -> Option<Packet> {
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

    build_packet(COMMAND_HEARTBEAT, sequence, &data)
}

pub fn build_battery_detail_packet(sequence: u8) -> Option<Packet> {
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

    build_packet(COMMAND_BATTERY_DETAIL, sequence, &data)
}

fn build_packet(command: u8, sequence: u8, data: &[u8]) -> Option<Packet> {
    let mut packet = Packet::new();
    push_all(&mut packet, &HEADER)?;
    let length = 1usize + 4 + 1 + 1 + data.len() + 2;
    if length > u16::MAX as usize {
        return None;
    }
    push_u16_raw_be(&mut packet, length as u16)?;
    packet.push(VERSION).ok()?;
    push_u32_raw_be(&mut packet, env_u32(DEVICE_ID_ENV).unwrap_or(DEFAULT_DEVICE_ID))?;
    packet.push(sequence).ok()?;
    packet.push(command).ok()?;
    push_all(&mut packet, data)?;

    let crc = crc16_ccitt_false_with_key(env_u32(CRC_KEY_ENV).unwrap_or(DEFAULT_CRC_KEY), &packet);
    push_u16_raw_be(&mut packet, crc)?;
    Some(packet)
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
    let manufacturer = state.battery_manufacturer.unwrap_or(0).min(0x07);
    let len = state.battery_sn_len.unwrap_or(0).min(27);
    let raw = (u16::from(len) << 11) | manufacturer;
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

fn crc16_ccitt_false_with_key(key: u32, packet_without_crc: &[u8]) -> u16 {
    let key_bytes = key.to_be_bytes();
    let mut crc = 0xffff;
    for byte in key_bytes.iter().chain(packet_without_crc.iter()) {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
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
