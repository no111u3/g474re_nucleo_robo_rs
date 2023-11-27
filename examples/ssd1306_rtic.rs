#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::prelude::*;
use hal::serial::{Event::Rxne, FullConfig};
use hal::spi;

use core::fmt::Write;

use dwt_systick_monotonic::*;

use ssd1306::{prelude::*, Ssd1306};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use hal::time::RateExtU32;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    // Default system clocked by HSI (16 MHz)
    const SYS_FREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYS_FREQ>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        let mut rcc = ctx.device.RCC.constrain();
        // monotonic timer
        let mono = DwtSystick::new(&mut ctx.core.DCB, ctx.core.DWT, ctx.core.SYST, SYS_FREQ);

        info!("Init UART");

        let gpio_a = ctx.device.GPIOA.split(&mut rcc);
        let tx = gpio_a.pa2.into_alternate();
        let rx = gpio_a.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        writeln!(serial, "TLE5012 demo\r\n").unwrap();

        info!("Init SPI");

        // Setup spi i/o
        let sck = gpio_a.pa5.into_alternate();
        let mosi = gpio_a.pa7.into_alternate();
        let mut nss = gpio_a.pa9.into_push_pull_output();
        nss.set_high().ok();
        let mut dc = gpio_a.pa6.into_push_pull_output();
        dc.set_high().ok();

        let spi = ctx.device.SPI1.spi(
            (sck, spi::NoMiso, mosi),
            spi::Mode {
                polarity: spi::Polarity::IdleLow,
                phase: spi::Phase::CaptureOnFirstTransition,
            },
            500.kHz(),
            &mut rcc,
        );

        let interface = display_interface_spi::SPIInterface::new(spi, dc, nss);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

        display.init().unwrap();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        Text::with_baseline("Hello world!", Point::zero(), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        Text::with_baseline("Hello Rust!", Point::new(0, 16), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        display.flush().unwrap();

        (Shared {}, Local {}, init::Monotonics(mono))
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
