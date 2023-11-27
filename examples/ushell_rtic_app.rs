#![no_std]
#![no_main]

use panic_halt as _;
use rtic::{self, Mutex};
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::gpio::*;
use hal::prelude::*;
use hal::pwm::*;
use hal::serial::{Event::Rxne, FullConfig, Serial};
use hal::stm32;
use hal::syscfg::SysCfgExt;

use dwt_systick_monotonic::DwtSystick;

use core::fmt::Write;

use btoi::btoi;

use lexical_core::BUFFER_SIZE;

type LedType = Pwm<stm32::TIM2, C1, ComplementaryImpossible, ActiveHigh, ActiveHigh>;

mod shell {
    use super::*;

    pub use ushell::{
        autocomplete::StaticAutocomplete, control, history::LRUHistory, Environment,
        Input as ushell_input, ShellError as ushell_error, SpinResult, UShell,
    };

    pub const CMD_MAX_LEN: usize = 32;

    pub type Autocomplete = StaticAutocomplete<7>;
    pub type History = LRUHistory<{ CMD_MAX_LEN }, 32>;
    pub type Uart = Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>;
    pub type Shell = UShell<Uart, Autocomplete, History, { CMD_MAX_LEN }>;

    pub enum EnvSignal {
        Shell,
        ButtonClick,
    }

    pub type Env<'a> = super::app::env::SharedResources<'a>;
    pub type EnvResult = SpinResult<Uart, ()>;

    impl Env<'_> {
        pub fn on_signal(&mut self, shell: &mut Shell, sig: EnvSignal) -> EnvResult {
            match sig {
                EnvSignal::Shell => shell.spin(self),
                EnvSignal::ButtonClick => self.button_click(),
            }
        }

        fn button_click(&mut self) -> EnvResult {
            let max_duty = self.led.lock(|pwm| pwm.get_max_duty());
            let duty = self.led.lock(|pwm| pwm.get_duty());
            if duty < max_duty / 2 {
                self.led.lock(|pwm| pwm.set_duty(max_duty));
            } else {
                self.led.lock(|pwm| pwm.set_duty(0));
            }
            Ok(())
        }

        fn pwm_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
            match btoi::<u32>(args.as_bytes()) {
                Ok(duty) if duty <= 100 => {
                    self.pwm_set_duty(duty);
                    write!(shell, "{0:}Led enabled: duty={1:}%{0:}\r\n", CR, duty)?;
                }
                _ => {
                    write!(shell, "{0:}unsupported duty cycle{0:}\r\n", CR)?;
                }
            }
            Ok(())
        }

        fn status_cmd(&mut self, shell: &mut Shell) -> EnvResult {
            let duty = self.pwm_get_duty();
            if duty == 0 {
                write!(shell, "{0:}Led disabled{0:}\r\n", CR)?;
            } else {
                write!(shell, "{0:}Led enabled: duty={1:}%{0:}\r\n", CR, duty)?;
            }
            Ok(())
        }

        fn off_cmd(&mut self, shell: &mut Shell) -> EnvResult {
            let duty = self.pwm_get_duty();
            if duty == 0 {
                write!(shell, "{0:}Led already off{0:}\r\n", CR)?;
            } else {
                self.pwm_set_duty(0);
                write!(shell, "{0:}Led disabled{0:}\r\n", CR)?;
            }
            Ok(())
        }

        fn on_cmd(&mut self, shell: &mut Shell) -> EnvResult {
            let duty = self.pwm_get_duty();
            if duty != 0 {
                write!(shell, "{0:}Led already on{0:}\r\n", CR)?;
            } else {
                self.pwm_set_duty(100);
                write!(shell, "{0:}Led enabled: duty={1:}%{0:}\r\n", CR, 100)?;
            }
            Ok(())
        }

        fn float_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
            match lexical_core::parse::<f32>(args.as_bytes()) {
                Ok(num) => {
                    let mut buffer = [b'0'; BUFFER_SIZE];
                    let out = lexical_core::write(num, &mut buffer);
                    write!(shell, "{0:}float={1:}{0:}", CR, core::str::from_utf8(&out).unwrap())?;
                    let out = lexical_core::write(num * 1.1f32, &mut buffer);
                    write!(shell, "{0:}also multiple by 1.1={1:}{0:}", CR, core::str::from_utf8(&out).unwrap())?;
                }
                _ => {
                    write!(shell, "{0:}unsupported float{0:}\r\n", CR)?;
                }
            }
            Ok(())
        }

        fn pwm_set_duty(&mut self, pwm_percentage: u32) {
            let max_duty = self.led.lock(|pwm| pwm.get_max_duty());
            let duty = max_duty * pwm_percentage / 100;
            self.led.lock(|pwm| pwm.set_duty(duty));
        }

        fn pwm_get_duty(&mut self) -> u32 {
            let max_duty = self.led.lock(|pwm| pwm.get_max_duty());
            self.led.lock(|pwm| pwm.get_duty()) * 100 / max_duty
        }

        fn help_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
            match args {
                _ => shell.write_str(HELP)?,
            }
            Ok(())
        }
    }

    impl Environment<Uart, Autocomplete, History, (), { CMD_MAX_LEN }> for Env<'_> {
        fn command(&mut self, shell: &mut Shell, cmd: &str, args: &str) -> EnvResult {
            match cmd {
                "clear" => shell.clear()?,
                "pwm" => self.pwm_cmd(shell, args)?,
                "status" => self.status_cmd(shell)?,
                "on" => self.on_cmd(shell)?,
                "off" => self.off_cmd(shell)?,
                "float" => self.float_cmd(shell, args)?,
                "help" => self.help_cmd(shell, args)?,
                "" => shell.write_str(CR)?,
                _ => write!(shell, "{0:}unsupported command: \"{1:}\"{0:}", CR, cmd)?,
            }
            shell.write_str(SHELL_PROMPT)?;
            Ok(())
        }

        fn control(&mut self, shell: &mut Shell, code: u8) -> EnvResult {
            match code {
                control::CTRL_C => {
                    shell.write_str(CR)?;
                    shell.write_str(SHELL_PROMPT)?;
                }
                _ => {}
            }
            Ok(())
        }
    }

    pub const AUTOCOMPLETE: Autocomplete =
        StaticAutocomplete(["clear", "help", "off", "on", "pwm", "status", "float"]);

    const SHELL_PROMPT: &str = "#> ";
    const CR: &str = "\r\n";
    const HELP: &str = "\r\n\
LED Shell v.1\r\n\r\n\
USAGE:\r\n\
\tcommand\r\n\r\n\
COMMANDS:\r\n\
\ton        Enable led\r\n\
\toff       Disable led\r\n\
\tpwm       Set pwm value\r\n\
\tstatus    Get led status\r\n\
\tfloat     Float parse test\r\n\
\tclear     Clear screen\r\n\
\thelp      Print this message\r\n\
";
}

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;
    type ButtonType = gpioc::PC13<Input<PullDown>>;

    // Default system clocked by HSI (16 MHz)
    const SYSFREQ: u32 = 16_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYSFREQ>;

    #[shared]
    struct Shared {
        led: LedType,
    }

    #[local]
    struct Local {
        button: ButtonType,
        shell: shell::Shell,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        // syscfg
        let mut syscfg = ctx.device.SYSCFG.constrain();
        // clocks
        let mut rcc = ctx.device.RCC.constrain();
        // monotonic timer
        let mono = DwtSystick::new(&mut ctx.core.DCB, ctx.core.DWT, ctx.core.SYST, SYSFREQ);
        // exti
        let mut exti = ctx.device.EXTI;

        info!("Init UART");

        let gpioa = ctx.device.GPIOA.split(&mut rcc);
        let gpioc = ctx.device.GPIOC.split(&mut rcc);

        // button
        let mut button = gpioc.pc13.into_pull_down_input();
        button.make_interrupt_source(&mut syscfg);
        button.trigger_on_edge(&mut exti, SignalEdge::Rising);
        button.enable_interrupt(&mut exti);
        // led
        let mut led = ctx
            .device
            .TIM2
            .pwm(gpioa.pa5.into_alternate(), 200.khz(), &mut rcc);
        led.set_duty(0);
        led.enable();

        // serial
        let tx = gpioa.pa2.into_alternate();
        let rx = gpioa.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        // shell
        let mut shell =
            shell::UShell::new(serial, shell::AUTOCOMPLETE, shell::LRUHistory::default());

        writeln!(shell, "\r\nHello from USART2\r\n").unwrap();

        (
            Shared {
                // Initialization of shared resources go here
                led,
            },
            Local {
                // Initialization of local resources go here
                button,
                shell,
            },
            init::Monotonics(mono),
        )
    }

    #[task(binds = EXTI15_10, local = [button])]
    fn button_click(ctx: button_click::Context) {
        ctx.local.button.clear_interrupt_pending_bit();
        env::spawn(shell::EnvSignal::ButtonClick).ok();
    }

    #[task(binds = USART2, priority = 1)]
    fn serial_callback(_: serial_callback::Context) {
        env::spawn(shell::EnvSignal::Shell).ok();
    }

    #[task(priority = 2, capacity = 8, local = [shell], shared = [led])]
    fn env(ctx: env::Context, sig: shell::EnvSignal) {
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
