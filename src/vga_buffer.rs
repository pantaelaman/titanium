use crate::sync::{LazyLock, Mutex};
use volatile::Volatile;
use x86_64::instructions::port::Port;

pub static WRITER: LazyLock<Mutex<Writer>> = LazyLock::new(&|| {
  let mut writer = Writer {
    row_position: 0,
    col_position: 0,
    register_port: Port::new(0x3d4),
    value_port: Port::new(0x3d5),
    colour_code: ColourCode::new(Colour::LightCyan, Colour::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  };

  writer.set_cursor_height(0xf);
  writer.hide_cursor();

  Mutex::new(writer)
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
  col_position: usize,
  row_position: usize,
  register_port: Port<u8>,
  value_port: Port<u8>,
  colour_code: ColourCode,
  buffer: &'static mut Buffer,
}

impl Writer {
  #[doc(hidden)]
  fn _write_byte(&mut self, byte: u8) {
    match byte {
      b'\n' => self.new_line(),
      byte => {
        if self.col_position >= BUFFER_WIDTH {
          self.new_line();
        }

        let row = self.row_position;
        let col = self.col_position;
        let colour_code = self.colour_code;
        self.buffer.chars[row][col].write(ScreenChar {
          ascii_character: byte,
          colour_code,
        });
        self.col_position += 1;
      }
    }
  }

  pub fn write_byte(&mut self, byte: u8) {
    self._write_byte(byte);
    self.move_cursor_to_position(self.row_position, self.col_position);
  }

  fn new_line(&mut self) {
    self.col_position = 0;
    self.row_position += 1;
    if self.row_position >= BUFFER_HEIGHT {
      self.row_position = 0;
    }
    self.clear_line(self.row_position);
  }

  fn clear_line(&mut self, line: usize) {
    let stashed_col = self.col_position;
    let stashed_row = self.row_position;
    self.col_position = 0;
    self.row_position = line;
    for _ in 0..BUFFER_WIDTH {
      self.write_byte(b' ');
    }
    self.col_position = stashed_col;
    self.row_position = stashed_row;
  }

  pub fn write_string(&mut self, s: &str) {
    for byte in s.bytes() {
      match byte {
        0x20..=0x7e | b'\n' => self._write_byte(byte),
        _ => self._write_byte(0xfe),
      }
    }
    self.move_cursor_to_position(self.row_position, self.col_position);
  }

  pub fn hide_cursor(&mut self) {
    unsafe {
      self.register_port.write(0x0a);
      let value = self.value_port.read() | 0x20;
      self.value_port.write(value);
    }
  }

  pub fn show_cursor(&mut self) {
    unsafe {
      self.register_port.write(0x0a);
      let value = self.value_port.read() & (0xff ^ 0x20);
      self.value_port.write(value);
    }
  }

  pub fn move_cursor_to_position(&mut self, row: usize, col: usize) {
    let pos: u16 = (row * BUFFER_WIDTH + col) as u16;

    unsafe {
      self.register_port.write(0x0f);
      self.value_port.write((pos & 0xff) as u8);
      self.register_port.write(0x0e);
      self.value_port.write(((pos >> 8) & 0xff) as u8);
    }
  }

  pub fn set_cursor_height(&mut self, shape: u8) {
    unsafe {
      self.register_port.write(0x0a);
      let value = self.value_port.read() & 0xf0;
      self.value_port.write(value | (0xf - (shape & 0x0f)));
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
