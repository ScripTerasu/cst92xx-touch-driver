#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use cst9217::CST92xx;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let _ = spawner;

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let i2c_freq_khz: u32 = 400;

    let i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(i2c_freq_khz)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO15)
    .with_scl(peripherals.GPIO14)
    .into_async();

    let driver = CST92xx::new(i2c);
    spawner.spawn(touch_task(driver).unwrap());

    loop {
        info!("Touch controller running");
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
async fn touch_task(mut touch_driver: CST92xx<I2c<'static, Async>>) {
    // 1. Initialize the driver at task startup
    if let Err(e) = touch_driver.init().await {
        error!("Failed to initialize touch panel: {:?}", e);
        return;
    }

    loop {
        match touch_driver.touches().await {
            Ok(points) => {
                // `flatten()` filters out `None` and unwraps `Some(Point)` in one pass
                for point in points.iter().flatten() {
                    info!(
                        "Touch detected -> ID: {}, X: {}, Y: {}",
                        point.track_id, point.x, point.y
                    );

                    // Send coordinates to your GUI (LVGL, Slint, etc.) or gesture logic
                }
            }
            Err(_e) => {
                error!("I2C communication error");
            }
        }

        // 3. Sampling frequency (~66Hz with a 15ms delay keeps it smooth)
        Timer::after_millis(15).await;
    }
}
