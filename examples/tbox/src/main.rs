#![no_main]
#![no_std]

use ariel_os::{
    log::*,
    time::Timer,
};

#[ariel_os::task(autostart)]
async fn main() {
    loop {
        info!("Hello World!");
        Timer::after_secs(1).await;
    }
}
