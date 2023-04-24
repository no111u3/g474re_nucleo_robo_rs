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
use hal::syscfg::SysCfgExt;

use dwt_systick_monotonic::DwtSystick;

use core::fmt::Write;

use ushell::{
    autocomplete::StaticAutocomplete, history::LRUHistory, Input as ushell_input,
    ShellError as ushell_error, UShell,
};

#[rtic::app(device = hal::stm32, peripherals = true, dispatchers = [USART1])]
mod app {
    use super::*;

    type LedType = gpioa::PA5<Output<PushPull>>;
    type ButtonType = gpioc::PC13<Input<PullDown>>;
    type ShellType = UShell<
        Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        StaticAutocomplete<5>,
        LRUHistory<32, 4>,
        32,
    >;

    const SHELL_PROMPT: &str = "#> ";
    const CR: &str = "\r\n";
    const HELP: &str = "\r\n\
LED Shell v.1\r\n\r\n\
USAGE:\r\n\
\tcommand\r\n\r\n\
COMMANDS:\r\n\
\ton        Enable led\r\n\
\toff       Disable led\r\n\
\tstatus    Get led status\r\n\
\tclear     Clear screen\r\n\
\thelp      Print this message\r\n\
";
    const SYSFREQ: u32 = 24_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<SYSFREQ>;

    #[shared]
    struct Shared {
        led_enabled: bool,
    }

    #[local]
    struct Local {
        button: ButtonType,
        led: LedType,
        shell: ShellType,
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
        let led = gpioa.pa5.into_push_pull_output();

        // serial
        let tx = gpioa.pa2.into_alternate();
        let rx = gpioa.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        // ushell
        let autocomplete = StaticAutocomplete(["clear", "help", "off", "on", "status"]);
        let history = LRUHistory::default();
        let mut shell = UShell::new(serial, autocomplete, history);

        writeln!(shell, "\r\nHello from USART2\r\n").unwrap();

        (
            Shared {
                // Initialization of shared resources go here
                led_enabled: false,
            },
            Local {
                // Initialization of local resources go here
                button,
                led,
                shell,
            },
            init::Monotonics(mono),
        )
    }

    #[task(local = [led], shared = [led_enabled])]
    fn setled(ctx: setled::Context) {
        let setled::LocalResources { led } = ctx.local;
        let setled::SharedResources { mut led_enabled } = ctx.shared;
        let led_on = led_enabled.lock(|e| *e);
        if !led_on {
            led.set_low().unwrap();
        } else {
            led.set_high().unwrap();
        }
    }

    #[task(binds = EXTI15_10, local = [button], shared = [led_enabled])]
    fn button_click(mut ctx: button_click::Context) {
        ctx.local.button.clear_interrupt_pending_bit();
        let led_on = ctx.shared.led_enabled.lock(|e| *e);
        if led_on {
            ctx.shared.led_enabled.lock(|e| *e = false);
        } else {
            ctx.shared.led_enabled.lock(|e| *e = true);
        }
        setled::spawn().unwrap();
    }

    #[task(binds = USART2, priority = 1, shared = [led_enabled], local = [shell])]
    fn serial_callback(ctx: serial_callback::Context) {
        let serial_callback::LocalResources { shell } = ctx.local;
        let serial_callback::SharedResources { mut led_enabled } = ctx.shared;

        loop {
            match shell.poll() {
                Ok(Some(ushell_input::Command((cmd, _args)))) => {
                    match cmd {
                        "help" => {
                            shell.write_str(HELP).ok();
                        }
                        "clear" => {
                            shell.clear().ok();
                        }
                        "on" => {
                            led_enabled.lock(|e| *e = true);
                            setled::spawn().unwrap();
                            shell.write_str(CR).ok();
                        }
                        "off" => {
                            led_enabled.lock(|e| *e = false);
                            setled::spawn().unwrap();
                            shell.write_str(CR).ok();
                        }
                        "status" => {
                            let on = led_enabled.lock(|e| *e);
                            let status = if on { "On" } else { "Off" };
                            write!(shell, "{0:}LED: {1:}{0:}", CR, status).ok();
                        }
                        "" => {
                            shell.write_str(CR).ok();
                        }
                        _ => {
                            write!(shell, "{0:}unsupported command{0:}", CR).ok();
                        }
                    }
                    shell.write_str(SHELL_PROMPT).ok();
                }
                Err(ushell_error::WouldBlock) => break,
                _ => {}
            }
        }
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
