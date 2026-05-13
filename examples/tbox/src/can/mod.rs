use ariel_os::{
    gpio::{Level, Output},
    log::Debug2Format,
    time::{Duration, Timer, with_timeout},
};
use core::fmt::Write as _;
use core::num::{NonZeroU8, NonZeroU16};
use embassy_futures::join::join;
use embassy_stm32::{bind_interrupts, can as stm_can, pac, peripherals};
use heapless::String;

const CAN_BITRATE: u32 = 250_000;
const CAN_SAMPLE_POINT_PERMILLE: u16 = 875;
const CAN_TEST_ID: u16 = 0x123;
const CAN2_FILTER_SPLIT_INDEX: u8 = 13;
const CAN2_FILTER_BANK_INDEX: u8 = 13;
const CAN_LOOPBACK_TEST_ID: u16 = 0x321;
const CAN_LOOPBACK_TIMEOUT_MS: u64 = 300;

mod irq {
    use super::{bind_interrupts, peripherals, stm_can};

    bind_interrupts!(pub struct CanIrqs {
        CAN2_TX => stm_can::TxInterruptHandler<peripherals::CAN2>;
        CAN2_RX0 => stm_can::Rx0InterruptHandler<peripherals::CAN2>;
        CAN2_RX1 => stm_can::Rx1InterruptHandler<peripherals::CAN2>;
        CAN2_SCE => stm_can::SceInterruptHandler<peripherals::CAN2>;
    });
}

pub async fn run(led_peripherals: ariel_os_boards::pins::LedPeripherals) {
    let mut led0 = Output::new(led_peripherals.led0, Level::Low);

    let (can2, can_rx, can_tx) = take_can2_resources();
    let mut can = stm_can::Can::new(can2, can_rx, can_tx, irq::CanIrqs);

    configure_can2_accept_all_filter();
    run_loopback_self_test(&mut can).await;
    configure_can_mode(&mut can, false, false).await;

    info!(
        "CAN2 ready on PB12/PB13, bitrate={}bps, sample-point={}‰, test-id=0x{:x}",
        CAN_BITRATE, CAN_SAMPLE_POINT_PERMILLE, CAN_TEST_ID
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
                            info!("CAN RX std id=0x{:x} dlc={}", id.as_raw(), data.len())
                        }
                        stm_can::Id::Extended(id) => {
                            info!("CAN RX ext id=0x{:x} dlc={}", id.as_raw(), data.len())
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

fn can_timing_250k_875() -> stm_can::util::NominalBitTiming {
    // APB1 = 42MHz 时：250kbps, 87.5% 采样点可用一组显式参数：
    // tq = (prescaler / 42MHz) = 21 / 42MHz
    // bit = 1 + seg1 + seg2 = 8 tq  => 42MHz / (21 * 8) = 250kbps
    // sample point = (1 + seg1) / 8 = (1 + 6) / 8 = 87.5%
    stm_can::util::NominalBitTiming {
        prescaler: NonZeroU16::new(21).expect("non-zero prescaler"),
        seg1: NonZeroU8::new(6).expect("non-zero seg1"),
        seg2: NonZeroU8::new(1).expect("non-zero seg2"),
        sync_jump_width: NonZeroU8::new(1).expect("non-zero sjw"),
    }
}

async fn configure_can_mode(can: &mut stm_can::Can<'_>, loopback: bool, silent: bool) {
    can.modify_config().set_bit_timing(can_timing_250k_875());
    can.modify_config()
        .set_loopback(loopback)
        .set_silent(silent);
    can.enable().await;
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

#[expect(
    unsafe_code,
    reason = "temporary direct take for CAN2 PB12/PB13 debug on stm32f427vg"
)]
fn take_can2_resources() -> (
    embassy_stm32::Peri<'static, peripherals::CAN2>,
    embassy_stm32::Peri<'static, peripherals::PB12>,
    embassy_stm32::Peri<'static, peripherals::PB13>,
) {
    unsafe {
        (
            peripherals::CAN2::steal(),
            peripherals::PB12::steal(),
            peripherals::PB13::steal(),
        )
    }
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
