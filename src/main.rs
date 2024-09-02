#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![feature(const_mut_refs)]
#![feature(sync_unsafe_cell)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

use vga_buffer::WRITER;

mod io;
mod serial;
mod sync;
mod vga_buffer;

static WELCOME_TEXT: &str = "Welcome to TITANIUM";

pub trait Testable {
  fn run(&self);
}

impl<T: Fn()> Testable for T {
  fn run(&self) {
    let name = core::any::type_name::<T>();
    serial_print!("{}...", name);
    for _ in name.len() + 3..60 {
      serial_print!(" ");
    }
    self();
    serial_println!("[\x1b[0;32mok\x1b[0m]");
  }
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
  serial_println!("Running {} tests", tests.len());
  for test in tests {
    test.run();
  }

  exit_qemu(QemuExitCode::Success);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
  #[cfg(test)]
  test_main();

  println!("{}", WELCOME_TEXT);
  WRITER.lock().show_cursor();

  loop {}
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
  Success = 0x10,
  Failure = 0x11,
}

#[cfg(test)]
pub fn exit_qemu(exit_code: QemuExitCode) {
  use x86_64::instructions::port::Port;

  unsafe {
    let mut port = Port::new(0xf4);
    port.write(exit_code as u32);
  }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  use vga_buffer::{Colour, ColourCode};

  WRITER.lock().colour_code = ColourCode::new(Colour::LightRed, Colour::Black);
  println!("{:#?}", info);
  loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  serial_println!("[\x1b[0;31mfailed\x1b[0m]");
  serial_println!("{:#?}", info);
  exit_qemu(QemuExitCode::Failure);
  loop {}
}

#[test_case]
fn trivial_assertion() {
  assert_eq!(1, 1);
}
