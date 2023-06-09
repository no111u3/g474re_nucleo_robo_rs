#![no_std]
#![no_main]

mod shell;

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::pwm::*;
use hal::stm32::*;
use hal::prelude::*;
use hal::serial::{Event::Rxne, FullConfig};

use dwt_systick_monotonic::DwtSystick;

use core::fmt::Write;

use shell::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MotorState {
    HardBrake,
    Brake(u32),
    Release,
    Cw(u32),
    Ccw(u32),
}

pub struct Mx1508 {
    pwm1: Pwm<TIM2, C2, ComplementaryImpossible, ActiveHigh, ActiveHigh>,
    pwm2: Pwm<TIM2, C3, ComplementaryImpossible, ActiveHigh, ActiveHigh>,
    motor_state: MotorState,
}

impl Mx1508 {
    pub fn hard_brake(&mut self) {
        self.pwm1.set_duty(self.pwm1.get_max_duty());
        self.pwm2.set_duty(self.pwm2.get_max_duty());
        self.motor_state = MotorState::HardBrake;
    }

    pub fn brake(&mut self, duty :u32) {
        self.pwm1.set_duty(duty);
        self.pwm2.set_duty(duty);
        self.motor_state = MotorState::Brake(duty);
    }

    pub fn release(&mut self) {
        self.pwm1.set_duty(0);
        self.pwm2.set_duty(0);
        self.motor_state = MotorState::Release;
    }

    pub fn cw(&mut self, duty :u32) {
        self.pwm1.set_duty(duty);
        self.pwm2.set_duty(0);
        self.motor_state = MotorState::Cw(duty);
    }

    pub fn ccw(&mut self, duty :u32) {
        self.pwm1.set_duty(0);
        self.pwm2.set_duty(duty);
        self.motor_state = MotorState::Ccw(duty);
    }

    pub fn get_max_duty(&self) -> u32 {
        self.pwm1.get_max_duty()
    }

    pub fn get_state(&self) -> MotorState {
        self.motor_state
    }
}

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;

    // Default system clocked by HSI (16 MHz)
    const SYS_FREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYS_FREQ>;

    #[shared]
    struct Shared {
        motor: Mx1508,
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
        let gpio_b = ctx.device.GPIOB.split(&mut rcc);

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

        // motor
        let (mut pwm1, mut pwm2) = ctx
            .device
            .TIM2
            .pwm((gpio_b.pb3.into_alternate(), gpio_b.pb10.into_alternate()), 500.hz(), &mut rcc);
        pwm1.set_duty(pwm1.get_max_duty());
        pwm2.set_duty(pwm2.get_max_duty());
        pwm1.enable();
        pwm2.enable();

        let motor_state = MotorState::HardBrake;

        // device objects
        let motor = Mx1508{
            pwm1,
            pwm2,
            motor_state,
        };

        (
            Shared {
                // Initialization of shared resources go here
                motor,
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

    #[task(priority = 2, capacity = 8, local = [shell], shared = [motor])]
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
