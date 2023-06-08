#![no_std]
#![no_main]

mod shell;

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::prelude::*;
use hal::serial::{Event::Rxne, FullConfig};

use dwt_systick_monotonic::DwtSystick;

use core::fmt::Write;

use shell::*;

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;

    // Default system clocked by HSI (16 MHz)
    const SYS_FREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYS_FREQ>;

    #[shared]
    struct Shared {
        device_on: bool,
    }

    #[local]
    struct Local {
        shell: Shell,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        // clocks
        let mut rcc = ctx.device.RCC.constrain();
        // monotonic timer
        let mono = DwtSystick::new(&mut ctx.core.DCB, ctx.core.DWT, ctx.core.SYST, SYS_FREQ);

        info!("Init UART");

        let gpio_a = ctx.device.GPIOA.split(&mut rcc);

        // serial
        let tx = gpio_a.pa2.into_alternate();
        let rx = gpio_a.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        // shell
        let mut shell = UShell::new(serial, AUTOCOMPLETE, LRUHistory::default());

        writeln!(shell, "\r\nSystem shell at USART2\r\n").unwrap();

        // device status TODO: Replace to real device shared objects
        let device_on = true;

        (
            Shared {
                // Initialization of shared resources go here
                device_on,
            },
            Local {
                // Initialization of local resources go here
                shell,
            },
            init::Monotonics(mono),
        )
    }

    #[task(binds = USART2, priority = 1)]
    fn serial_callback(_: serial_callback::Context) {
        env::spawn(EnvSignal::Shell).ok();
    }

    #[task(priority = 2, capacity = 8, local = [shell], shared = [device_on])]
    fn env(ctx: env::Context, sig: EnvSignal) {
        let mut env = ctx.shared;
        env.on_signal(ctx.local.shell, sig).ok();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
