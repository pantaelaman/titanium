use acpi::sdt::hpet::HpetTable;
use bitfield::bitfield;
use x86_64::{
  PhysAddr, VirtAddr,
  structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{MapperAllocator, bitfield_volatile_bitrange, serial_print, serial_println, vmem::HPET_ADDR};

pub struct HPET {
  addr: VirtAddr,
  minimal_tick: u16,
}

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct Capabilities(u64);
  no default BitRange;
  impl Debug;

  pub u32, period_fs, _ : 63, 32;
  pub u16, vendor_id, _ : 31, 16;
  pub supports_legrep, _ : 15; // supports legacy replacement irq mode
  pub supports_64bit, _ : 13;
  pub u8, last_timer, _ : 12, 8;
  pub u8, rev_id, _ : 7, 0;
}

crate::bitfield_volatile_bitrange!(struct Capabilities(u64));

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct Configuration(u64);
  no default BitRange;
  impl Debug;

  pub is_legrep_enabled, set_legrep_enabled : 1;
  pub is_enabled, set_enabled : 0;
}

crate::bitfield_volatile_bitrange!(struct Configuration(u64));

bitfield! {
  #[repr(transparent)]
  pub struct TimerConfig(u64);
  no default BitRange;
  impl Debug;

  pub u32, irq_capabilities, _ : 63, 32;
  pub supports_fsb, _ : 15;
  pub fsb, set_fsb : 14;
  pub u8, active_irq, set_active_irq : 13, 9;
  pub forced_32bit, set_32bit : 8;
  pub _, set_cnf : 6;
  pub is_64bit, _ : 5;
  pub supports_periodic, _ : 4;
  pub is_periodic, set_periodic : 3;
  pub is_irq_enabled, set_irq_enabled : 2;
  pub is_level, set_level : 1;
}

crate::bitfield_volatile_bitrange!(struct TimerConfig(u64));

#[repr(C)]
pub struct HPETTimer {
  pub config: TimerConfig,
  comparator: u64,
  _fsb: u64,
  _padding: u64,
}

impl HPETTimer {
  #[inline]
  pub fn comparator(&self) -> u64 {
    unsafe {
      (&self.comparator as *const u64).read_volatile()
    }
  }

  #[inline]
  pub fn set_comparator(&mut self, value: u64) {
    unsafe {
      (&mut self.comparator as *mut u64).write_volatile(value);
    }
  }
}

impl HPET {
  const CAPABILITIES_OFFSET: u64 = 0x000;
  const CONFIGURATION_OFFSET: u64 = 0x010;
  const INTERRUPT_STATUS_OFFSET: u64 = 0x020;
  /// Main counter value register
  const MC_VALUE_OFFSET: u64 = 0x0f0;
  /// Offset to first timer configuration
  const TIMER_BASE_OFFSET: u64 = 0x100;

  pub fn new(table: &HpetTable, mapper: &mut MapperAllocator) -> Self {
    let frame: PhysFrame<Size4KiB> =
      PhysFrame::from_start_address(PhysAddr::new(table.base_address.address)).unwrap();
    let page = Page::containing_address(HPET_ADDR);
    unsafe {
      mapper
        .mapper
        .map_to(
          page,
          frame,
          PageTableFlags::PRESENT
            | PageTableFlags::WRITE_THROUGH
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_CACHE
            | PageTableFlags::NO_EXECUTE,
          &mut mapper.allocator,
        )
        .unwrap()
        .flush();
    }

    Self {
      addr: page.start_address(),
      minimal_tick: table.clock_tick_unit,
    }
  }

  #[inline]
  pub fn capabilities(&self) -> &Capabilities {
    unsafe {
      &*(self.addr + Self::CAPABILITIES_OFFSET).as_ptr::<Capabilities>()
    }
  }

  #[inline]
  pub fn timer(&self, index: u8) -> &HPETTimer {
    unsafe {
      &*(self.addr + Self::TIMER_BASE_OFFSET + 0x20 * index as u64).as_ptr()
    }
  }

  #[inline]
  fn timer_mut(&mut self, index: u8) -> &mut HPETTimer {
    unsafe {
      &mut *(self.addr + Self::TIMER_BASE_OFFSET + 0x20 * index as u64).as_mut_ptr()
    }
  }

  #[inline]
  pub fn counter(&self) -> u64 {
    unsafe {
      (self.addr + Self::MC_VALUE_OFFSET).as_ptr::<u64>().read_volatile()
    }
  }

  #[inline]
  pub fn interrupt_status(&self) -> u32 {
    let reg = unsafe {
      (self.addr + Self::INTERRUPT_STATUS_OFFSET).as_ptr::<u64>().read_volatile()
    };

    (reg & 0xffff_ffff) as u32
  }

  #[inline]
  fn configuration(&mut self) -> &mut Configuration {
    unsafe {
      &mut *(self.addr + Self::CONFIGURATION_OFFSET).as_mut_ptr()
    }
  }

  /// Enables a certain timer (by index) to start sending interrupts.
  /// Interrupts will be disabled during this function, which may break
  /// expected behaviour elsewhere, hence the unsafety.
  /// This function will panic if `on_irq` does not match an available
  /// irq for the timer.
  pub unsafe fn enable_timer(&mut self, index: u8, on_irq: u8, period_fs: u64) {
    // disable interrupts while configuring timers
    self.configuration().set_enabled(false);

    let curcount = self.counter();
    let irq_bit = 1 << on_irq;
    let timer = self.timer_mut(index);

    // ensure that we can enable on this irq
    assert!(timer.config.irq_capabilities() & irq_bit != 0);
    timer.config.set_active_irq(on_irq);
    timer.config.set_irq_enabled(true);
    timer.config.set_periodic(true);
    timer.config.set_cnf(true);
    // set initial count activation thanks to `set_cnf`
    timer.set_comparator(curcount.wrapping_add(period_fs));
    // set additional count activation (normal behaviour)
    timer.set_comparator(period_fs);
  }

  pub unsafe fn enable(&mut self) {
    // enable interrupts
    self.configuration().set_enabled(true);
  }

  pub fn disable(&mut self) {
    self.configuration().set_enabled(false);
  }
}

pub extern "C" fn hpet_interrupt() {
  serial_print!(".");
}
