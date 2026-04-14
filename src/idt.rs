use core::mem::MaybeUninit;

use x86_64::{
  VirtAddr,
  registers::segmentation::{self, Segment},
  structures::{DescriptorTablePointer, gdt::SegmentSelector},
};

use crate::{serial_print, serial_println};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Options(pub u16);

impl Options {
  pub fn empty() -> Self {
    let mut raw = 0;
    raw |= 0b111 << 9; // set the ones bits
    Self(raw)
  }

  pub fn new() -> Self {
    let mut options = Self::empty();
    options.0 |= 1 << 15; // make it present
    options
  }

  pub fn is_present(&self) -> bool {
    self.0 & (1 << 15) != 0
  }

  pub fn is_trap(&self) -> bool {
    self.0 & (1 << 8) != 0
  }

  pub fn with_trap(&mut self, trapped: bool) -> &mut Self {
    self.0 = (self.0 & !(1 << 8)) | ((trapped as u16) << 8);
    self
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Entry {
  ptr_low: u16,
  gdt_sel: SegmentSelector,
  opts: Options,
  ptr_mid: u16,
  ptr_top: u32,
  _reserved: u32,
}

impl Entry {
  pub fn empty() -> Self {
    Self {
      ptr_low: 0,
      ptr_mid: 0,
      ptr_top: 0,
      _reserved: 0,
      gdt_sel: SegmentSelector::NULL,
      opts: Options::empty(),
    }
  }

  pub fn new(gdt_sel: SegmentSelector, handler: HandlerFn) -> Self {
    let ptr = handler as u64;
    Self {
      gdt_sel,
      ptr_low: ptr as u16,
      ptr_mid: (ptr >> 16) as u16,
      ptr_top: (ptr >> 32) as u32,
      opts: Options::new(),
      _reserved: 0,
    }
  }
}

type HandlerFn = extern "C" fn() -> !;

macro_rules! handler {
  ($name:ident) => {{
    #[unsafe(naked)]
    extern "C" fn wrapper() -> ! {
      core::arch::naked_asm! {
        "mov rdi, rsp",
        "call {}",
        "iretq",
        sym $name,
      };
    }

    wrapper
  }};
}

macro_rules! handler_with_ec {
  ($name:ident) => {{
    #[unsafe(naked)]
    extern "C" fn wrapper() -> ! {
      core::arch::naked_asm! {
        "pop rsi",
        "mov rdi, rsp",
        "call {}",
        "iretq",
        sym $name,
      };
    }

    wrapper
  }}
}

#[derive(Debug)]
#[repr(C)]
struct InterruptStackFrame {
  ip: u64,
  cs: u64,
  rflags: u64,
  sp: u64,
  ss: u64,
}

extern "C" fn double_fault_handler(stack_frame: *const InterruptStackFrame, ec: u64) -> ! {
  let stack_frame = unsafe { &*stack_frame };
  serial_println!("=== DOUBLE FAULT ===");
  serial_println!("{:#?}", stack_frame);

  loop {
    x86_64::instructions::hlt();
  }
}

extern "C" fn breakpoint_handler(stack_frame: *const InterruptStackFrame) {
  let stack_frame = unsafe { &*stack_frame };
  serial_println!("=== BREAKPOINT ===");
  serial_println!("{:#?}", stack_frame);
}

extern "C" fn page_fault_handler(stack_frame: *const InterruptStackFrame, ec: u64) -> ! {
  let stack_frame = unsafe { &*stack_frame };
  serial_println!("=== PAGE FAULT ===");
  serial_println!("EC: {}", ec);
  serial_println!("{:#?}", stack_frame);

  loop {
    x86_64::instructions::hlt();
  }
}

struct InterruptDescriptorTable {
  entries: [Entry; 16],
}

impl InterruptDescriptorTable {
  pub fn new() -> Self {
    Self {
      entries: [Entry::empty(); 16],
    }
  }

  pub fn set_handler(&mut self, entry: u8, handler: HandlerFn) -> &mut Options {
    self.entries[entry as usize] =
      Entry::new(segmentation::CS::get_reg(), handler);
    &mut self.entries[entry as usize].opts
  }

  pub fn load(&'static self) {
    let ptr = DescriptorTablePointer {
      base: VirtAddr::from_ptr(self as *const _),
      limit: (size_of::<Self>() - 1) as u16,
    };

    unsafe {
      x86_64::instructions::tables::lidt(&ptr);
    }
  }
}

static mut IDT: MaybeUninit<InterruptDescriptorTable> = MaybeUninit::uninit();

pub fn init_interrupts() {
  let idt = unsafe {
    #[allow(static_mut_refs)]
    IDT.write(InterruptDescriptorTable::new())
  };

  idt.set_handler(3, handler!(breakpoint_handler)).with_trap(false);
  idt.set_handler(8, handler_with_ec!(double_fault_handler)).with_trap(true);
  idt.set_handler(14, handler_with_ec!(page_fault_handler)).with_trap(true);
  idt.load();

  x86_64::instructions::interrupts::enable();
}
