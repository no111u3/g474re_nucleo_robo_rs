#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use dwt_systick_monotonic::DwtSystick;

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;

    // Default system clocked by HSI (16 MHz)
    const SYSFREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYSFREQ>;

    #[shared]
    struct Shared {
    }

    #[local]
    struct Local {
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        // monotonic timer
        let mono = DwtSystick::new(&mut ctx.core.DCB, ctx.core.DWT, ctx.core.SYST, SYSFREQ);

        (
            Shared {
                // Initialization of shared resources go here
            },
            Local {
                // Initialization of local resources go here
            },
            init::Monotonics(mono),
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
