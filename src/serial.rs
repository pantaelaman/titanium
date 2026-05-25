use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;

lazy_static! {
  pub static ref SERIAL1: Mutex<SerialPort> = {
    let mut serial_port = unsafe { SerialPort::new(0x3f8) };
    serial_port.init();
    Mutex::new(serial_port)
  };
}

pub unsafe extern "C" fn debug_dot() {
  use core::fmt::Write;

  if let Some(mut lock) = SERIAL1.try_lock() {
    lock.write_char('.').expect("Printing to serial failed");
  }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
  x86_64::instructions::interrupts::disable();
  use core::fmt::Write;
  SERIAL1
    .lock()
    .write_fmt(args)
    .expect("Printing to serial failed");
  x86_64::instructions::interrupts::enable();
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
