#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![feature(const_mut_refs)]
#![feature(sync_unsafe_cell)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

use vga_buffer::WRITER;

mod io;
mod sync;
mod vga_buffer;

static WELCOME_TEXT: &str = "Welcome to TITANIUM";

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
  println!("Running {} tests", tests.len());
  for test in tests {
    test();
  }

  exit_qemu(QemuExitCode::Success);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
  println!("{}", WELCOME_TEXT);
  WRITER.lock().show_cursor();

  #[cfg(test)]
  test_main();

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
fn panic(_info: &core::panic::PanicInfo) -> ! {
  loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
  exit_qemu(QemuExitCode::Failure);
  loop {}
}

#[test_case]
fn trivial_assertion() {
  print!("Trivial assertion...");
  assert_eq!(1, 1);
  println!("[ok]");
}
