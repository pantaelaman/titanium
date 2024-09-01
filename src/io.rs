#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
  use core::fmt::Write;
  crate::vga_buffer::WRITER.lock().write_fmt(args).unwrap();
}
