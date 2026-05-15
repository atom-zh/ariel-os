//! Pure electric vehicle CAN protocol V4.6, BMS section.
//!
//! Source document: `doc/my/纯电动汽车CAN 通信协议V4.6-20250613.pdf`.
//! The protocol uses Intel/little-endian byte order on the 250 kbit/s vehicle CAN.

use super::*;

pub struct BmsV46Profile;

impl VehicleCanProfile for BmsV46Profile {
    fn name(&self) -> &'static str {
        "bms-v4.6"
    }

    fn parse(&self, frame: &VehicleCanFrame) -> Result<Option<VehicleMessage>, ParseError> {
        require_len(frame, 8)?;

        let data = frame.data;
        let message = match frame.id {
            ID_BMS_CMD_STATUS => VehicleMessage::BmsCommandStatus(parse_bms_command_status(data)),
            ID_BATTERY_SN_1 => VehicleMessage::BatteryIdentityChunk(parse_identity_chunk(data, 1)),
            ID_BATTERY_SN_2 => VehicleMessage::BatteryIdentityChunk(parse_identity_chunk(data, 2)),
            ID_BATTERY_SN_3 => VehicleMessage::BatteryIdentityChunk(parse_identity_chunk(data, 3)),
            ID_BATTERY_SN_4 => VehicleMessage::BatteryIdentityChunk(parse_identity_chunk(data, 4)),
            ID_BATT_INFO_1 => VehicleMessage::BatteryNameplate(parse_battery_nameplate(data)),
            ID_BATT_INFO_2 => VehicleMessage::BatteryPackLayout(parse_battery_pack_layout(data)),
            ID_TEMP_SAMPLES => {
                VehicleMessage::TemperatureSamples(parse_temperature_samples(frame.id, data))
            }
            ID_CELL_VOLTAGE_1 | ID_CELL_VOLTAGE_2 => {
                VehicleMessage::CellVoltageSamples(parse_cell_voltage_samples(frame.id, data))
            }
            ID_EXPANDABLE_PACK_VOLTAGE => {
                VehicleMessage::PackVoltage(parse_expandable_pack_voltage(data))
            }
            ID_BMS_STATUS_0 => VehicleMessage::BmsStatus0(parse_bms_status0(data)),
            ID_BMS_STATUS_1 => {
                check_xor(frame.id, data)?;
                VehicleMessage::BmsStatus1(parse_bms_status1(data))
            }
            ID_BATTERY_REALTIME => VehicleMessage::BatteryRealtime(parse_battery_realtime(data)),
            ID_INSULATION_STATUS => VehicleMessage::InsulationStatus(parse_insulation_status(data)),
            ID_TEMP_SUMMARY => VehicleMessage::TemperatureSummary(parse_temperature_summary(data)),
            ID_CELL_VOLTAGE_MAX => VehicleMessage::CellVoltageMaximum(parse_cell_voltage_max(data)),
            ID_CELL_VOLTAGE_MIN_AND_RELAY_TEMP => {
                VehicleMessage::CellVoltageMinimum(parse_cell_voltage_min(data))
            }
            ID_CHARGE_CONNECTOR_TEMPS => VehicleMessage::ChargeConnectorTemperatures(
                parse_charge_connector_temperatures(data),
            ),
            ID_VERSION_INFO => VehicleMessage::VersionInfo(parse_version_info(data)),
            ID_WATER_LOOP_TEMPS => {
                VehicleMessage::WaterLoopTemperatures(parse_water_loop_temperatures(data))
            }
            ID_EXTENDED_RELAY_STATUS => {
                VehicleMessage::ExtendedRelayStatus(parse_extended_relay_status(data))
            }
            ID_PACK_POLE_STATUS => VehicleMessage::PackPoleStatus(parse_pack_pole_status(data)),
            ID_ENERGY_CHG_DISCHG_24 => VehicleMessage::EnergyCounter(parse_energy_24(
                data,
                EnergyCounterKind::ChargeDischarge24,
                0,
                3,
                Some(6),
            )),
            ID_ENERGY_REGEN_SWAP_24 => VehicleMessage::EnergyCounter(parse_energy_24(
                data,
                EnergyCounterKind::RegenSwap24,
                0,
                3,
                None,
            )),
            ID_ENERGY_PLUG_24 => VehicleMessage::EnergyCounter(parse_energy_24(
                data,
                EnergyCounterKind::PlugCharge24,
                0,
                6,
                None,
            )),
            ID_ENERGY_CHG_DISCHG_32 => VehicleMessage::EnergyCounter(parse_energy_32(
                data,
                EnergyCounterKind::ChargeDischarge32,
                0,
                4,
                None,
            )),
            ID_ENERGY_REGEN_SWAP_32 => VehicleMessage::EnergyCounter(parse_energy_32(
                data,
                EnergyCounterKind::RegenSwap32,
                0,
                4,
                None,
            )),
            ID_ENERGY_PLUG_SINGLE_32 => VehicleMessage::EnergyCounter(parse_energy_32(
                data,
                EnergyCounterKind::PlugAndSingleCharge32,
                0,
                4,
                None,
            )),
            ID_ENERGY_SINGLE_REGEN_32 => VehicleMessage::EnergyCounter(EnergyCounter {
                kind: EnergyCounterKind::SingleRegen32,
                first_deci_kwh: valid_u32(le_u32(data, 0)),
                second_deci_kwh: None,
                extra: valid_u16(le_u16(data, 4)).map(u32::from),
            }),
            ID_CAPACITY_CHG_DISCHG_32 => VehicleMessage::CapacityCounter(parse_capacity_32(
                data,
                CapacityCounterKind::ChargeDischarge32,
                0,
                4,
            )),
            ID_CAPACITY_REGEN_SWAP_32 => VehicleMessage::CapacityCounter(parse_capacity_32(
                data,
                CapacityCounterKind::RegenSwap32,
                0,
                4,
            )),
            ID_CAPACITY_PLUG_SINGLE_32 => VehicleMessage::CapacityCounter(parse_capacity_32(
                data,
                CapacityCounterKind::PlugAndSingleCharge32,
                0,
                4,
            )),
            ID_DBC_VIN => VehicleMessage::VinChunk(parse_vin_chunk(data)),
            ID_DBC_VEHICLE_INFO => VehicleMessage::VehicleInfo(parse_vehicle_info(data)),
            ID_BBOX_AUTH_REQUEST
            | ID_BBOX_AUTH_RESPONSE
            | ID_B2V_ST11
            | ID_OBC_CHARGE_CTRL
            | ID_STORAGE_FAULT
            | ID_BMS_CLOUD_INFO => VehicleMessage::KnownReserved { id: frame.id },
            _ => return Ok(None),
        };

        Ok(Some(message))
    }
}

#[allow(dead_code)]
pub mod outbound_ids {
    //! IDs defined by the V4.6 PDF for frames sent by VCU, cloud, BBOX, or OBC.
    //!
    //! They are kept with the parser profile so the later TCP/cloud and command paths can use
    //! the same source of truth as the BMS receive parser.

    pub const VCU_COMMAND: u32 = 0x1802_F3D0;
    pub const BBOX_COMMAND: u32 = 0x1803_F3D0;
    pub const OBC_FEEDBACK: u32 = 0x1804_F3D0;
    pub const CLOUD_COMMAND: u32 = 0x1801_F3D1;
    pub const CLOUD_TIME: u32 = 0x1802_F3D1;
    pub const VCU_INSULATION_COMMAND: u32 = 0x18AA_F3D0;
}

const ID_BMS_CMD_STATUS: u32 = 0x1801_D0F3;
const ID_BATTERY_SN_1: u32 = 0x18E1_D0F3;
const ID_BATTERY_SN_2: u32 = 0x18E2_D0F3;
const ID_BATTERY_SN_3: u32 = 0x18E3_D0F3;
const ID_BATTERY_SN_4: u32 = 0x18E4_D0F3;
const ID_BATT_INFO_1: u32 = 0x18E5_D0F3;
const ID_BATT_INFO_2: u32 = 0x18E6_D0F3;
const ID_TEMP_SAMPLES: u32 = 0x18C2_D0F3;
const ID_CELL_VOLTAGE_1: u32 = 0x18C1_D0F3;
const ID_CELL_VOLTAGE_2: u32 = 0x18C3_D0F3;
const ID_EXPANDABLE_PACK_VOLTAGE: u32 = 0x18C4_D0F3;
const ID_BMS_STATUS_0: u32 = 0x1880_D0F3;
const ID_BMS_STATUS_1: u32 = 0x1881_D0F3;
const ID_BATTERY_REALTIME: u32 = 0x1882_D0F3;
const ID_INSULATION_STATUS: u32 = 0x1883_D0F3;
const ID_TEMP_SUMMARY: u32 = 0x1884_D0F3;
const ID_CELL_VOLTAGE_MAX: u32 = 0x1885_D0F3;
const ID_CELL_VOLTAGE_MIN_AND_RELAY_TEMP: u32 = 0x1886_D0F3;
const ID_CHARGE_CONNECTOR_TEMPS: u32 = 0x1887_D0F3;
const ID_VERSION_INFO: u32 = 0x1888_D0F3;
const ID_BBOX_AUTH_REQUEST: u32 = 0x1889_D0F3;
const ID_BBOX_AUTH_RESPONSE: u32 = 0x188A_D0F3;
const ID_B2V_ST11: u32 = 0x188B_D0F3;
const ID_WATER_LOOP_TEMPS: u32 = 0x188C_D0F3;
const ID_OBC_CHARGE_CTRL: u32 = 0x188D_D0F3;
const ID_EXTENDED_RELAY_STATUS: u32 = 0x188E_D0F3;
const ID_PACK_POLE_STATUS: u32 = 0x188F_D0F3;
const ID_ENERGY_CHG_DISCHG_24: u32 = 0x18F1_D0F3;
const ID_ENERGY_REGEN_SWAP_24: u32 = 0x18F2_D0F3;
const ID_ENERGY_PLUG_24: u32 = 0x18F3_D0F3;
const ID_ENERGY_CHG_DISCHG_32: u32 = 0x18F4_D0F3;
const ID_ENERGY_REGEN_SWAP_32: u32 = 0x18F5_D0F3;
const ID_ENERGY_PLUG_SINGLE_32: u32 = 0x18F6_D0F3;
const ID_ENERGY_SINGLE_REGEN_32: u32 = 0x18F7_D0F3;
const ID_CAPACITY_CHG_DISCHG_32: u32 = 0x18F8_D0F3;
const ID_CAPACITY_REGEN_SWAP_32: u32 = 0x18F9_D0F3;
const ID_CAPACITY_PLUG_SINGLE_32: u32 = 0x18FA_D0F3;
const ID_DBC_VIN: u32 = 0x18E1_F3D0;
const ID_DBC_VEHICLE_INFO: u32 = 0x18FF_A7EF;
const ID_STORAGE_FAULT: u32 = 0x1880_D2F3;
const ID_BMS_CLOUD_INFO: u32 = 0x1880_D1F3;

fn require_len(frame: &VehicleCanFrame, expected: usize) -> Result<(), ParseError> {
    if frame.len == expected {
        Ok(())
    } else {
        Err(ParseError::InvalidDlc {
            id: frame.id,
            expected,
            actual: frame.len,
        })
    }
}

fn check_xor(id: u32, data: [u8; 8]) -> Result<(), ParseError> {
    let expected = data[1..].iter().fold(0, |acc, byte| acc ^ byte);
    if data[0] == expected {
        Ok(())
    } else {
        Err(ParseError::Checksum {
            id,
            expected,
            actual: data[0],
        })
    }
}

fn parse_bms_command_status(data: [u8; 8]) -> BmsCommandStatus {
    BmsCommandStatus {
        full_charge_request: bits(data[0], 4, 0b11),
        power_down_request: bits(data[0], 2, 0b11),
        pack_current_deci_a: valid_u16(le_u16(data, 3)).map(|raw| i32::from(raw) - 32_000),
        special_charge_discharge_state: bits(data[5], 0, 0b111),
    }
}

fn parse_identity_chunk(data: [u8; 8], chunk_index: u8) -> BatteryIdentityChunk {
    let mut bytes = [0xff; 7];
    let start = if chunk_index == 1 { 2 } else { 1 };
    let valid_bytes = (8 - start).min(bytes.len());
    bytes[..valid_bytes].copy_from_slice(&data[start..8]);

    BatteryIdentityChunk {
        chunk_index,
        declared_len: if chunk_index == 1 {
            Some(data[1] >> 3)
        } else {
            None
        },
        manufacturer: if chunk_index == 1 {
            Some(u16::from(data[1] & 0b111))
        } else {
            None
        },
        bytes,
        valid_bytes,
    }
}

fn parse_vin_chunk(data: [u8; 8]) -> VinChunk {
    let frame_seq = data[0];
    let mut bytes = [0xff; 7];
    let valid_bytes = match frame_seq {
        1 | 2 => 7,
        3 => 3,
        _ => 0,
    };
    bytes[..valid_bytes].copy_from_slice(&data[1..1 + valid_bytes]);

    VinChunk {
        frame_seq,
        bytes,
        valid_bytes,
    }
}

fn parse_vehicle_info(data: [u8; 8]) -> VehicleInfo {
    VehicleInfo {
        total_mileage_deci_km: valid_u32(le_u32(data, 0)),
        speed_1_256_kph: valid_u16(le_u16(data, 4)),
        shift: data[6] & 0x0f,
        handbrake: data[6] >> 4,
    }
}

fn parse_battery_nameplate(data: [u8; 8]) -> BatteryNameplate {
    BatteryNameplate {
        rated_capacity_ah: valid_u16(le_u16(data, 0)),
        rated_voltage_deci_v: valid_u16(le_u16(data, 2)),
        rated_energy_deci_kwh: valid_u16(le_u16(data, 4)),
        a_plus_wakeup: data[6] & 0x80 != 0,
        key_on_wakeup: data[6] & 0x40 != 0,
        cooling_mode: bits(data[6], 4, 0b11),
        chemistry: data[7],
    }
}

fn parse_battery_pack_layout(data: [u8; 8]) -> BatteryPackLayout {
    BatteryPackLayout {
        csc_count: valid_u8(data[0]),
        cell_count: valid_u16(le_u16(data, 1)),
        temperature_probe_count: valid_u16(le_u16(data, 3)),
        nominal_cell_voltage_deci_v: valid_u8(data[5]),
        min_cell_voltage_deci_v: valid_u8(data[6]),
        max_cell_voltage_deci_v: valid_u8(data[7]),
    }
}

fn parse_temperature_samples(id: u32, data: [u8; 8]) -> TemperatureSamples {
    TemperatureSamples {
        id,
        sequence: valid_u8(data[0]),
        csc: valid_u8(data[1]),
        temperatures_c: [
            temp_c(data[2]),
            temp_c(data[3]),
            temp_c(data[4]),
            temp_c(data[5]),
            temp_c(data[6]),
            temp_c(data[7]),
        ],
    }
}

fn parse_cell_voltage_samples(id: u32, data: [u8; 8]) -> CellVoltageSamples {
    CellVoltageSamples {
        id,
        sequence: valid_u8(data[0]),
        csc: valid_u8(data[1]),
        voltages_mv: [
            valid_u16(le_u16(data, 2)),
            valid_u16(le_u16(data, 4)),
            valid_u16(le_u16(data, 6)),
        ],
    }
}

fn parse_expandable_pack_voltage(data: [u8; 8]) -> ExpandablePackVoltage {
    let count_raw = u16::from(data[5]) | (u16::from(data[6] & 0x0f) << 8);
    ExpandablePackVoltage {
        pack_index: data[0],
        pack_voltage_deci_v: valid_u16_except(le_u16(data, 1), 0xfffe),
        min_parallel_unit_mv: valid_u16_except(le_u16(data, 3), 0xfffe),
        min_parallel_unit_count: if count_raw == 0x0fff || count_raw == 0x0ffe {
            None
        } else {
            Some(count_raw)
        },
    }
}

fn parse_bms_status0(data: [u8; 8]) -> BmsStatus0 {
    BmsStatus0 {
        rechargeable_system_index: data[0] & 0x0f,
        main_positive_relay_state: bits(data[1], 4, 0b11),
        main_negative_relay_state: bits(data[1], 6, 0b11),
        precharge_relay_state: bits(data[2], 0, 0b11),
        charge_positive_relay1_state: bits(data[2], 2, 0b11),
        charge_negative_relay1_state: bits(data[2], 4, 0b11),
        system_fault_code: data[7],
        raw: data,
    }
}

fn parse_bms_status1(data: [u8; 8]) -> BmsStatus1 {
    BmsStatus1 {
        balance_active: data[1] & 0x80 != 0,
        accessory_relay_closed: data[1] & 0x40 != 0,
        bms_state: bits(data[1], 4, 0b11),
        alive_counter: data[1] & 0x0f,
        swap_station_charge_connected: data[2] & 0x80 != 0,
        max_alarm_level: bits(data[2], 5, 0b11),
        charge_state: bits(data[2], 3, 0b11),
        charge_mode: bits(data[2], 1, 0b11),
        charge_connector_connected: data[2] & 0x01 != 0,
        gb32960_fault_count: data[7] >> 3,
        thermal_runaway_alarm: data[7] & 0x04 != 0,
        smoke_alarm: data[7] & 0x02 != 0,
        hv_interlock_alarm: data[7] & 0x01 != 0,
        raw: data,
    }
}

fn parse_battery_realtime(data: [u8; 8]) -> BatteryRealtime {
    BatteryRealtime {
        soc_deci_percent: valid_u8(data[0]).map(|v| u16::from(v) * 4),
        soh_deci_percent: valid_u8(data[1]).map(|v| u16::from(v) * 4),
        pack_current_deci_a: valid_u16(le_u16(data, 2)).map(|raw| i32::from(raw) - 10_000),
        max_regen_current_deci_a: valid_u16(le_u16(data, 4)),
        max_discharge_current_deci_a: valid_u16(le_u16(data, 6)),
    }
}

fn parse_insulation_status(data: [u8; 8]) -> InsulationStatus {
    InsulationStatus {
        positive_insulation_kohm: valid_u16(le_u16(data, 0)),
        negative_insulation_kohm: valid_u16(le_u16(data, 2)),
        battery_side_voltage_deci_v: valid_u16(le_u16(data, 4)),
        bus_side_voltage_deci_v: valid_u16(le_u16(data, 6)),
    }
}

fn parse_temperature_summary(data: [u8; 8]) -> TemperatureSummary {
    TemperatureSummary {
        max_cell_temp_c: temp_c(data[0]),
        min_cell_temp_c: temp_c(data[1]),
        avg_cell_temp_c: temp_c(data[2]),
        max_temp_csc: valid_u8(data[3]),
        max_temp_probe: valid_u8(data[4]),
        min_temp_csc: valid_u8(data[5]),
        min_temp_probe: valid_u8(data[6]),
    }
}

fn parse_cell_voltage_max(data: [u8; 8]) -> CellVoltageMaximum {
    CellVoltageMaximum {
        max_cell_voltage_mv: valid_u16(le_u16(data, 0)),
        max_cell_csc: valid_u8(data[2]),
        max_cell_index: valid_u8(data[3]),
        avg_cell_voltage_mv: valid_u16(le_u16(data, 4)),
    }
}

fn parse_cell_voltage_min(data: [u8; 8]) -> CellVoltageMinimum {
    CellVoltageMinimum {
        min_cell_voltage_mv: valid_u16(le_u16(data, 0)),
        min_cell_csc: valid_u8(data[2]),
        min_cell_index: valid_u8(data[3]),
        main_positive1_temp_c: temp_c(data[4]),
        main_positive2_temp_c: temp_c(data[5]),
        main_negative1_temp_c: temp_c(data[6]),
        main_negative2_temp_c: temp_c(data[7]),
    }
}

fn parse_charge_connector_temperatures(data: [u8; 8]) -> ChargeConnectorTemperatures {
    ChargeConnectorTemperatures {
        temperatures_c: [
            temp_c(data[0]),
            temp_c(data[1]),
            temp_c(data[2]),
            temp_c(data[3]),
            temp_c(data[4]),
            temp_c(data[5]),
            temp_c(data[6]),
            temp_c(data[7]),
        ],
    }
}

fn parse_version_info(data: [u8; 8]) -> VersionInfo {
    let mut bytes = [0xff; 7];
    bytes.copy_from_slice(&data[1..8]);
    VersionInfo {
        kind: data[0],
        bytes,
    }
}

fn parse_water_loop_temperatures(data: [u8; 8]) -> WaterLoopTemperatures {
    WaterLoopTemperatures {
        sequence: data[0],
        pack1_or_3_out_deci_c: temp_deci_c_12(
            u16::from(data[1]) | (u16::from(data[2] & 0x0f) << 8),
        ),
        pack1_or_3_in_deci_c: temp_deci_c_12(u16::from(data[3]) | (u16::from(data[2] >> 4) << 8)),
        pack2_or_4_in_deci_c: temp_deci_c_12(u16::from(data[4]) | (u16::from(data[5] & 0x0f) << 8)),
        pack2_or_4_out_deci_c: temp_deci_c_12(u16::from(data[6]) | (u16::from(data[5] >> 4) << 8)),
    }
}

fn parse_extended_relay_status(data: [u8; 8]) -> ExtendedRelayStatus {
    ExtendedRelayStatus {
        main_negative2_state: bits(data[0], 6, 0b11),
        main_positive2_state: bits(data[0], 4, 0b11),
        charge_negative3_state: bits(data[1], 6, 0b11),
        charge_positive3_state: bits(data[1], 4, 0b11),
        charge_negative4_state: bits(data[2], 6, 0b11),
        charge_positive4_state: bits(data[2], 4, 0b11),
        lead_acid_voltage_v: if data[3] & 0x3f == 0x3f {
            None
        } else {
            Some(data[3] & 0x3f)
        },
        bms_charge_allowed_current_deci_a: valid_u16(le_u16(data, 4))
            .map(|raw| i32::from(raw) - 20_000),
        charge_stop_reason: data[6],
    }
}

fn parse_pack_pole_status(data: [u8; 8]) -> PackPoleStatus {
    PackPoleStatus {
        insulation_detection_state: data[0] & 0b11,
        pack_positive_pole_temp_c: temp_c(data[1]),
        pack_positive_index: valid_u8(data[2]),
        pack_negative_pole_temp_c: temp_c(data[3]),
        pack_negative_index: valid_u8(data[4]),
        not_full_charge_count: valid_u8(data[5]),
        cc2_states: [
            bits(data[6], 0, 0b11),
            bits(data[6], 2, 0b11),
            bits(data[6], 4, 0b11),
            bits(data[6], 6, 0b11),
        ],
        thermal_runaway_reason_bits: data[7],
    }
}

fn parse_energy_24(
    data: [u8; 8],
    kind: EnergyCounterKind,
    first: usize,
    second: usize,
    extra: Option<usize>,
) -> EnergyCounter {
    EnergyCounter {
        kind,
        first_deci_kwh: valid_u24(le_u24(data, first)),
        second_deci_kwh: valid_u24(le_u24(data, second)),
        extra: extra.and_then(|idx| valid_u16(le_u16(data, idx)).map(u32::from)),
    }
}

fn parse_energy_32(
    data: [u8; 8],
    kind: EnergyCounterKind,
    first: usize,
    second: usize,
    extra: Option<usize>,
) -> EnergyCounter {
    EnergyCounter {
        kind,
        first_deci_kwh: valid_u32(le_u32(data, first)),
        second_deci_kwh: valid_u32(le_u32(data, second)),
        extra: extra.and_then(|idx| valid_u16(le_u16(data, idx)).map(u32::from)),
    }
}

fn parse_capacity_32(
    data: [u8; 8],
    kind: CapacityCounterKind,
    first: usize,
    second: usize,
) -> CapacityCounter {
    CapacityCounter {
        kind,
        first_deci_ah: valid_u32(le_u32(data, first)),
        second_deci_ah: valid_u32(le_u32(data, second)),
    }
}

fn bits(byte: u8, shift: u8, mask: u8) -> u8 {
    (byte >> shift) & mask
}

fn le_u16(data: [u8; 8], start: usize) -> u16 {
    u16::from(data[start]) | (u16::from(data[start + 1]) << 8)
}

fn le_u24(data: [u8; 8], start: usize) -> u32 {
    u32::from(data[start]) | (u32::from(data[start + 1]) << 8) | (u32::from(data[start + 2]) << 16)
}

fn le_u32(data: [u8; 8], start: usize) -> u32 {
    u32::from(data[start])
        | (u32::from(data[start + 1]) << 8)
        | (u32::from(data[start + 2]) << 16)
        | (u32::from(data[start + 3]) << 24)
}

fn valid_u8(raw: u8) -> Option<u8> {
    if raw == 0xff { None } else { Some(raw) }
}

fn valid_u16(raw: u16) -> Option<u16> {
    valid_u16_except(raw, 0xffff)
}

fn valid_u16_except(raw: u16, invalid: u16) -> Option<u16> {
    if raw == invalid { None } else { Some(raw) }
}

fn valid_u24(raw: u32) -> Option<u32> {
    if raw == 0x00ff_ffff { None } else { Some(raw) }
}

fn valid_u32(raw: u32) -> Option<u32> {
    if raw == 0xffff_ffff { None } else { Some(raw) }
}

fn temp_c(raw: u8) -> Option<i16> {
    valid_u8(raw).map(|value| i16::from(value) - 50)
}

fn temp_deci_c_12(raw: u16) -> Option<i16> {
    if raw == 0x0fff {
        None
    } else {
        Some(raw as i16 - 500)
    }
}
