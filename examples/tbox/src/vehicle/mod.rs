//! Vehicle CAN protocol parsing.
//!
//! Keep the application-facing API stable and put vehicle/model-specific details
//! behind [`VehicleCanProfile`]. To support another vehicle, add a new profile
//! module and switch [`ACTIVE_PROFILE`].

pub mod bms_v46;

pub use bms_v46::BmsV46Profile;

/// The currently selected vehicle CAN profile.
pub static ACTIVE_PROFILE: BmsV46Profile = BmsV46Profile;

/// One CAN frame normalized for protocol parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VehicleCanFrame {
    pub id: u32,
    pub data: [u8; 8],
    pub len: usize,
}

impl VehicleCanFrame {
    pub fn new(id: u32, data: &[u8]) -> Self {
        let mut bytes = [0xff; 8];
        let len = data.len().min(bytes.len());
        bytes[..len].copy_from_slice(&data[..len]);
        Self {
            id,
            data: bytes,
            len,
        }
    }
}

/// A replaceable vehicle CAN parser profile.
pub trait VehicleCanProfile {
    fn name(&self) -> &'static str;
    fn parse(&self, frame: &VehicleCanFrame) -> Result<Option<VehicleMessage>, ParseError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    InvalidDlc {
        id: u32,
        expected: usize,
        actual: usize,
    },
    Checksum {
        id: u32,
        expected: u8,
        actual: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VehicleMessage {
    BmsCommandStatus(BmsCommandStatus),
    BatteryIdentityChunk(BatteryIdentityChunk),
    BatteryNameplate(BatteryNameplate),
    BatteryPackLayout(BatteryPackLayout),
    TemperatureSamples(TemperatureSamples),
    CellVoltageSamples(CellVoltageSamples),
    PackVoltage(ExpandablePackVoltage),
    BmsStatus0(BmsStatus0),
    BmsStatus1(BmsStatus1),
    BatteryRealtime(BatteryRealtime),
    InsulationStatus(InsulationStatus),
    TemperatureSummary(TemperatureSummary),
    CellVoltageMaximum(CellVoltageMaximum),
    CellVoltageMinimum(CellVoltageMinimum),
    ChargeConnectorTemperatures(ChargeConnectorTemperatures),
    VersionInfo(VersionInfo),
    WaterLoopTemperatures(WaterLoopTemperatures),
    ExtendedRelayStatus(ExtendedRelayStatus),
    PackPoleStatus(PackPoleStatus),
    EnergyCounter(EnergyCounter),
    CapacityCounter(CapacityCounter),
    VehicleInfo(VehicleInfo),
    VinChunk(VinChunk),
    KnownReserved { id: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmsCommandStatus {
    /// 0.1 A, charge negative, discharge positive.
    pub pack_current_deci_a: Option<i32>,
    pub full_charge_request: u8,
    pub power_down_request: u8,
    pub special_charge_discharge_state: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryIdentityChunk {
    pub chunk_index: u8,
    pub declared_len: Option<u8>,
    pub manufacturer: Option<u16>,
    pub bytes: [u8; 7],
    pub valid_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryNameplate {
    pub rated_capacity_ah: Option<u16>,
    pub rated_voltage_deci_v: Option<u16>,
    pub rated_energy_deci_kwh: Option<u16>,
    pub a_plus_wakeup: bool,
    pub key_on_wakeup: bool,
    pub cooling_mode: u8,
    pub chemistry: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryPackLayout {
    pub csc_count: Option<u8>,
    pub cell_count: Option<u16>,
    pub temperature_probe_count: Option<u16>,
    pub nominal_cell_voltage_deci_v: Option<u8>,
    pub min_cell_voltage_deci_v: Option<u8>,
    pub max_cell_voltage_deci_v: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemperatureSamples {
    pub id: u32,
    pub sequence: Option<u8>,
    pub csc: Option<u8>,
    /// ℃, invalid samples are `None`.
    pub temperatures_c: [Option<i16>; 6],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVoltageSamples {
    pub id: u32,
    pub sequence: Option<u8>,
    pub csc: Option<u8>,
    /// millivolts, invalid samples are `None`.
    pub voltages_mv: [Option<u16>; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandablePackVoltage {
    pub pack_index: u8,
    pub pack_voltage_deci_v: Option<u16>,
    pub min_parallel_unit_mv: Option<u16>,
    pub min_parallel_unit_count: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmsStatus0 {
    pub rechargeable_system_index: u8,
    pub main_positive_relay_state: u8,
    pub main_negative_relay_state: u8,
    pub precharge_relay_state: u8,
    pub charge_positive_relay1_state: u8,
    pub charge_negative_relay1_state: u8,
    pub system_fault_code: u8,
    pub raw: [u8; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmsStatus1 {
    pub balance_active: bool,
    pub accessory_relay_closed: bool,
    pub bms_state: u8,
    pub alive_counter: u8,
    pub swap_station_charge_connected: bool,
    pub max_alarm_level: u8,
    pub charge_state: u8,
    pub charge_mode: u8,
    pub charge_connector_connected: bool,
    pub gb32960_fault_count: u8,
    pub thermal_runaway_alarm: bool,
    pub smoke_alarm: bool,
    pub hv_interlock_alarm: bool,
    pub raw: [u8; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryRealtime {
    pub soc_deci_percent: Option<u16>,
    pub soh_deci_percent: Option<u16>,
    /// 0.1 A, charge negative, discharge positive.
    pub pack_current_deci_a: Option<i32>,
    pub max_regen_current_deci_a: Option<u16>,
    pub max_discharge_current_deci_a: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InsulationStatus {
    pub positive_insulation_kohm: Option<u16>,
    pub negative_insulation_kohm: Option<u16>,
    pub battery_side_voltage_deci_v: Option<u16>,
    pub bus_side_voltage_deci_v: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemperatureSummary {
    pub max_cell_temp_c: Option<i16>,
    pub min_cell_temp_c: Option<i16>,
    pub avg_cell_temp_c: Option<i16>,
    pub max_temp_csc: Option<u8>,
    pub max_temp_probe: Option<u8>,
    pub min_temp_csc: Option<u8>,
    pub min_temp_probe: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVoltageMaximum {
    pub max_cell_voltage_mv: Option<u16>,
    pub max_cell_csc: Option<u8>,
    pub max_cell_index: Option<u8>,
    pub avg_cell_voltage_mv: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVoltageMinimum {
    pub min_cell_voltage_mv: Option<u16>,
    pub min_cell_csc: Option<u8>,
    pub min_cell_index: Option<u8>,
    pub main_positive1_temp_c: Option<i16>,
    pub main_positive2_temp_c: Option<i16>,
    pub main_negative1_temp_c: Option<i16>,
    pub main_negative2_temp_c: Option<i16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChargeConnectorTemperatures {
    /// A1, A2, B1, B2, C1, C2, D1, D2 in ℃.
    pub temperatures_c: [Option<i16>; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionInfo {
    pub kind: u8,
    pub bytes: [u8; 7],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaterLoopTemperatures {
    pub sequence: u8,
    /// 0.1 ℃ fixed point.
    pub pack1_or_3_out_deci_c: Option<i16>,
    pub pack1_or_3_in_deci_c: Option<i16>,
    pub pack2_or_4_in_deci_c: Option<i16>,
    pub pack2_or_4_out_deci_c: Option<i16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtendedRelayStatus {
    pub main_negative2_state: u8,
    pub main_positive2_state: u8,
    pub charge_negative3_state: u8,
    pub charge_positive3_state: u8,
    pub charge_negative4_state: u8,
    pub charge_positive4_state: u8,
    pub lead_acid_voltage_v: Option<u8>,
    pub bms_charge_allowed_current_deci_a: Option<i32>,
    pub charge_stop_reason: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackPoleStatus {
    pub insulation_detection_state: u8,
    pub pack_positive_pole_temp_c: Option<i16>,
    pub pack_positive_index: Option<u8>,
    pub pack_negative_pole_temp_c: Option<i16>,
    pub pack_negative_index: Option<u8>,
    pub not_full_charge_count: Option<u8>,
    pub cc2_states: [u8; 4],
    pub thermal_runaway_reason_bits: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyCounterKind {
    ChargeDischarge24,
    RegenSwap24,
    PlugCharge24,
    ChargeDischarge32,
    RegenSwap32,
    PlugAndSingleCharge32,
    SingleRegen32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnergyCounter {
    pub kind: EnergyCounterKind,
    /// 0.1 kWh fixed point. Meaning depends on `kind`.
    pub first_deci_kwh: Option<u32>,
    /// 0.1 kWh fixed point. Meaning depends on `kind`.
    pub second_deci_kwh: Option<u32>,
    /// 0.1 kWh or minutes for `SingleRegen32` second extra field.
    pub extra: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityCounterKind {
    ChargeDischarge32,
    RegenSwap32,
    PlugAndSingleCharge32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityCounter {
    pub kind: CapacityCounterKind,
    /// 0.1 Ah fixed point. Meaning depends on `kind`.
    pub first_deci_ah: Option<u32>,
    /// 0.1 Ah fixed point. Meaning depends on `kind`.
    pub second_deci_ah: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VehicleInfo {
    /// 0.1 km fixed point from DBC `VehicleInfo.TotalMileage`.
    pub total_mileage_deci_km: Option<u32>,
    /// 1/256 km/h fixed point from DBC `VehicleInfo.Speed`.
    pub speed_1_256_kph: Option<u16>,
    pub shift: u8,
    pub handbrake: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VinChunk {
    pub frame_seq: u8,
    pub bytes: [u8; 7],
    pub valid_bytes: usize,
}
