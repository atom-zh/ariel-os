#![no_main]
#![no_std]

#[macro_use]
mod tagged_log;
mod can;
mod modem;
mod pins;
mod tcp_client;

use ariel_os::{
    gpio::{Level, Output},
    time::Timer,
};
use embassy_futures::join::join;
use embassy_stm32::{bind_interrupts, peripherals, usart};

const MODEM_POWER_OFF_WAIT_MS: u64 = 1_000;
const MODEM_RETRY_WAIT_MS: u64 = 1_000;
const MODEM_FIXED_BAUD: u32 = 921_600;

mod irq {
    use super::{bind_interrupts, peripherals, usart};

    bind_interrupts!(pub struct ModemIrqs {
        USART3 => usart::InterruptHandler<peripherals::USART3>;
    });
}

#[ariel_os::task(autostart, peripherals)]
async fn main(peripherals: pins::Peripherals) {
    let power_active_high = ariel_os_boards::modem::POWER_ACTIVE_HIGH;
    let pwrkey_active_high = ariel_os_boards::modem::PWRKEY_ACTIVE_HIGH;

    let mut modem_power = Output::new(
        peripherals.modem.modem_power,
        inactive_level(power_active_high),
    );
    let mut modem_pwrkey = Output::new(
        peripherals.modem.modem_pwrkey,
        inactive_level(pwrkey_active_high),
    );

    let configured_baudrate = MODEM_FIXED_BAUD;
    let mut config = embassy_stm32::usart::Config::default();
    config.baudrate = configured_baudrate;

    let (uart_peri, tx_dma, rx_dma) = take_modem_uart_resources();

    let uart = embassy_stm32::usart::Uart::new(
        uart_peri,
        peripherals.modem.modem_rx,
        peripherals.modem.modem_tx,
        irq::ModemIrqs,
        tx_dma,
        rx_dma,
        config,
    )
    .expect("invalid DMA UART configuration for modem");

    info!(
        "tbox starting: {} on {} @ {}bps (dma requested: {})",
        ariel_os_boards::modem::MODEL,
        ariel_os_boards::modem::UART,
        configured_baudrate,
        ariel_os_boards::modem::UART_DMA,
    );

    let mut uart = modem::DmaUart::new(uart);

    let modem_task = async {
        loop {
            power_cycle_modem(
                &mut modem_power,
                power_active_high,
                &mut modem_pwrkey,
                pwrkey_active_high,
            )
            .await;

            let outcome = modem::run(&mut uart).await;

            match outcome {
                modem::RunOutcome::RetryPowerCycle => {
                    warn!("EC800M session failed, power-cycling and retrying");
                    Timer::after_millis(MODEM_RETRY_WAIT_MS).await;
                }
            }
        }
    };

    join(can::run(peripherals.led, peripherals.can), modem_task).await;
}

fn inactive_level(active_high: bool) -> Level {
    if active_high { Level::Low } else { Level::High }
}

fn active_level(active_high: bool) -> Level {
    if active_high { Level::High } else { Level::Low }
}

async fn power_cycle_modem(
    modem_power: &mut Output<'_>,
    power_active_high: bool,
    modem_pwrkey: &mut Output<'_>,
    pwrkey_active_high: bool,
) {
    info!("powering off EC800M for {}ms", MODEM_POWER_OFF_WAIT_MS);
    modem_power.set_level(inactive_level(power_active_high));
    modem_pwrkey.set_level(inactive_level(pwrkey_active_high));
    Timer::after_millis(MODEM_POWER_OFF_WAIT_MS).await;

    info!("powering on EC800M");
    modem_power.set_level(active_level(power_active_high));
    Timer::after_millis(100).await;

    info!(
        "pulsing EC800M PWRKEY for {}ms",
        ariel_os_boards::modem::PWRKEY_PULSE_MS
    );
    modem_pwrkey.set_level(active_level(pwrkey_active_high));
    Timer::after_millis(ariel_os_boards::modem::PWRKEY_PULSE_MS).await;
    modem_pwrkey.set_level(inactive_level(pwrkey_active_high));
}

#[expect(
    unsafe_code,
    reason = "temporary direct take for DMA modem UART on stm32f427vg"
)]
fn take_modem_uart_resources() -> (
    embassy_stm32::Peri<'static, peripherals::USART3>,
    embassy_stm32::Peri<'static, peripherals::DMA1_CH3>,
    embassy_stm32::Peri<'static, peripherals::DMA1_CH1>,
) {
    unsafe {
        (
            peripherals::USART3::steal(),
            peripherals::DMA1_CH3::steal(),
            peripherals::DMA1_CH1::steal(),
        )
    }
}
