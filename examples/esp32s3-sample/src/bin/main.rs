#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use cst92xx::CST92xx;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
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

    // TP_SCL/TP_SDA are shared with the onboard ES8311 codec and QMI8658C IMU on this
    // board (see docs/ESP32-S3-Touch-AMOLED-1.75C-schematic.pdf, sheet 1) — if you add
    // audio or IMU support to this example, it's the same I2C bus, not a separate one.
    let i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(i2c_freq_khz)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO15)
    .with_scl(peripherals.GPIO14)
    .into_async();

    // TP_RESET is active-low (see the driver README's wiring section), so idle it
    // high — the driver's own reset() pulses it low/high on init(), we just own the
    // pin here. GPIO2 confirmed against the board schematic (TP_RESET row, sheet 1).
    let rst = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    // TP_INT confirmed as GPIO11 against the board schematic (TP_INT row, sheet 1).
    // The CST9217 datasheet (section 10.6, "中断方式") only says the interrupt edge is
    // configurable as rising or falling — it doesn't document which one this panel's
    // firmware actually uses, and there's no register in this driver to query or set
    // it. Wait for either edge instead of guessing a polarity; worst case is one
    // harmless extra `touches()` read. Pull::Up is a safe default in case the line is
    // open-drain without its own external pull-up.
    let touch_int = Input::new(
        peripherals.GPIO11,
        InputConfig::default().with_pull(Pull::Up),
    );

    let driver = CST92xx::new(i2c, Delay).with_reset(rst);
    spawner.spawn(touch_task(driver, touch_int).unwrap());

    loop {
        info!("Touch controller running");
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
#[allow(
    clippy::large_stack_frames,
    reason = "clippy sums the whole async state machine (which embassy stores in the static \
    TaskPool, not on the call stack) as if it were the function's stack frame. Verified via \
    objdump on the built xtensa-esp32s3-none-elf binary: the real `poll()` entry frame is 192 \
    bytes, well under the crate's 1024-byte threshold."
)]
async fn touch_task(
    mut touch_driver: CST92xx<I2c<'static, Async>, Delay, Output<'static>>,
    mut touch_int: Input<'static>,
) {
    // 1. Initialize the driver at task startup (this also pulses the RST pin)
    if let Err(e) = touch_driver.init().await {
        error!("Failed to initialize touch panel: {:?}", e);
        return;
    }

    // 2. Log what init() discovered, so a flashed board tells you what it found
    // instead of just "it works" — useful when swapping between CST9217/CST9220
    // parts or chasing a mismatched panel resolution. ChipInfo derives
    // defmt::Format, so this prints every field (chip_type, resolution,
    // project_id, fw_version, checksum) without hand-picking any of them.
    info!(
        "Touch panel ready: {} -> {}",
        touch_driver.model_name(),
        touch_driver.chip_info()
    );

    loop {
        // 3. Sleep until TOUCH_INT actually toggles instead of polling on a fixed
        // interval — the chip only drives it when it has a report ready (see the
        // comment where `touch_int` is created), so this keeps the task idle (and
        // the I2C bus quiet) between touches instead of reading ~66 times a second
        // whether or not anything changed.
        touch_int.wait_for_any_edge().await;

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
            Err(e) => {
                error!("I2C communication error: {:?}", e);
            }
        }
    }
}
