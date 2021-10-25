use std::fmt::{self, Write};

use super::{SCREEN_HEIGHT, SCREEN_WIDTH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorPosition {
    pub row: u8,
    pub column: u8,
}

impl CursorPosition {
    pub fn offset(mut self, offset: u8) -> Self {
        self.column += offset;

        while self.column >= SCREEN_WIDTH {
            self.row += 1;
            self.column -= SCREEN_WIDTH;
        }

        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Segment {
    pub position: CursorPosition,
    pub length: u8,
}

impl Segment {
    pub fn split(self, offset: u8) -> (Self, Self) {
        (
            Self {
                position: self.position,
                length: offset,
            },
            Self {
                position: self.position.offset(offset),
                length: self.length - offset,
            },
        )
    }
}

/// A Line on the screen. Used to paint text at the start of a particular line.
/// Note that the top line is line 0
///
/// # Example
/// ```
/// use app::{TextDisplay, Line};
///
/// fn run(mut text_display: impl TextDisplay) {
///     text_display.write_to(Line(2), "Hello World")
/// }
/// ```
#[derive(Clone, Copy)]
pub struct Line(pub u8);

impl Line {
    pub fn split(self, offset: u8) -> (Segment, Segment) {
        Segment::from(self).split(offset)
    }
}

impl From<Line> for Segment {
    fn from(Line(row): Line) -> Self {
        Self {
            position: CursorPosition { row, column: 0 },
            length: SCREEN_WIDTH,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Lines(pub u8, pub u8);

impl From<Lines> for Segment {
    fn from(Lines(row_start, row_end): Lines) -> Self {
        Self {
            position: CursorPosition {
                row: row_start,
                column: 0,
            },
            length: (1 + row_end - row_start) * SCREEN_WIDTH,
        }
    }
}

#[derive(Clone, Copy)]
pub struct EntireScreen;

impl From<EntireScreen> for Segment {
    fn from(_: EntireScreen) -> Self {
        Self {
            position: CursorPosition { row: 0, column: 0 },
            length: SCREEN_WIDTH * SCREEN_HEIGHT,
        }
    }
}

/// A CharacterDisplay displays characters onto a screen
///
/// # Example
/// ```
/// fn run(mut character_display: impl app::CharacterDisplay) {
///     // Clear the screen and write "Hello World" in the top-left corner
///     character_display.clear();
///     character_display.move_cursor(app::CursorPosition { row: 0, column: 0 });
///     for c in "Hello World".chars() {
///         character_display.write_char(c);
///     }
/// }
/// ```
pub trait CharacterDisplay {
    /// Clears the entire screen
    fn clear(&mut self);
    /// Move the cursor to the specified position
    fn move_cursor(&mut self, position: CursorPosition);
    /// Write a single character to the screen, and move the cursor one place to the right
    fn write_char(&mut self, c: char);
}

/// A TextDisplay display formatted strings onto a screen
///
/// # Example
/// ```
/// fn run(mut text_display: impl app::TextDisplay) {
///     // Clear the screen and write "Hello World" in the top-left corner
///     text_display.clear();
///     text_display.write_to(app::EntireScreen, "Hello World");
/// }
/// ```
pub trait TextDisplay {
    fn clear(&mut self);
    fn write_to(&mut self, segment: impl Into<Segment>, item: impl fmt::Display);
}

/// WrappingTextDisplay wraps long strings by automatically moving the cursor when having written to the end of a line
///
/// It's mostly used via the [TextDisplay] trait
pub struct WrappingTextDisplay<D: CharacterDisplay> {
    character_display: D,
    segment: Segment,
}

impl<D: CharacterDisplay> WrappingTextDisplay<D> {
    pub fn new(character_display: D) -> Self {
        Self {
            character_display,
            segment: EntireScreen.into(),
        }
    }
}

impl<D: CharacterDisplay> core::fmt::Write for WrappingTextDisplay<D> {
    fn write_char(&mut self, c: char) -> fmt::Result {
        if self.segment.position.column >= SCREEN_WIDTH {
            self.segment.position.row += 1;
            self.segment.position.column = 0;
            self.character_display.move_cursor(self.segment.position);
        }

        if self.segment.length > 0 {
            self.character_display.write_char(c);
            self.segment.position.column += 1;
            self.segment.length -= 1;
        }

        Ok(())
    }

    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.chars().try_for_each(|c| self.write_char(c))
    }
}

impl<D: CharacterDisplay> TextDisplay for WrappingTextDisplay<D> {
    fn clear(&mut self) {
        self.character_display.clear();
    }

    fn write_to(&mut self, segment: impl Into<Segment>, item: impl fmt::Display) {
        self.segment = segment.into();
        self.character_display.move_cursor(self.segment.position);

        // Cannot fail as Self::write_char cannot fail
        let _ = self.write_fmt(format_args!("{}", item));

        while self.segment.length > 0 {
            // Cannot fail as Self::write_char cannot fail
            let _ = self.write_char(' ');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mockall::mock! {
        pub CharacterDisplay { }

        impl CharacterDisplay for CharacterDisplay {
            fn clear(&mut self);
            fn move_cursor(&mut self, position: CursorPosition);
            fn write_char(&mut self, c: char);
        }
    }

    use mockall::{predicate::eq, Sequence};

    fn expect_move_cursor(
        mock_character_device: &mut MockCharacterDisplay,
        seq: &mut Sequence,
        cursor_position: CursorPosition,
    ) {
        mock_character_device
            .expect_move_cursor()
            .once()
            .in_sequence(seq)
            .with(eq(cursor_position))
            .returning(|_| ());
    }

    fn expect_write_string(
        mock_character_device: &mut MockCharacterDisplay,
        seq: &mut Sequence,
        text: &str,
    ) {
        for c in text.chars() {
            mock_character_device
                .expect_write_char()
                .once()
                .in_sequence(seq)
                .with(eq(c))
                .returning(|_| ());
        }
    }

    #[test]
    fn test_clear() {
        let mut mock_character_device = MockCharacterDisplay::new();

        mock_character_device.expect_clear().return_once(|| ());

        let mut display = WrappingTextDisplay::new(mock_character_device);
        display.clear();
    }

    #[test]
    fn test_short_string() {
        let mut seq = Sequence::new();

        let mut mock_character_device = MockCharacterDisplay::new();

        let cursor_position = CursorPosition { row: 1, column: 2 };

        let text = "abc";

        let segment = Segment {
            position: cursor_position,
            length: 3,
        };

        expect_move_cursor(&mut mock_character_device, &mut seq, cursor_position);
        expect_write_string(&mut mock_character_device, &mut seq, text);

        let mut display = WrappingTextDisplay::new(mock_character_device);

        display.write_to(segment, text);
    }

    #[test]
    fn test_wrapping_string() {
        let mut seq = Sequence::new();

        let mut mock_character_device = MockCharacterDisplay::new();

        let first_line_cursor_position = CursorPosition { row: 1, column: 18 };
        let second_line_cursor_position = CursorPosition { row: 2, column: 0 };
        let first_line = "ab";
        let second_line = "cd";

        let segment = Segment {
            position: first_line_cursor_position,
            length: 4,
        };

        expect_move_cursor(
            &mut mock_character_device,
            &mut seq,
            first_line_cursor_position,
        );
        expect_write_string(&mut mock_character_device, &mut seq, first_line);
        expect_move_cursor(
            &mut mock_character_device,
            &mut seq,
            second_line_cursor_position,
        );
        expect_write_string(&mut mock_character_device, &mut seq, second_line);

        let mut display = WrappingTextDisplay::new(mock_character_device);

        display.write_to(segment, format_args!("{}{}", first_line, second_line));
    }

    #[test]
    fn test_write_to_end_of_line() {
        let mut seq = Sequence::new();

        let mut mock_character_device = MockCharacterDisplay::new();

        let cursor_position = CursorPosition { row: 1, column: 18 };
        let text = "ab";

        let segment = Segment {
            position: cursor_position,
            length: 2,
        };

        expect_move_cursor(&mut mock_character_device, &mut seq, cursor_position);
        expect_write_string(&mut mock_character_device, &mut seq, text);

        mock_character_device
            .expect_move_cursor()
            .never()
            .in_sequence(&mut seq);

        let mut display = WrappingTextDisplay::new(mock_character_device);

        display.write_to(segment, text);
    }

    #[test]
    fn multiple_writes_without_wrapping() {
        use std::convert::TryInto;

        let mut seq = Sequence::new();

        let mut mock_character_device = MockCharacterDisplay::new();

        let text_to_write = [
            (CursorPosition { row: 0, column: 15 }, "abcd"),
            (CursorPosition { row: 1, column: 14 }, "efghi"),
            (CursorPosition { row: 2, column: 13 }, "jklmno"),
        ];

        for &(cursor_position, text) in &text_to_write {
            expect_move_cursor(&mut mock_character_device, &mut seq, cursor_position);
            expect_write_string(&mut mock_character_device, &mut seq, text);
        }

        let mut display = WrappingTextDisplay::new(mock_character_device);

        for &(cursor_position, text) in &text_to_write {
            display.write_to(
                Segment {
                    position: cursor_position,
                    length: text.chars().count().try_into().unwrap(),
                },
                text,
            );
        }
    }
}
