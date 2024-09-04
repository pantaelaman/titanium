#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
  test_main();

  loop {}
}

fn test_runner(tests: &[&dyn titanium::Testable]) {
  for test in tests {
    test.run_opposed();
  }
  titanium::exit_qemu(titanium::QemuExitCode::Failure);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
  titanium::serial_println!("[\x1b[0;32mok\x1b[0m]");
  titanium::exit_qemu(titanium::QemuExitCode::Success);
  loop {}
}

#[test_case]
#[should_panic]
fn panicky() {
  assert_eq!(1, 0);
}
