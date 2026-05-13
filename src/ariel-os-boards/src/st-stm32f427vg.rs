// @generated

pub mod pins {
    use ariel_os_hal::hal::peripherals;
    ariel_os_hal::define_peripherals!(LedPeripherals { led0: PE9, });
    ariel_os_hal::define_peripherals!(ButtonPeripherals { button0: PC13, });
    ariel_os_hal::define_peripherals!(
        ModemPeripherals {
            modem_power: PD15,
            modem_pwrkey: PD13,
            modem_rx: PD9,
            modem_tx: PD8,
        }
    );
}

pub mod modem {
    pub const MODEL: &str = "EC800M";
    pub const UART: &str = "USART3";
    pub const BAUDRATE: u32 = 921_600;
    pub const UART_DMA: bool = true;
    pub const POWER_ACTIVE_HIGH: bool = false;
    pub const PWRKEY_ACTIVE_HIGH: bool = false;
    pub const PWRKEY_PULSE_MS: u64 = 1_000;
}

pub mod tbox_log {
    pub mod kernel {
        pub const TAG: &str = "kernel";
        pub const ENABLED: bool = true;
    }

    pub mod modem {
        pub const TAG: &str = "modem";
        pub const ENABLED: bool = false;
    }

    pub mod can {
        pub const TAG: &str = "can";
        pub const ENABLED: bool = true;
    }
}

#[allow(unused_variables)]
pub fn init(peripherals: &mut ariel_os_hal::hal::OptionalPeripherals) {}
