#![cfg_attr(test, no_main)]
#![test_runner(crate::test_runner)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![feature(const_mut_refs)]
#![feature(sync_unsafe_cell)]
#![feature(abi_x86_interrupt)]
#![no_std]

pub mod gdt;
pub mod idt;
pub mod io;
pub mod serial;
pub mod sync;
pub mod vga_buffer;

#[cfg(any(test, feature = "debug"))]
pub trait Testable {
  fn run(&self) {
    self.run_direct();
    serial_println!("[\x1b[0;32mok\x1b[0m]");
  }
  fn run_opposed(&self) {
    self.run_direct();
    serial_println!("[\x1b[0;31mfailed\x1b[0m]");
  }
  fn run_direct(&self);
}

#[cfg(any(test, feature = "debug"))]
impl<T: Fn()> Testable for T {
  fn run_direct(&self) {
    let name = core::any::type_name::<T>();
    serial_print!("{}...", name);
    for _ in name.len() + 3..60 {
      serial_print!(" ");
    }
    self();
  }
}

#[cfg(any(test, feature = "debug"))]
pub fn test_runner(tests: &[&dyn Testable]) {
  serial_println!("Running {} tests", tests.len());
  for test in tests {
    test.run();
  }

  exit_qemu(QemuExitCode::Success);
}

#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
  init();
  test_main();
  loop {}
}

#[cfg(any(test, feature = "debug"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
  Success = 0x10,
  Failure = 0x11,
}

#[cfg(any(test, feature = "debug"))]
pub fn exit_qemu(exit_code: QemuExitCode) {
  use x86_64::instructions::port::Port;

  unsafe {
    let mut port = Port::new(0xf4);
    port.write(exit_code as u32);
  }
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  serial_println!("[\x1b[0;31mfailed\x1b[0m]");
  serial_println!("{:#?}", info);
  exit_qemu(QemuExitCode::Failure);
  loop {}
}

pub fn init() {
  crate::idt::init();
  crate::gdt::init();
}
