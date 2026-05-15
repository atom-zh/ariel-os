use crate::remote;
use crate::vehicle::{ACTIVE_PROFILE, VehicleCanFrame, VehicleCanProfile};
use ariel_os::{
    gpio::{Level, Output},
    log::Debug2Format,
    time::{Duration, Timer, with_timeout},
};
use core::fmt::Write as _;
use core::num::{NonZeroU8, NonZeroU16};
use embassy_futures::join::join;
use embassy_stm32::{bind_interrupts, can as stm_can, pac, rcc};
use heapless::String;

type BoardCanMasterPeripheral = ariel_os_boards::can::MasterPeripheral;
type BoardCanPeripheral = ariel_os_boards::can::Peripheral;
type BoardCanRxPin = ariel_os_boards::can::RxPin;
type BoardCanStandbyPin = ariel_os_boards::can::StandbyPin;
type BoardCanTxPin = ariel_os_boards::can::TxPin;

const CAN_BITRATE: u32 = ariel_os_boards::can::BITRATE;
const CAN_SAMPLE_POINT_PERMILLE: u16 = ariel_os_boards::can::SAMPLE_POINT_PERMILLE;
const CAN_TEST_ID: u16 = ariel_os_boards::can::TEST_ID;
const CAN2_FILTER_SPLIT_INDEX: u8 = ariel_os_boards::can::FILTER_SPLIT_INDEX;
const CAN2_FILTER_BANK_INDEX: u8 = ariel_os_boards::can::FILTER_BANK_INDEX;
const CAN_LOOPBACK_TEST_ID: u16 = ariel_os_boards::can::LOOPBACK_TEST_ID;
const CAN_LOOPBACK_TIMEOUT_MS: u64 = ariel_os_boards::can::LOOPBACK_TIMEOUT_MS;

mod irq {
    use super::{BoardCanPeripheral, bind_interrupts, stm_can};

    bind_interrupts!(pub struct CanIrqs {
        CAN2_TX => stm_can::TxInterruptHandler<BoardCanPeripheral>;
        CAN2_RX0 => stm_can::Rx0InterruptHandler<BoardCanPeripheral>;
        CAN2_RX1 => stm_can::Rx1InterruptHandler<BoardCanPeripheral>;
        CAN2_SCE => stm_can::SceInterruptHandler<BoardCanPeripheral>;
    });
}

pub async fn run(
    led_peripherals: ariel_os_boards::pins::LedPeripherals,
    can_peripherals: ariel_os_boards::pins::CanPeripherals,
) {
    let mut led0 = Output::new(led_peripherals.led0, Level::Low);
    let ariel_os_boards::pins::CanPeripherals {
        can_rx,
        can_tx,
        can_standby,
    } = can_peripherals;

    let _can_standby = enable_can_transceiver(can_standby);
    enable_can_shared_master_clock();
    let (can2, can_rx, can_tx) = take_can_resources(can_rx, can_tx);
    let mut can = stm_can::Can::new(can2, can_rx, can_tx, irq::CanIrqs);

    configure_can2_accept_all_filter();
    run_loopback_self_test(&mut can).await;
    configure_can_mode(&mut can, false, false).await;

    info!(
        "{} ready on {}/{}, phy={} stb={}({}), bitrate={}bps, sample-point={}‰, test-id=0x{:x}",
        ariel_os_boards::can::PERIPHERAL,
        ariel_os_boards::can::RX_PIN,
        ariel_os_boards::can::TX_PIN,
        ariel_os_boards::can::PHY,
        ariel_os_boards::can::STANDBY_PIN,
        can_transceiver_normal_level_label(),
        CAN_BITRATE,
        CAN_SAMPLE_POINT_PERMILLE,
        CAN_TEST_ID
    );

    let (mut can_tx, mut can_rx) = can.split();

    let blink_task = async {
        loop {
            led0.toggle();
            Timer::after_millis(500).await;
        }
    };

    let tx_task = async {
        let mut seq: u8 = 0;

        loop {
            let payload = [0x54, 0x42, 0x4f, 0x58, seq, 0x00, 0x00, 0x00];
            match stm_can::Frame::new_standard(CAN_TEST_ID, &payload) {
                Ok(frame) => {
                    let _status = can_tx.write(&frame).await;
                    info!(
                        "CAN TX std id=0x{:x} seq={} dlc={}",
                        CAN_TEST_ID,
                        seq,
                        payload.len()
                    );
                    log_can_bytes("CAN TX", &payload);
                    seq = seq.wrapping_add(1);
                }
                Err(err) => {
                    warn!("CAN TX frame build failed: {:?}", Debug2Format(&err));
                }
            }

            Timer::after_secs(1).await;
        }
    };

    let rx_task = async {
        let mut err_count: u32 = 0;
        loop {
            match can_rx.read().await {
                Ok(envelope) => {
                    err_count = 0;
                    let frame = envelope.frame;
                    let data = frame.data();

                    match frame.id() {
                        stm_can::Id::Standard(id) => {
                            let raw_id = u32::from(id.as_raw());
                            info!("CAN RX std id=0x{:x} dlc={}", raw_id, data.len());
                            parse_vehicle_can(raw_id, data);
                        }
                        stm_can::Id::Extended(id) => {
                            let raw_id = id.as_raw();
                            info!("CAN RX ext id=0x{:x} dlc={}", raw_id, data.len());
                            parse_vehicle_can(raw_id, data);
                        }
                    }

                    log_can_bytes("CAN RX", data);
                }
                Err(err) => {
                    err_count = err_count.saturating_add(1);
                    let mut err_text: String<24> = String::new();
                    let _ = write!(&mut err_text, "{:?}", Debug2Format(&err));

                    if err_count == 1 || err_count % 1000 == 0 {
                        warn!("CAN RX error x{}: {}", err_count, err_text.as_str());
                    }

                    Timer::after_millis(2).await;
                }
            }
        }
    };

    join(join(blink_task, tx_task), rx_task).await;
}

fn parse_vehicle_can(id: u32, data: &[u8]) {
    let frame = VehicleCanFrame::new(id, data);
    match ACTIVE_PROFILE.parse(&frame) {
        Ok(Some(message)) => {
            remote::update_from_vehicle_message(&message);
            crate::tagged_log::info_for_tag(
                "vehicle",
                format_args!(
                    "vehicle[{}] parsed: {:?}",
                    ACTIVE_PROFILE.name(),
                    Debug2Format(&message)
                ),
            );
        }
        Ok(None) => {}
        Err(err) => crate::tagged_log::warn_for_tag(
            "vehicle",
            format_args!(
                "vehicle[{}] parse error: {:?}",
                ACTIVE_PROFILE.name(),
                Debug2Format(&err)
            ),
        ),
    }
}

fn can_timing_250k_889() -> stm_can::util::NominalBitTiming {
    // APB1 = 45MHz 时：
    // tq = prescaler / 45MHz = 10 / 45MHz
    // bit = 1 + seg1 + seg2 = 18 tq  => 45MHz / (10 * 18) = 250kbps
    // sample point = (1 + seg1) / 18 = (1 + 15) / 18 = 88.9%
    // 说明：45MHz 下无法整除得到精确 87.5% 采样点，这里选择最接近且稳定的一组参数。
    stm_can::util::NominalBitTiming {
        prescaler: NonZeroU16::new(10).expect("non-zero prescaler"),
        seg1: NonZeroU8::new(15).expect("non-zero seg1"),
        seg2: NonZeroU8::new(2).expect("non-zero seg2"),
        sync_jump_width: NonZeroU8::new(1).expect("non-zero sjw"),
    }
}

async fn configure_can_mode(can: &mut stm_can::Can<'_>, loopback: bool, silent: bool) {
    can.modify_config().set_bit_timing(can_timing_250k_889());
    can.modify_config()
        .set_loopback(loopback)
        .set_silent(silent);
    can.enable().await;
}

fn enable_can_transceiver(
    standby_pin: embassy_stm32::Peri<'static, BoardCanStandbyPin>,
) -> Output<'static> {
    let normal_level = can_transceiver_normal_level();
    info!(
        "CAN PHY {} standby pin {} -> {} before CAN init",
        ariel_os_boards::can::PHY,
        ariel_os_boards::can::STANDBY_PIN,
        can_transceiver_normal_level_label()
    );
    Output::new(standby_pin, normal_level)
}

fn can_transceiver_normal_level() -> Level {
    if ariel_os_boards::can::STANDBY_NORMAL_LEVEL_HIGH {
        Level::High
    } else {
        Level::Low
    }
}

fn can_transceiver_normal_level_label() -> &'static str {
    if ariel_os_boards::can::STANDBY_NORMAL_LEVEL_HIGH {
        "high"
    } else {
        "low"
    }
}

fn enable_can_shared_master_clock() {
    // STM32F4 bxCAN2 is a slave instance: filters/SRAM are shared with CAN1.
    // Keep the CAN1 clock enabled before touching shared filter registers or CAN2.
    rcc::enable_and_reset::<BoardCanMasterPeripheral>();
}

async fn run_loopback_self_test(can: &mut stm_can::Can<'_>) {
    configure_can_mode(can, true, false).await;

    let payload = [0x4c, 0x42, 0x54, 0x45, 0x53, 0x54, 0x00, 0x01];
    let frame = match stm_can::Frame::new_standard(CAN_LOOPBACK_TEST_ID, &payload) {
        Ok(frame) => frame,
        Err(err) => {
            warn!("CAN loopback frame build failed: {:?}", Debug2Format(&err));
            return;
        }
    };

    let _ = can.write(&frame).await;
    match with_timeout(Duration::from_millis(CAN_LOOPBACK_TIMEOUT_MS), can.read()).await {
        Ok(Ok(envelope)) => {
            let id_ok = matches!(
                envelope.frame.id(),
                stm_can::Id::Standard(id) if id.as_raw() == CAN_LOOPBACK_TEST_ID
            );
            if id_ok {
                info!("CAN loopback self-test: PASS");
            } else {
                warn!("CAN loopback self-test: RX ID mismatch");
            }
        }
        Ok(Err(err)) => warn!("CAN loopback self-test bus error: {:?}", Debug2Format(&err)),
        Err(_) => warn!("CAN loopback self-test timeout"),
    }
}

fn configure_can2_accept_all_filter() {
    // bxCAN (CAN1/CAN2) shares one filter-bank block. CAN2 uses the "slave" range,
    // whose start index is CAN2SB. We reserve at least one bank for CAN2.
    pac::CAN1.fmr().modify(|reg| reg.set_finit(true));
    pac::CAN1
        .fmr()
        .modify(|reg| reg.set_can2sb(CAN2_FILTER_SPLIT_INDEX));

    // Configure bank 13 as 32-bit mask filter, accept all, route to FIFO0.
    pac::CAN1
        .fm1r()
        .modify(|reg| reg.set_fbm(CAN2_FILTER_BANK_INDEX as usize, false));
    pac::CAN1
        .fs1r()
        .modify(|reg| reg.set_fsc(CAN2_FILTER_BANK_INDEX as usize, true));

    let bank = pac::CAN1.fb(CAN2_FILTER_BANK_INDEX as usize);
    bank.fr1().write(|w| w.0 = 0);
    bank.fr2().write(|w| w.0 = 0);

    pac::CAN1
        .ffa1r()
        .modify(|reg| reg.set_ffa(CAN2_FILTER_BANK_INDEX as usize, false));
    pac::CAN1
        .fa1r()
        .modify(|reg| reg.set_fact(CAN2_FILTER_BANK_INDEX as usize, true));

    pac::CAN1.fmr().modify(|reg| reg.set_finit(false));
}

fn take_can_resources(
    can_rx: embassy_stm32::Peri<'static, BoardCanRxPin>,
    can_tx: embassy_stm32::Peri<'static, BoardCanTxPin>,
) -> (
    embassy_stm32::Peri<'static, BoardCanPeripheral>,
    embassy_stm32::Peri<'static, BoardCanRxPin>,
    embassy_stm32::Peri<'static, BoardCanTxPin>,
) {
    (ariel_os_boards::can::steal_peripheral(), can_rx, can_tx)
}

fn log_can_bytes(prefix: &str, bytes: &[u8]) {
    const PREVIEW: usize = 48;
    let preview_len = bytes.len().min(PREVIEW);

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

    let mut hex: String<{ PREVIEW * 3 }> = String::new();
    for &byte in &bytes[..preview_len] {
        let _ = write!(hex, "{:02X} ", byte);
    }

    info!(
        "{} (len={}, shown={}): [{}] ascii=\"{}\"",
        prefix,
        bytes.len(),
        preview_len,
        hex.as_str(),
        ascii.as_str()
    );
}
