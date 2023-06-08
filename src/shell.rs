use core::fmt::Write;

use stm32g4xx_hal as hal;

use hal::gpio::*;
use hal::serial::Serial;
use hal::stm32;

pub use ushell::{
    autocomplete::StaticAutocomplete, history::LRUHistory, Input as ushell_input,
    ShellError as ushell_error, SpinResult, UShell, Environment, control
};

pub const CMD_MAX_LEN: usize = 32;

pub type Autocomplete = StaticAutocomplete<2>;
pub type History = LRUHistory<{ CMD_MAX_LEN }, 32>;
pub type Uart = Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>;
pub type Shell = UShell<Uart, Autocomplete, History, { CMD_MAX_LEN }>;

pub enum EnvSignal {
    Shell,
}

pub type Env<'a> = super::app::env::SharedResources<'a>;
pub type EnvResult = SpinResult<Uart, ()>;

impl Env<'_> {
    pub fn on_signal(&mut self, shell: &mut Shell, sig: EnvSignal) -> EnvResult {
        match sig {
            EnvSignal::Shell =>
                shell.spin(self),
        }
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
    StaticAutocomplete(["clear", "help"]);

const SHELL_PROMPT: &str = "#> ";
const CR: &str = "\r\n";
const HELP: &str = "\r\n\
G474 ROBO Shell v.1\r\n\r\n\
USAGE:\r\n\
\tcommand\r\n\r\n\
COMMANDS:\r\n\
\tclear     Clear screen\r\n\
\thelp      Print this message\r\n\
";
