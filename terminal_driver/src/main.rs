//! # Terminal Driver
//! A development implementation of the screen driver application which outputs to the terminal

use std::io::Write;

use crossterm::{
    cursor::MoveTo,
    terminal::{Clear, ClearType},
    ExecutableCommand,
};

struct MockTemperatureSource(u8);

impl app::TemperatureSource for MockTemperatureSource {
    fn get_temperature(&mut self) -> app::Temperature {
        self.0 = self.0.wrapping_add(1);

        app::Temperature(self.0)
    }
}

struct TerminalDisplay {
    stdout: std::io::Stdout,
}

impl TerminalDisplay {
    fn new() -> Self {
        Self {
            stdout: std::io::stdout(),
        }
    }
}

impl app::CharacterDisplay for TerminalDisplay {
    fn clear(&mut self) {
        self.stdout.execute(Clear(ClearType::All)).unwrap();
    }

    fn move_cursor(&mut self, position: app::CursorPosition) {
        self.stdout
            .execute(MoveTo(position.column.into(), position.row.into()))
            .unwrap();
        self.stdout.flush().unwrap();
    }

    fn write_char(&mut self, c: char) {
        let c = match c {
            '\u{E000}' => '▌',
            '\u{E001}' => '▏',
            '\u{E002}' => '|',
            '\u{E003}' => '▕',
            '\u{E004}' => '▐',
            _ => c,
        };
        write!(self.stdout, "{}", c).unwrap();
        self.stdout.flush().unwrap();
    }
}

fn main() {
    app::run("MOCK IP", MockTemperatureSource(0), TerminalDisplay::new())
}
