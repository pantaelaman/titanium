#![feature(abi_x86_interrupt)]
#![feature(ptr_metadata)]
#![feature(ptr_cast_array)]
#![no_std]
#![no_main]

use x86_64::{
  VirtAddr,
  registers::rflags::RFlags,
  structures::{
    gdt::SegmentSelector,
    idt::{InterruptDescriptorTable, InterruptStackFrame},
  },
};

mod framebuffer;
mod idt;
mod limine;
mod serial;
mod util;

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

  idt::init_interrupts();

  serial_println!(
    "are interrupts enabled? {}",
    x86_64::instructions::interrupts::are_enabled()
  );

  if let Some(framebuffer) = limine::framebuffers().and_then(|b| b.first()) {
    serial_println!(
      "Found framebuffer ({},{})",
      framebuffer.width,
      framebuffer.height
    );

    framebuffer::draw_icon(framebuffer);
    framebuffer::print_str_at(
      framebuffer,
      framebuffer::Point { x: 0, y: 0 },
      "Hello, world!",
    );
  }

  x86_64::instructions::interrupts::int3();

  serial_println!("made it past the breakpoint");

  loop {
    x86_64::instructions::hlt();
  }
}
