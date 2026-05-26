#![feature(int_roundings)]
#![feature(coroutines)]
#![feature(iter_from_coroutine)]
#![feature(cstr_display)]
#![feature(generic_atomic)]
#![feature(abi_x86_interrupt)]
#![feature(ptr_metadata)]
#![feature(ptr_cast_array)]
#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

use core::mem::MaybeUninit;

use alloc::vec;
use crossbeam::epoch::Pointable;
use spin::Mutex;
use x86_64::{
  PhysAddr, VirtAddr,
  registers::control::Cr4Flags,
  structures::paging::{
    Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB, mapper::{MapToError, MapperFlush}
  },
};

mod acpi;
mod framebuffer;
mod gdt;
mod hdconf;
mod hdint;
mod heap;
mod idt;
mod limine;
mod paging;
mod queue;
mod scheduler;
mod serial;
mod stack;
mod sys;
mod uacpi;
mod util;
mod vmem;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  x86_64::instructions::interrupts::disable();
  serial_println!("--- PANIC! ---");
  serial_println!("{:#?}", info);
  loop {
    x86_64::instructions::hlt();
  }
}

pub use paging::MapperAllocator;

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
  assert!(limine::is_base_revision_supported());
  assert!(limine::paging_mode() == limine::PagingMode::L4);

  let hhdm_offset = VirtAddr::new(limine::hhdm_offset());

  serial_println!("Welcome to Rune!");
  serial_println!("HHDM offset: {:?}", hhdm_offset);

  unsafe {
    gdt::init();
  }

  idt::init_interrupts();

  let memmap_entries =
    limine::memmap_entries().expect("couldn't load memmap entries");
  serial_println!("Entry count: {}", memmap_entries.len());
  for entry in memmap_entries {
    serial_println!(
      "Entry {:?} +{} ({:?})",
      entry.base,
      entry.length,
      entry.ty
    );
  }

  // pretty sure I don't need this now that I have a custom target
  // this would be to enable legacy SSE instructions
  // unsafe {
  //   x86_64::registers::control::Cr4::update(|flags| {
  //     *flags |= Cr4Flags::OSFXSR;
  //   });
  // }

  serial_println!(
    "are interrupts enabled? {}",
    x86_64::instructions::interrupts::are_enabled()
  );

  let l4_page_table = unsafe { paging::active_l4_page_table(hhdm_offset) };
  for (i, entry) in l4_page_table
    .iter()
    .enumerate()
    .filter(|(_, entry)| !entry.is_unused())
  {
    serial_println!("Page {}: {:?}", i, entry);
  }

  let mut mapper = unsafe { paging::init(hhdm_offset) };
  let frame_allocator = paging::RuneFrameAllocator::new(memmap_entries);

  let mut mapper = MapperAllocator {
    mapper,
    allocator: frame_allocator,
  };

  unsafe { heap::init(&mut mapper) };

  let lapic = unsafe { hdint::init_local(&mut mapper) };

  if let Some(framebuffer) =
    crate::limine::framebuffers().and_then(|b| b.first())
  {
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

  let uacpi_ctx = unsafe { acpi::init_acpi() }.unwrap();
  let madt_handle = uacpi_ctx.madt();
  let madt = madt_handle.as_ref();
  serial_println!("MADT: {:?}", madt);
  for entry in madt.entries() {
    serial_println!("Entry: {:?}", entry);
  }

  let mut ioapic = madt
    .entries()
    .find_map(|entry| {
      if let acpi::madt::MADTEntry::IOApic(entry) = entry {
        Some(unsafe { hdint::IOApic::new(entry, &mut mapper) })
      } else {
        None
      }
    })
    .expect("no IOAPIC exists on this system");

  for entry in madt.entries() {
    if let acpi::madt::MADTEntry::IOApicOverride(entry) = entry {
      ioapic.register_override(entry);
    }
  }

  let mut pit = ioapic.init_pit();
  unsafe {
    lapic.setup_timer_pit(&mut pit);
  }
  // don't need it after setting up the lapic's timer
  ioapic.disable_pit();

  serial_println!("before seeking the hpet");
  let mut hpet = {
    let hpet_handle = uacpi_ctx.hpet();
    let hpet = hpet_handle.as_ref();
    ioapic.init_hpet(hpet, &mut mapper)
  };
  unsafe {
    hpet.enable();
  }

  loop {
    x86_64::instructions::hlt();
  }
}
