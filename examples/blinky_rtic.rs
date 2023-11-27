#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::prelude::*;
use hal::pwm::*;
use hal::stm32;
use hal::syscfg::SysCfgExt;

pub enum PwmDuty {
    Quarter,
    Half,
    ThreeQuarters,
    Full,
}

use hal::time::RateExtU32;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        exti: stm32::EXTI,
        button: gpioc::PC13<Input<PullDown>>,
        pwm: Pwm<stm32::TIM2, C1, ComplementaryImpossible, ActiveHigh, ActiveHigh>,
        pwm_duty: PwmDuty,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        let mut exti = ctx.device.EXTI;
        let mut rcc = ctx.device.RCC.constrain();
        let mut syscfg = ctx.device.SYSCFG.constrain();

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let port_c = ctx.device.GPIOC.split(&mut rcc);

        let mut pwm = ctx
            .device
            .TIM2
            .pwm(port_a.pa5.into_alternate(), 2.Hz(), &mut rcc);
        pwm.set_duty(pwm.get_max_duty() / 2);
        pwm.enable();

        let mut button = port_c.pc13.into_pull_down_input();
        button.make_interrupt_source(&mut syscfg);
        button.trigger_on_edge(&mut exti, SignalEdge::Rising);
        button.enable_interrupt(&mut exti);

        (
            Shared {},
            Local {
                exti,
                button,
                pwm,
                pwm_duty: PwmDuty::Half,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = EXTI15_10, local = [exti, button, pwm, pwm_duty])]
    fn button_click(ctx: button_click::Context) {
        let new_pwm_duty = match ctx.local.pwm_duty {
            PwmDuty::Quarter => {
                info!("Half");
                ctx.local.pwm.set_duty(ctx.local.pwm.get_max_duty() / 2);
                PwmDuty::Half
            }
            PwmDuty::Half => {
                info!("Three Quarters");
                ctx.local.pwm.set_duty(ctx.local.pwm.get_max_duty() * 3 / 4);
                PwmDuty::ThreeQuarters
            }
            PwmDuty::ThreeQuarters => {
                info!("Full");
                ctx.local.pwm.set_duty(ctx.local.pwm.get_max_duty());
                PwmDuty::Full
            }
            PwmDuty::Full => {
                info!("Quarter");
                ctx.local.pwm.set_duty(ctx.local.pwm.get_max_duty() / 4);
                PwmDuty::Quarter
            }
        };
        *ctx.local.pwm_duty = new_pwm_duty;
        ctx.local.exti.unpend(hal::exti::Event::GPIO10);
        ctx.local.button.clear_interrupt_pending_bit();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
