use anyhow::Context;

mod character_pattern;
mod hal;

pub type ClerkDisplay = clerk::Display<
    clerk::ParallelConnection<
        hal::Line,
        hal::FakeLine,
        hal::Line,
        clerk::DataPins4Lines<hal::Line, hal::Line, hal::Line, hal::Line>,
        hal::Delay,
    >,
    clerk::DefaultLines,
>;

pub struct LcdScreen {
    lcd: ClerkDisplay,
}

impl LcdScreen {
    pub fn new() -> anyhow::Result<Self> {
        let wiring_pins_file = "/boot/wiring_pins.toml";
        let pins_src = std::fs::read_to_string(wiring_pins_file).with_context(|| {
            format!(
                "Failed to read GPIO pin declarations file {}",
                wiring_pins_file
            )
        })?;

        let pins: PinDeclarations =
            toml::from_str(&pins_src).context("Failed to parse GPIO pin declarations file")?;
        log::info!("GPIO pins {:?}", pins);
        let mut chip = gpio_cdev::Chip::new("/dev/gpiochip0")
            .context("Failed to open GPIO character device")?; // no delay needed here
        let mut lcd = pins
            .create_display(&mut chip)
            .context("Could not create display")?;

        lcd.seek_cgram(clerk::SeekFrom::Home(0)); // Seek to character generator RAM, i.e. update the character patterns
        for character_bitmap in &character_pattern::BITMAPS {
            for row in character_bitmap {
                lcd.write(*row);
            }
        }
        lcd.seek(clerk::SeekFrom::Home(0)); // Seek to display data RAM, i.e. reset the cursor

        Ok(Self { lcd })
    }
}

impl app::CharacterDisplay for LcdScreen {
    fn clear(&mut self) {
        self.lcd.clear();
        std::thread::sleep(std::time::Duration::from_millis(3));
    }

    fn move_cursor(&mut self, app::CursorPosition { row, column }: app::CursorPosition) {
        const NUM_CHARACTERS_PER_LINE: u8 = 20;
        const ROW_OFFSET: u8 = 0x40;

        let line_start = match row {
            0 => 0,
            1 => ROW_OFFSET,
            2 => NUM_CHARACTERS_PER_LINE,
            _ => ROW_OFFSET + NUM_CHARACTERS_PER_LINE,
        };

        self.lcd.seek(clerk::SeekFrom::Home(line_start + column));
    }

    fn write_char(&mut self, c: char) {
        let code = match c {
            '\u{E000}' => 0,
            '\u{E001}' => 1,
            '\u{E002}' => 2,
            '\u{E003}' => 3,
            '\u{E004}' => 4,
            'é' => 5, // e accute fifth bespoke character defined starting with the zeroeth bespoke character
            'è' => 6, // e grave
            'à' => 7, // a grave
            'ä' => 0xE1, // a umlaut            // see look up table in GDM2004D.pdf page 9/9
            'ñ' => 0xEE, // n tilde
            'ö' => 0xEF, // o umlaut++
            'ü' => 0xF5, // u umlaut
            'π' => 0xE4, // pi
            'µ' => 0xF7, // mu
            '~' => 0xF3, // cannot display tilde using the standard character set in GDM2004D.pdf. This is the best we can do.
            '' => 0xFF, // <Control>  = 0x80 replaced by splodge
            '\x00'..='\x7F' => c as u8,
            _ => 0xFF,
        };

        self.lcd.write(code);
    }
}

#[derive(Debug, serde::Deserialize)]
struct PinDeclarations {
    rs: u32,     // Register Select
    enable: u32, // Also known as strobe and clock
    data4: u32,
    data5: u32,
    data6: u32,
    data7: u32,
}
impl PinDeclarations {
    fn create_display(self, chip: &mut gpio_cdev::Chip) -> Result<ClerkDisplay, anyhow::Error> {
        let register_select = get_line(chip, self.rs, "register_select")?;
        let read = hal::FakeLine;
        let enable = get_line(chip, self.enable, "enable")?;
        let data4 = get_line(chip, self.data4, "data4")?;
        let data5 = get_line(chip, self.data5, "data5")?;
        let data6 = get_line(chip, self.data6, "data6")?;
        let data7 = get_line(chip, self.data7, "data7")?;

        let pins = clerk::Pins {
            register_select,
            read,
            enable,
            data: clerk::DataPins4Lines {
                data4,
                data5,
                data6,
                data7,
            },
        };

        let lcd =
            clerk::Display::<_, clerk::DefaultLines>::new(pins.into_connection::<hal::Delay>());

        lcd.init(clerk::FunctionSetBuilder::default().set_line_number(clerk::LineNumber::Two)); // screen has 4 lines, but electrically, only 2
        std::thread::sleep(std::time::Duration::from_millis(3)); // with this line commented out, screen goes blank, and cannot be written to subsequently
                                                                 // 1.5 ms is marginal as 1.2ms does not work.

        lcd.set_display_control(
            clerk::DisplayControlBuilder::default() // defaults are display on cursor off blinking off ie cursor is an underscore
                .set_cursor(clerk::CursorState::Off), // normally we want the cursor off
        ); //no extra delay needed here

        lcd.clear();
        std::thread::sleep(std::time::Duration::from_millis(2)); // if this line is commented out, garbage or nothing appears. 1ms is marginal

        Ok(lcd)
    }
}

fn get_line(
    chip: &mut gpio_cdev::Chip,
    offset: u32,
    consumer: &'static str,
) -> Result<hal::Line, anyhow::Error> {
    let handle = chip
        .get_line(offset)
        .with_context(|| format!("Failed to get GPIO pin for {:?}", consumer))?
        .request(gpio_cdev::LineRequestFlags::OUTPUT, 0, consumer)
        .with_context(|| format!("GPIO pin for {:?} already in use. Are you running another copy of the program elsewhere?", consumer))?;
    Ok(hal::Line::new(handle))
}
