#![feature(ptr_cast_array)]
#![no_std]
#![no_main]

mod limine;
mod serial;
mod framebuffer;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  serial_println!("--- PANIC! ---");
  serial_println!("{:#?}", info);
  loop {
    x86_64::instructions::hlt();
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
  assert!(limine::is_base_revision_supported());

  serial_println!("Welcome to Rune!");

  if let Some(framebuffer) = limine::framebuffers().and_then(|b| b.first()) {
    serial_println!(
      "Found framebuffer ({},{})",
      framebuffer.width,
      framebuffer.height
    );

    framebuffer::draw_icon(framebuffer);
  }

  loop {
    x86_64::instructions::hlt();
  }
}
