#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::prelude::*;
use hal::serial::{Event::Rxne, FullConfig, Serial};
use hal::spi::Spi;
use hal::stm32;

use core::fmt::Write;

use tle5012::{self, Tle5012, MODE};

use dwt_systick_monotonic::*;

use hal::time::{ExtU32, RateExtU32};

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;

    // Default system clocked by HSI (16 MHz)
    const SYS_FREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYS_FREQ>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        serial: Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        angle_sensor: Tle5012<
            Spi<
                stm32::SPI1,
                (
                    gpioa::PA5<Alternate<5>>,
                    gpioa::PA6<Alternate<5>>,
                    gpioa::PA7<Alternate<5>>,
                ),
            >,
            gpioa::PA9<Output<PushPull>>,
        >,
    }

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
        let miso = gpio_a.pa6.into_alternate();
        let mosi = gpio_a.pa7.into_alternate();
        let mut nss = gpio_a.pa9.into_push_pull_output();
        nss.set_high().ok();

        let spi = ctx
            .device
            .SPI1
            .spi((sck, miso, mosi), MODE, 500.kHz(), &mut rcc);

        let mut angle_sensor = Tle5012::new(spi, nss).unwrap();

        match angle_sensor.read_status() {
            Ok(status) => {
                writeln!(serial, "Angle sensor status is 0x{:x}\r\n", status).unwrap();
            }
            Err(error) => {
                writeln!(serial, "Error for read status is {:?}\r\n", error).unwrap();
            }
        }

        // Schedule the tle5012 task
        tle5012::spawn().ok();

        (
            Shared {},
            Local {
                serial,
                angle_sensor,
            },
            init::Monotonics(mono),
        )
    }

    #[task(local = [serial, angle_sensor])]
    fn tle5012(ctx: tle5012::Context) {
        let tle5012::LocalResources {
            serial,
            angle_sensor,
        } = ctx.local;

        match angle_sensor.read_angle_value() {
            Ok(angle_value) => {
                writeln!(serial, "Angle value is {}\r\n", angle_value).unwrap();
            }
            Err(error) => {
                writeln!(serial, "Error for read status is {:?}\r\n", error).unwrap();
            }
        }

        match angle_sensor.read_angle_speed() {
            Ok(angle_speed) => {
                writeln!(serial, "Angle speed is {}\r\n", angle_speed).unwrap();
            }
            Err(error) => {
                writeln!(serial, "Error for read status is {:?}\r\n", error).unwrap();
            }
        }

        tle5012::spawn_after(200.millis()).unwrap();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
