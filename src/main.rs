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

use ::acpi::{
  MadtError, aml::namespace::AmlName, platform::AcpiPlatform, sdt::{
    SdtHeader,
    hpet::HpetTable,
    madt::{Madt, MadtEntry},
  }
};
use alloc::vec;
use crossbeam::epoch::Pointable;
use spin::Mutex;
use x86_64::{
  PhysAddr, VirtAddr, registers::control::Cr4Flags,
  structures::paging::OffsetPageTable,
};

mod uacpi;
mod acpi;
mod executor;
mod framebuffer;
mod gdt;
mod hdconf;
mod hdint;
mod heap;
mod idt;
mod limine;
mod paging;
mod queue;
mod serial;
mod sys;
mod util;
mod vmem;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
  serial_println!("--- PANIC! ---");
  serial_println!("{:#?}", info);
  loop {
    x86_64::instructions::hlt();
  }
}

pub struct MapperAllocator<'a> {
  pub mapper: OffsetPageTable<'a>,
  pub allocator: paging::RuneFrameAllocator<'a>,
}

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

  idt::init_interrupts();

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
  let mut frame_allocator = paging::RuneFrameAllocator::new(memmap_entries);

  let lapic = unsafe {
    let lapic = hdint::init_local(&mut mapper, &mut frame_allocator);

    heap::init(&mut mapper, &mut frame_allocator)
      .expect("couldn't initialise the heap");

    lapic
  };

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

  let acpi_tables = unsafe { acpi::init() };

  let Some(madt_frame) = acpi_tables.find_table::<Madt>() else {
    panic!("no madt table found");
  };

  let Some(hpet_frame) = acpi_tables.find_table::<HpetTable>() else {
    panic!("no hpet table found");
  };

  let mut mapper = MapperAllocator {
    mapper,
    allocator: frame_allocator,
  };

  let mut ioapic = madt_frame
    .get()
    .entries()
    .find_map(|entry| {
      if let MadtEntry::IoApic(entry) = entry {
        Some(unsafe { hdint::IOApic::new(entry, &mut mapper) })
      } else {
        None
      }
    })
    .expect("no IOAPIC exists on this system");

  for ovr in madt_frame.get().entries().filter_map(|entry| {
    if let MadtEntry::InterruptSourceOverride(ovr) = entry {
      Some(ovr)
    } else {
      None
    }
  }) {
    serial_println!("IOAPIC Override: {:?}", ovr);
    ioapic.override_irq(ovr.irq as usize, ovr.global_system_interrupt as usize);
  }

  let mut pit = ioapic.init_pit();
  unsafe {
    lapic.setup_timer_pit(&mut pit);
  }
  // don't need it after setting up the lapic's timer
  ioapic.disable_pit();

  let acpi_platform =
    AcpiPlatform::new(acpi_tables, &acpi::ACPIHandler {}).unwrap();
  let interpreter =
    ::acpi::aml::Interpreter::new_from_platform(&acpi_platform).unwrap();
  let result = interpreter.evaluate(AmlName::root(), vec![]).unwrap();

  loop {
    x86_64::instructions::hlt();
  }
}

async fn test() {
  async fn test_inner() -> usize {
    return 10;
  }

  serial_println!("tested async: {}", test_inner().await);
}
