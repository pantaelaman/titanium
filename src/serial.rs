use core::fmt::Write;

use crate::sync::{LazyLock, Mutex};
use uart_16550::SerialPort;

pub static SERIAL1: LazyLock<Mutex<SerialPort>> = LazyLock::new(&|| {
  let mut serial_port = unsafe { SerialPort::new(0x3f8) };
  serial_port.init();
  Mutex::new(serial_port)
});

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
  SERIAL1
    .lock()
    .write_fmt(args)
    .expect("Printing to serial failed");
}
