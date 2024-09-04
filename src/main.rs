#![feature(custom_test_frameworks)]
#![test_runner(titanium::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(const_mut_refs)]
#![feature(sync_unsafe_cell)]
#![no_std]
#![no_main]

use titanium::*;
use vga_buffer::WRITER;

static WELCOME_TEXT: &str = "Welcome to TITANIUM";

#[no_mangle]
pub extern "C" fn _start() -> ! {
  init();

  #[cfg(test)]
  test_main();

  println!("{}", WELCOME_TEXT);
  WRITER.lock().show_cursor();

  x86_64::instructions::interrupts::int3();

  loop {
    x86_64::instructions::hlt();
  }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  use vga_buffer::{Colour, ColourCode};

  WRITER.lock().colour_code = Colour::LightRed.into();
  println!("{}", info);
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
