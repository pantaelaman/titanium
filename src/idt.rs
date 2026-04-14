use core::mem::MaybeUninit;

use x86_64::{VirtAddr, registers::segmentation::{self, Segment}, structures::{DescriptorTablePointer, gdt::SegmentSelector}};

use crate::serial_println;

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
    options.0 |= 1 << 8; // trap type (disable interrupts)
    options
  }

  pub fn is_present(&self) -> bool {
    self.0 & (1 << 15) != 0
  }

  pub fn is_trap(&self) -> bool {
    self.0 & (1 << 8) != 0
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

extern "C" fn default_handler() -> ! {
  serial_println!("=== INTERRUPT TRIGGERED ===");

  loop {
    x86_64::instructions::hlt();
  }
}

#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame {
  ip: u64,
  cs: u64,
  rflags: u64,
  sp: u64,
  ss: u64,
}

#[unsafe(naked)]
extern "C" fn double_fault_handler() -> ! {
  core::arch::naked_asm! {
    "pop rsi",
    "mov rdi, rsp",
    "call {}",
    sym handler,
  };

  extern "C" fn handler(stack_frame: *const ExceptionStackFrame, ec: u64) -> ! {
    let stack_frame = unsafe { &*stack_frame };
    serial_println!("=== DOUBLE FAULT ===");
    serial_println!("{:#?}", stack_frame);

    loop {
      x86_64::instructions::hlt();
    }
  }
}

#[unsafe(naked)]
extern "C" fn breakpoint_handler() -> ! {
  core::arch::naked_asm! {
    "mov rdi, rsp",
    "call {}",
    "iretq",
    sym handler,
  };

  extern "C" fn handler(stack_frame: *const ExceptionStackFrame) {
    let stack_frame = unsafe { &*stack_frame };
    serial_println!("=== BREAKPOINT ===");
    serial_println!("{:#?}", stack_frame);
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
    self.entries[entry as usize] = Entry::new(segmentation::CS::get_reg(), handler);
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

  idt.set_handler(3, breakpoint_handler);
  idt.set_handler(8, double_fault_handler);
  idt.load();

  x86_64::instructions::interrupts::enable();
}
