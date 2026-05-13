use ariel_os_boards::pins;

ariel_os::hal::group_peripherals!(Peripherals {
    led: pins::LedPeripherals,
    modem: pins::ModemPeripherals,
});

#[cfg(not(context = "st-stm32f427vg"))]
compile_error!("tbox PPP prototype currently supports only the st-stm32f427vg board.");
