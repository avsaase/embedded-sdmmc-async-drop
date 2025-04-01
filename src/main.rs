#![no_std]
#![no_main]

use core::fmt::Write;

use defmt::{error, info, warn};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{Level, Output},
    peripherals::{self},
    spi::{self, Config, Spi},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{block_for, Delay, Duration, Timer};
use embedded_hal_async::spi::SpiDevice as _;
use embedded_sdmmc::{
    asynchronous::{Mode, SdCard, VolumeIdx, VolumeManager},
    blocking::{TimeSource, Timestamp},
};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

type SpiBus = Mutex<CriticalSectionRawMutex, Spi<'static, peripherals::SPI0, spi::Async>>;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialise Peripherals
    let p = embassy_rp::init(Default::default());

    let mut spi_config = Config::default();
    spi_config.frequency = 400_000;
    let spi: Spi<'_, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async> = Spi::new(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.DMA_CH4, p.DMA_CH5, spi_config,
    );
    static SPI_BUS: StaticCell<SpiBus> = StaticCell::new();
    let spi_bus = SPI_BUS.init(Mutex::new(spi));
    let cs_sd = Output::new(p.PIN_23, Level::High);
    let cs_other = Output::new(p.PIN_24, Level::High);

    spawner.must_spawn(other_task(spi_bus, cs_other));
    spawner.must_spawn(sd_task(spi_bus, cs_sd));
}

#[embassy_executor::task]
async fn sd_task(bus: &'static SpiBus, cs: Output<'static>) {
    bus.lock().await.set_frequency(25_000_000);

    let spi_device = SpiDevice::new(bus, cs);
    let sd_card = SdCard::new(spi_device, Delay);
    let volume_manager = VolumeManager::new(sd_card, DummyTimeSource);

    let volume0 = volume_manager.open_volume(VolumeIdx(0)).await.unwrap();

    let root_dir = volume0.open_root_dir().unwrap();

    let new_file = root_dir
        .open_file_in_dir("TEST.TXT", Mode::ReadWriteCreateOrTruncate)
        .await
        .unwrap();

    let mut buf = heapless::String::<64>::new();

    for i in 0..100_000 {
        let mut attempt = 0;
        write!(buf, "Line: {:05}\n", i).unwrap();
        'retry: loop {
            attempt += 1;
            match new_file.write(buf.as_bytes()).await {
                Ok(_) => {
                    // info!("Successfully wrote to file at attempt {}", attempt);
                    break 'retry;
                }
                Err(e) => {
                    warn!("Error writing to file at attempt {}: {:?}", attempt, e);
                    if attempt > 3 {
                        error!("Failed to write to file after 3 attempts");
                        break 'retry;
                    }
                }
            }
        }
        buf.clear();
    }
    new_file.close().await.unwrap();
    root_dir.close().unwrap();
    volume0.close().await.unwrap();
    info!("SD task done");
}

#[embassy_executor::task]
async fn other_task(bus: &'static SpiBus, cs: Output<'static>) {
    let mut spi_device = SpiDevice::new(bus, cs);
    loop {
        Timer::after(Duration::from_millis(100)).await;
        block_for(Duration::from_millis(10));
        // Write some dummy bytes
        spi_device.write(&[0x55; 10]).await.unwrap();
        spi_device.write(&[0x55; 10]).await.unwrap();
        spi_device.write(&[0x55; 10]).await.unwrap();
        spi_device.write(&[0x55; 10]).await.unwrap();
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
