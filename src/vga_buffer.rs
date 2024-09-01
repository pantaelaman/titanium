use crate::sync::{LazyLock, Mutex};
use volatile::Volatile;

pub static WRITER: LazyLock<Mutex<Writer>> = LazyLock::new(&|| {
  Mutex::new(Writer {
    column_position: 0,
    colour_code: ColourCode::new(Colour::LightCyan, Colour::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  })
});

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Colour {
  Black = 0,
  Blue,
  Green,
  Cyan,
  Red,
  Magenta,
  Brown,
  LightGray,
  DarkGray,
  LightBlue,
  LightGreen,
  LightCyan,
  LightRed,
  Pink,
  Yellow,
  White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColourCode(u8);

impl ColourCode {
  const fn new(foreground: Colour, background: Colour) -> Self {
    ColourCode((background as u8) << 4 | foreground as u8)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
  ascii_character: u8,
  colour_code: ColourCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
  chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
  column_position: usize,
  colour_code: ColourCode,
  buffer: &'static mut Buffer,
}

impl Writer {
  pub fn write_byte(&mut self, byte: u8) {
    match byte {
      b'\n' => self.new_line(),
      byte => {
        if self.column_position >= BUFFER_WIDTH {
          self.new_line();
        }

        let row = BUFFER_HEIGHT - 1;
        let col = self.column_position;
        let colour_code = self.colour_code;
        self.buffer.chars[row][col].write(ScreenChar {
          ascii_character: byte,
          colour_code,
        });
        self.column_position += 1;
      }
    }
  }

  fn new_line(&mut self) {
    self.column_position = 0;
  }

  pub fn write_string(&mut self, s: &str) {
    for byte in s.bytes() {
      match byte {
        0x20..=0x7e | b'\n' => self.write_byte(byte),
        _ => self.write_byte(0xfe),
      }
    }
  }
}

impl core::fmt::Write for Writer {
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    self.write_string(s);
    Ok(())
  }
}

#[macro_export]
macro_rules! println {
  () => {
    print!("\n")
  };
  ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
  ($($arg:tt)*) => {
    $crate::io::_print(format_args!($($arg)*))
  };
}
