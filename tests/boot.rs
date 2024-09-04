#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
  test_main();

  loop {}
}

fn test_runner(tests: &[&dyn titanium::Testable]) {
  for test in tests {
    test.run();
  }
  titanium::exit_qemu(titanium::QemuExitCode::Success);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
  titanium::serial_println!("[\x1b[0;31mfailed\x1b[0m]");
  titanium::exit_qemu(titanium::QemuExitCode::Failure);
  loop {}
}

#[test_case]
fn println() {
  titanium::println!("simple println test");
}
