use core::cell::RefCell;
use critical_section::Mutex;

use crate::vehicle::{
    BatteryIdentityChunk, CapacityCounterKind, EnergyCounterKind, VehicleMessage, VinChunk,
};

static STATE: Mutex<RefCell<RemoteState>> = Mutex::new(RefCell::new(RemoteState::new()));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteSnapshot {
    pub soh_percent: Option<u8>,
    pub soc_percent: Option<u8>,
    pub vehicle_state: Option<u8>,
    pub key_state: Option<u8>,
    pub shift: Option<u8>,
    pub speed_kph: Option<u8>,
    pub total_mileage_deci_km: Option<u32>,
    pub handbrake: Option<u8>,
    pub battery_voltage_deci_v: Option<u16>,
    pub bus_voltage_deci_v: Option<u16>,
    pub total_current_deci_a: Option<i32>,
    pub positive_insulation_kohm: Option<u16>,
    pub negative_insulation_kohm: Option<u16>,
    pub max_cell_temp_c: Option<i16>,
    pub min_cell_temp_c: Option<i16>,
    pub avg_cell_temp_c: Option<i16>,
    pub max_temp_csc: Option<u8>,
    pub max_temp_probe: Option<u8>,
    pub min_temp_csc: Option<u8>,
    pub min_temp_probe: Option<u8>,
    pub max_cell_voltage_mv: Option<u16>,
    pub avg_cell_voltage_mv: Option<u16>,
    pub min_cell_voltage_mv: Option<u16>,
    pub max_cell_csc: Option<u8>,
    pub max_cell_index: Option<u8>,
    pub min_cell_csc: Option<u8>,
    pub min_cell_index: Option<u8>,
    pub main_negative_relay_state: Option<u8>,
    pub bms_version: [u8; 6],
    pub total_charge_deci_kwh: Option<u32>,
    pub total_discharge_deci_kwh: Option<u32>,
    pub total_plug_charge_deci_kwh: Option<u32>,
    pub total_swap_charge_deci_kwh: Option<u32>,
    pub total_regen_deci_kwh: Option<u32>,
    pub battery_manufacturer: Option<u16>,
    pub battery_sn_len: Option<u8>,
    pub battery_sn: [u8; 27],
    pub vin: [u8; 17],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RemoteState {
    snapshot: RemoteSnapshot,
}

impl RemoteState {
    const fn new() -> Self {
        Self {
            snapshot: RemoteSnapshot {
                soh_percent: None,
                soc_percent: None,
                vehicle_state: None,
                key_state: None,
                shift: None,
                speed_kph: None,
                total_mileage_deci_km: None,
                handbrake: None,
                battery_voltage_deci_v: None,
                bus_voltage_deci_v: None,
                total_current_deci_a: None,
                positive_insulation_kohm: None,
                negative_insulation_kohm: None,
                max_cell_temp_c: None,
                min_cell_temp_c: None,
                avg_cell_temp_c: None,
                max_temp_csc: None,
                max_temp_probe: None,
                min_temp_csc: None,
                min_temp_probe: None,
                max_cell_voltage_mv: None,
                avg_cell_voltage_mv: None,
                min_cell_voltage_mv: None,
                max_cell_csc: None,
                max_cell_index: None,
                min_cell_csc: None,
                min_cell_index: None,
                main_negative_relay_state: None,
                bms_version: [0xff; 6],
                total_charge_deci_kwh: None,
                total_discharge_deci_kwh: None,
                total_plug_charge_deci_kwh: None,
                total_swap_charge_deci_kwh: None,
                total_regen_deci_kwh: None,
                battery_manufacturer: None,
                battery_sn_len: None,
                battery_sn: [0xff; 27],
                vin: [0xff; 17],
            },
        }
    }

    fn apply(&mut self, message: &VehicleMessage) {
        match *message {
            VehicleMessage::BatteryIdentityChunk(chunk) => self.apply_battery_identity(chunk),
            VehicleMessage::BmsStatus0(status) => {
                self.snapshot.main_negative_relay_state = Some(status.main_negative_relay_state);
                self.snapshot.vehicle_state = Some(status.rechargeable_system_index);
            }
            VehicleMessage::BmsStatus1(status) => {
                self.snapshot.vehicle_state = Some(status.bms_state);
            }
            VehicleMessage::BatteryRealtime(realtime) => {
                self.snapshot.soc_percent = realtime.soc_deci_percent.map(deci_percent_to_percent);
                self.snapshot.soh_percent = realtime.soh_deci_percent.map(deci_percent_to_percent);
                self.snapshot.total_current_deci_a = realtime.pack_current_deci_a;
            }
            VehicleMessage::InsulationStatus(status) => {
                self.snapshot.positive_insulation_kohm = status.positive_insulation_kohm;
                self.snapshot.negative_insulation_kohm = status.negative_insulation_kohm;
                self.snapshot.battery_voltage_deci_v = status.battery_side_voltage_deci_v;
                self.snapshot.bus_voltage_deci_v = status.bus_side_voltage_deci_v;
            }
            VehicleMessage::TemperatureSummary(summary) => {
                self.snapshot.max_cell_temp_c = summary.max_cell_temp_c;
                self.snapshot.min_cell_temp_c = summary.min_cell_temp_c;
                self.snapshot.avg_cell_temp_c = summary.avg_cell_temp_c;
                self.snapshot.max_temp_csc = summary.max_temp_csc;
                self.snapshot.max_temp_probe = summary.max_temp_probe;
                self.snapshot.min_temp_csc = summary.min_temp_csc;
                self.snapshot.min_temp_probe = summary.min_temp_probe;
            }
            VehicleMessage::CellVoltageMaximum(maximum) => {
                self.snapshot.max_cell_voltage_mv = maximum.max_cell_voltage_mv;
                self.snapshot.avg_cell_voltage_mv = maximum.avg_cell_voltage_mv;
                self.snapshot.max_cell_csc = maximum.max_cell_csc;
                self.snapshot.max_cell_index = maximum.max_cell_index;
            }
            VehicleMessage::CellVoltageMinimum(minimum) => {
                self.snapshot.min_cell_voltage_mv = minimum.min_cell_voltage_mv;
                self.snapshot.min_cell_csc = minimum.min_cell_csc;
                self.snapshot.min_cell_index = minimum.min_cell_index;
            }
            VehicleMessage::VersionInfo(version) if version.kind == 1 => {
                self.snapshot
                    .bms_version
                    .copy_from_slice(&version.bytes[..6]);
            }
            VehicleMessage::EnergyCounter(counter) => match counter.kind {
                EnergyCounterKind::ChargeDischarge24 | EnergyCounterKind::ChargeDischarge32 => {
                    self.snapshot.total_charge_deci_kwh = counter.first_deci_kwh;
                    self.snapshot.total_discharge_deci_kwh = counter.second_deci_kwh;
                }
                EnergyCounterKind::RegenSwap24 | EnergyCounterKind::RegenSwap32 => {
                    self.snapshot.total_regen_deci_kwh = counter.first_deci_kwh;
                    self.snapshot.total_swap_charge_deci_kwh = counter.second_deci_kwh;
                }
                EnergyCounterKind::PlugCharge24 | EnergyCounterKind::PlugAndSingleCharge32 => {
                    self.snapshot.total_plug_charge_deci_kwh = counter.first_deci_kwh;
                }
                EnergyCounterKind::SingleRegen32 => {}
            },
            VehicleMessage::CapacityCounter(counter) => match counter.kind {
                CapacityCounterKind::ChargeDischarge32
                | CapacityCounterKind::RegenSwap32
                | CapacityCounterKind::PlugAndSingleCharge32 => {}
            },
            VehicleMessage::VehicleInfo(info) => {
                self.snapshot.total_mileage_deci_km = info.total_mileage_deci_km;
                self.snapshot.speed_kph =
                    info.speed_1_256_kph.map(|raw| (raw / 256).min(254) as u8);
                self.snapshot.shift = Some(match info.shift {
                    1 => 1,
                    2 => 2,
                    3 => 3,
                    4 => 4,
                    _ => 0,
                });
                self.snapshot.handbrake = Some(if info.handbrake == 1 { 1 } else { 0 });
            }
            VehicleMessage::VinChunk(chunk) => self.apply_vin_chunk(chunk),
            _ => {}
        }
    }

    fn apply_battery_identity(&mut self, chunk: BatteryIdentityChunk) {
        if let Some(manufacturer) = chunk.manufacturer {
            if manufacturer != 0 {
                self.snapshot.battery_manufacturer = Some(manufacturer);
            }
        }
        if let Some(len) = chunk.declared_len {
            if len != 0 {
                self.snapshot.battery_sn_len = Some(len.min(27));
            }
        }

        let offset = match chunk.chunk_index {
            1 => 0,
            2 => 6,
            3 => 13,
            4 => 20,
            _ => return,
        };
        copy_partial(
            &mut self.snapshot.battery_sn,
            offset,
            &chunk.bytes,
            chunk.valid_bytes,
        );
    }

    fn apply_vin_chunk(&mut self, chunk: VinChunk) {
        let offset = match chunk.frame_seq {
            1 => 0,
            2 => 7,
            3 => 14,
            _ => return,
        };
        copy_partial(
            &mut self.snapshot.vin,
            offset,
            &chunk.bytes,
            chunk.valid_bytes,
        );
    }
}

pub fn update_from_vehicle_message(message: &VehicleMessage) {
    critical_section::with(|cs| STATE.borrow(cs).borrow_mut().apply(message));
}

pub fn snapshot() -> RemoteSnapshot {
    critical_section::with(|cs| STATE.borrow(cs).borrow().snapshot)
}

fn copy_partial<const N: usize, const M: usize>(
    dst: &mut [u8; N],
    offset: usize,
    src: &[u8; M],
    len: usize,
) {
    let count = len.min(M).min(N.saturating_sub(offset));
    if count > 0 {
        dst[offset..offset + count].copy_from_slice(&src[..count]);
    }
}

fn deci_percent_to_percent(value: u16) -> u8 {
    (value / 10).min(100) as u8
}
