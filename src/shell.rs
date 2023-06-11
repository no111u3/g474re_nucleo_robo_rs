use core::fmt::Write;

use stm32g4xx_hal as hal;

use hal::gpio::*;
use hal::serial::Serial;
use hal::stm32;

pub use ushell::{
    autocomplete::StaticAutocomplete, control, history::LRUHistory, Environment,
    Input as ushell_input, ShellError as ushell_error, SpinResult, UShell,
};

use crate::MotorState;
use btoi::btoi;
use rtic::Mutex;

pub const CMD_MAX_LEN: usize = 32;

pub type Autocomplete = StaticAutocomplete<8>;
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
            EnvSignal::Shell => shell.spin(self),
        }
    }

    fn hard_brake_cmd(&mut self, shell: &mut Shell) -> EnvResult {
        let state = self.motor.lock(|motor| motor.get_state());

        if state != MotorState::HardBrake {
            self.motor.lock(|motor| motor.hard_brake());
            write!(shell, "{0:}ALARM!!!{0:}HARD BRAKE!!!{0:}", CR)?;
        } else {
            write!(shell, "{0:}Already hard brake{0:}", CR)?;
        }

        Ok(())
    }

    fn brake_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
        let max_duty = self.motor.lock(|motor| motor.get_max_duty());
        match btoi::<u32>(args.as_bytes()) {
            Ok(duty) if duty <= max_duty => {
                self.motor.lock(|motor| motor.brake(duty));
                write!(shell, "{0:}Brake enabled: duty={1:}%{0:}\r\n", CR, duty)?;
            }
            _ => {
                write!(shell, "{0:}unsupported duty cycle{0:}\r\n", CR)?;
            }
        }
        Ok(())
    }

    fn release_cmd(&mut self, shell: &mut Shell) -> EnvResult {
        self.motor.lock(|motor| motor.release());
        write!(shell, "{0:}Release brake{0:}\r\n", CR)?;

        Ok(())
    }

    fn cw_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
        let max_duty = self.motor.lock(|motor| motor.get_max_duty());
        match btoi::<u32>(args.as_bytes()) {
            Ok(duty) if duty <= max_duty => {
                self.motor.lock(|motor| motor.cw(duty));
                write!(shell, "{0:}Clockwise enabled: duty={1:}%{0:}\r\n", CR, duty)?;
            }
            _ => {
                write!(shell, "{0:}unsupported duty cycle{0:}\r\n", CR)?;
            }
        }
        Ok(())
    }

    fn ccw_cmd(&mut self, shell: &mut Shell, args: &str) -> EnvResult {
        let max_duty = self.motor.lock(|motor| motor.get_max_duty());
        match btoi::<u32>(args.as_bytes()) {
            Ok(duty) if duty <= max_duty => {
                self.motor.lock(|motor| motor.ccw(duty));
                write!(
                    shell,
                    "{0:}Counter-clockwise enabled: duty={1:}%{0:}\r\n",
                    CR, duty
                )?;
            }
            _ => {
                write!(shell, "{0:}unsupported duty cycle{0:}\r\n", CR)?;
            }
        }
        Ok(())
    }

    fn state_cmd(&mut self, shell: &mut Shell) -> EnvResult {
        let state = self.motor.lock(|motor| motor.get_state());
        let max_duty = self.motor.lock(|motor| motor.get_max_duty());

        write!(
            shell,
            "{0:}Motor state: {1:?}\r\nMax duty: {2}{0:}",
            CR, state, max_duty
        )?;

        Ok(())
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
            "hard" => self.hard_brake_cmd(shell)?,
            "brake" => self.brake_cmd(shell, args)?,
            "release" => self.release_cmd(shell)?,
            "cw" => self.cw_cmd(shell, args)?,
            "ccw" => self.ccw_cmd(shell, args)?,
            "state" => self.state_cmd(shell)?,
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

pub const AUTOCOMPLETE: Autocomplete = StaticAutocomplete([
    "hard", "brake", "release", "cw", "ccw", "state", "clear", "help",
]);

const SHELL_PROMPT: &str = "#> ";
const CR: &str = "\r\n";
const HELP: &str = "\r\n\
G474 ROBO Shell v.1\r\n\r\n\
USAGE:\r\n\
\tcommand\r\n\r\n\
COMMANDS:\r\n\
\thard      Hard brake\r\n\
\tbrake     Brake\r\n\
\trelease   Release\r\n\
\tcw        Clockwise\r\n\
\tccw       Counter-clockwise\r\n\
\tstate     Motor state\r\n\
\tclear     Clear screen\r\n\
\thelp      Print this message\r\n\
";
