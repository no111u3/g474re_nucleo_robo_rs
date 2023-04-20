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
use hal::stm32;

use core::fmt::Write;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        serial: Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        cnt: u32,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        let mut rcc = ctx.device.RCC.constrain();

        info!("Init UART");

        let gpioa = ctx.device.GPIOA.split(&mut rcc);
        let tx = gpioa.pa2.into_alternate();
        let rx = gpioa.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        writeln!(serial, "Hello from USART2\r\n").unwrap();

        (Shared {}, Local { serial, cnt: 0 }, init::Monotonics())
    }

    #[task(binds = USART2, priority = 1, local = [serial, cnt])]
    fn serial_callback(ctx: serial_callback::Context) {
        let serial_callback::LocalResources { serial, cnt } = ctx.local;

        match serial.read() {
            Ok(byte) => {
                writeln!(serial, "{}: {}\r", cnt, byte).unwrap();
            }
            _ => {
                info!("Error reading from serial");
            }
        }
        *cnt += 1;
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
