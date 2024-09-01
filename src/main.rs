#![feature(const_mut_refs)]
#![feature(sync_unsafe_cell)]
#![no_std]
#![no_main]

mod io;
mod sync;
mod vga_buffer;

static WELCOME_TEXT: &str = "Welcome to TITANIUM";

#[no_mangle]
pub extern "C" fn _start() -> ! {
  println!("{}", WELCOME_TEXT);

  loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
  loop {}
}
