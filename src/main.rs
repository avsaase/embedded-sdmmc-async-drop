#![no_std]
#![no_main]

use cortex_m::asm::wfi;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{Level, Output},
    spi::{Config, Spi},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::Delay;
use embedded_sdmmc::{
    asynchronous::{SdCard, VolumeIdx, VolumeManager},
    blocking::{TimeSource, Timestamp},
};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialise Peripherals
    let p = embassy_rp::init(Default::default());

    let cs = Output::new(p.PIN_23, Level::High);
    let mut spi_config = Config::default();
    spi_config.frequency = 400_000;
    let spi: Spi<'_, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async> = Spi::new(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.DMA_CH4, p.DMA_CH5, spi_config,
    );
    let bus: Mutex<
        CriticalSectionRawMutex,
        Spi<'_, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
    > = Mutex::new(spi);
    let spi_device = SpiDevice::new(&bus, cs);
    let sd_card = SdCard::new(spi_device, Delay);

    let volume_manager = VolumeManager::new(sd_card, DummyTimeSource);

    let volume0 = volume_manager.open_volume(VolumeIdx(0)).await.unwrap();

    drop(volume0); // <- this panics with 'Found waker not created by the Embassy executor. `embassy_time::Timer` only works with the Embassy executor.'

    loop {
        wfi();
    }
}

struct DummyTimeSource;

impl TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
