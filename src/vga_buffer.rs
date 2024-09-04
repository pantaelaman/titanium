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
    stashed_colour_code: ColourCode::new(Colour::LightCyan, Colour::Black),
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

impl Into<ColourCode> for Colour {
  fn into(self) -> ColourCode {
    ColourCode::new_fg(self)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColourCode(u8);

impl ColourCode {
  pub const fn new(foreground: Colour, background: Colour) -> Self {
    ColourCode((background as u8) << 4 | foreground as u8)
  }

  pub const fn new_fg(foreground: Colour) -> Self {
    Self::new(foreground, Colour::Black)
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
  pub col_position: usize,
  pub row_position: usize,
  register_port: Port<u8>,
  value_port: Port<u8>,
  pub colour_code: ColourCode,
  stashed_colour_code: ColourCode,
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

  pub fn set_temp_colour_code(&mut self, colour_code: ColourCode) {
    self.stashed_colour_code = self.colour_code;
    self.colour_code = colour_code;
  }

  pub fn release_temp_colour_code(&mut self) {
    self.colour_code = self.stashed_colour_code;
  }
}

impl core::fmt::Write for Writer {
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    self.write_string(s);
    Ok(())
  }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
  use core::fmt::Write;
  crate::vga_buffer::WRITER.lock().write_fmt(args).unwrap();
}

#[cfg(test)]
#[test_case]
fn println_simple() {
  crate::println!("simple println test");
}

#[cfg(test)]
#[test_case]
fn println_many() {
  for _ in 0..200 {
    crate::println!("many println test");
  }
}

#[cfg(test)]
#[test_case]
fn println_output() {
  let s = "println output test";
  crate::println!("\n{}", s);
  for (i, c) in s.chars().enumerate() {
    let writer = WRITER.lock();
    let screen_char = writer.buffer.chars[writer.row_position - 1][i].read();
    assert_eq!(char::from(screen_char.ascii_character), c);
  }
}

#[cfg(test)]
#[test_case]
fn println_colour() {
  let mut writer = WRITER.lock();
  let colour_code = ColourCode::new(Colour::LightGreen, Colour::Red);
  writer.colour_code = colour_code;

  crate::print!("\nc");
  assert_eq!(
    colour_code,
    writer.buffer.chars[writer.row_position][writer.col_position - 1]
      .read()
      .colour_code
  )
}

#[cfg(test)]
#[test_case]
fn cursor_position() {
  let mut writer = WRITER.lock();
  writer.move_cursor_to_position(14, 3);

  let mut pos: u16 = 0;
  unsafe {
    writer.register_port.write(0x0f);
    pos |= writer.value_port.read() as u16;
    writer.register_port.write(0x0e);
    pos |= (writer.value_port.read() as u16) << 8;
  }
  assert_eq!(3, pos as usize % BUFFER_WIDTH);
  assert_eq!(14, pos as usize / BUFFER_WIDTH);
}

#[cfg(test)]
#[test_case]
fn cursor_height() {
  let mut writer = WRITER.lock();
  writer.set_cursor_height(0x3);
  let height;
  unsafe {
    writer.register_port.write(0x0a);
    height = writer.value_port.read() & 0xf;
  }
  assert_eq!(0xf - 0x3, height)
}
