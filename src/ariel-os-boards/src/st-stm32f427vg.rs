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
    ariel_os_hal::define_peripherals!(
        CanPeripherals {
            can_rx: PB12,
            can_tx: PB13,
            can_standby: PB4,
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

pub mod can {
    pub const PERIPHERAL: &str = "CAN2";
    pub const MASTER_PERIPHERAL: &str = "CAN1";
    pub const RX_PIN: &str = "PB12";
    pub const TX_PIN: &str = "PB13";
    pub const PHY: &str = "TJA1042";
    pub const STANDBY_PIN: &str = "PB4";
    pub const STANDBY_NORMAL_LEVEL_HIGH: bool = false;
    pub type Peripheral = ariel_os_hal::hal::peripherals::CAN2;
    pub type MasterPeripheral = ariel_os_hal::hal::peripherals::CAN1;
    pub type RxPin = ariel_os_hal::hal::peripherals::PB12;
    pub type TxPin = ariel_os_hal::hal::peripherals::PB13;
    pub type StandbyPin = ariel_os_hal::hal::peripherals::PB4;

    #[expect(unsafe_code, reason = "board-selected CAN peripheral")]
    pub fn steal_peripheral() -> ariel_os_hal::hal::Peri<'static, Peripheral> {
        unsafe { Peripheral::steal() }
    }

    pub const BITRATE: u32 = 250_000;
    pub const SAMPLE_POINT_PERMILLE: u16 = 889;
    pub const FILTER_SPLIT_INDEX: u8 = 13;
    pub const FILTER_BANK_INDEX: u8 = 13;
    pub const TEST_ID: u16 = 0x123;
    pub const LOOPBACK_TEST_ID: u16 = 0x321;
    pub const LOOPBACK_TIMEOUT_MS: u64 = 300;
}

pub mod tbox_log {
    pub mod kernel {
        pub const TAG: &str = "kernel";
        pub const ENABLED: bool = true;
    }

    pub mod modem {
        pub const TAG: &str = "modem";
        pub const ENABLED: bool = true;
    }

    pub mod can {
        pub const TAG: &str = "can";
        pub const ENABLED: bool = true;
    }

    pub mod vehicle {
        pub const TAG: &str = "vehicle";
        pub const ENABLED: bool = true;
    }

    pub mod remote {
        pub const TAG: &str = "remote";
        pub const ENABLED: bool = true;
    }
}

#[allow(unused_variables)]
pub fn init(peripherals: &mut ariel_os_hal::hal::OptionalPeripherals) {}
