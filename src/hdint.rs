use core::sync::atomic::Ordering;

use crate::{
  MapperAllocator, idt, serial_print, vmem::{APIC_NEXT_OFFSET, APIC_START, LAPIC_ADDR}
};
use x86_64::{
  PhysAddr, VirtAddr,
  structures::{
    idt::PageFaultErrorCode,
    paging::{
      FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB,
    },
  },
};

use crate::serial_println;

pub mod pit;
pub mod rtc;
pub mod hpet;

pub const GLOBAL_PIT: usize = 0;
pub const GLOBAL_RTC: usize = 8;
pub const GLOBAL_KBD: usize = 1;

pub struct LocalApic {
  addr: VirtAddr,
}

impl LocalApic {
  pub const LAPIC_ID_OFFSET: u64 = 0x20;
  pub const LAPIC_VERSION_OFFSET: u64 = 0x30;
  pub const SPURIOUS_OFFSET: u64 = 0xf0;
  pub const EOI_OFFSET: u64 = 0xb0;
  pub const EOI_FLAG: u32 = 0;
  pub const LVT_TIMER_OFFSET: u64 = 0x320;
  pub const TMRINITCNT_OFFSET: u64 = 0x380;
  pub const TMRCURRCNT_OFFSET: u64 = 0x390;
  pub const TMRDIV_OFFSET: u64 = 0x3e0;

  pub const TIMER_MODE_PERIODIC: u32 = 0x20000;

  pub const LVT_INT_MASKED: u32 = 1 << 16;
  pub const LVT_INT_UNMASKED: u32 = 0 << 16;

  pub fn id(&self) -> u32 {
    unsafe {
      (self.addr + Self::LAPIC_ID_OFFSET)
        .as_mut_ptr::<u32>()
        .read_volatile()
    }
  }

  #[inline]
  fn write(&self, offset: u64, value: u32) {
    unsafe {
      (self.addr + offset)
        .as_mut_ptr::<u32>()
        .write_volatile(value);
    }
  }

  #[inline]
  fn read(&self, offset: u64) -> u32 {
    unsafe { (self.addr + offset).as_ptr::<u32>().read_volatile() }
  }

  pub fn set_spurious(&mut self, spurious: u32) {
    self.write(Self::SPURIOUS_OFFSET, spurious);
  }

  pub fn spurious(&self) -> u32 {
    self.read(Self::SPURIOUS_OFFSET)
  }

  pub unsafe fn eoi(&self) {
    self.write(Self::EOI_OFFSET, Self::EOI_FLAG);
  }

  pub unsafe fn get() -> Self {
    Self { addr: LAPIC_ADDR }
  }

  pub unsafe fn setup_timer_rtc(&self, rtc: &mut rtc::RTC) {
    self.write(Self::TMRDIV_OFFSET, 0x3);
    rtc.set_rate(6);

    self.write(Self::TMRINITCNT_OFFSET, 0xffff_ffff);
    rtc::TICKS_SINCE_BOOT.store(0, Ordering::Release);
    while rtc::TICKS_SINCE_BOOT.load(Ordering::Acquire) < 512 {
      x86_64::instructions::hlt();
    }
    self.write(Self::LVT_TIMER_OFFSET, Self::LVT_INT_MASKED);
    let ticks_per_halfsec = 0xffff_ffff - self.read(Self::TMRCURRCNT_OFFSET);
    serial_println!("ticks per halfsec: {:?}", ticks_per_halfsec);
    let ticks_per_ms = ticks_per_halfsec / 500;
    serial_println!("ticks per ms: {:?}", ticks_per_ms);

    self.write(Self::LVT_TIMER_OFFSET, idt::IRQ_LAPIC_CLK as u32 | Self::TIMER_MODE_PERIODIC);
    self.write(Self::TMRDIV_OFFSET, 0x3);
    self.write(Self::TMRINITCNT_OFFSET, ticks_per_ms);
  }

  pub unsafe fn setup_timer_pit(&self, pit: &mut pit::PIT) {
    self.write(Self::TMRDIV_OFFSET, 0x3);
    self.write(Self::TMRINITCNT_OFFSET, 0xffff_ffff);
    pit.sleep_us(10000);
    self.write(Self::LVT_TIMER_OFFSET, Self::LVT_INT_MASKED);
    let ticks_per_10ms = 0xffff_ffff - self.read(Self::TMRCURRCNT_OFFSET);
    self.write(Self::LVT_TIMER_OFFSET, idt::IRQ_LAPIC_CLK as u32 | Self::TIMER_MODE_PERIODIC);
    self.write(Self::TMRDIV_OFFSET, 0x3);
    self.write(Self::TMRINITCNT_OFFSET, ticks_per_10ms);
  }
}

pub extern "C" fn lapic_clk_interrupt() {
  serial_print!(".");
}

pub unsafe fn init_local(
  mapper: &mut impl Mapper<Size4KiB>,
  frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> LocalApic {
  let (apic_phys_frame, _) =
    x86_64::registers::model_specific::ApicBase::read();

  serial_println!("found local apic at frame {:?}", apic_phys_frame);

  unsafe {
    mapper
      .map_to(
        Page::from_start_address(LAPIC_ADDR).expect("invalid page boundary"),
        apic_phys_frame,
        PageTableFlags::PRESENT
          | PageTableFlags::WRITABLE
          | PageTableFlags::NO_CACHE
          | PageTableFlags::WRITE_THROUGH,
        frame_allocator,
      )
      .expect("couldn't map the LAPIC page")
      .flush();
  }

  let lapic = LocalApic { addr: LAPIC_ADDR };

  serial_println!(
    "current lapic ({}) spurious: 0x{:x}",
    lapic.id(),
    lapic.spurious()
  );

  lapic
}

pub struct IOApic {
  id: u8,
  interrupt_base: u32,
  base: VirtAddr,
  irqs: [usize; 32],
}

impl IOApic {
  pub const IOAPICID: u32 = 0;
  pub const IOAPICVER: u32 = 1;
  pub const IOAPICARB: u32 = 2;

  pub unsafe fn new(
    entry: &acpi::sdt::madt::IoApicEntry,
    mapper: &mut MapperAllocator,
  ) -> Self {
    let addr = PhysAddr::new(entry.io_apic_address as u64);
    const APIC_MAPPING_SIZE: u64 = 2 * size_of::<u32>() as u64;

    let start_frame = PhysFrame::<Size4KiB>::containing_address(addr);
    let end_frame = PhysFrame::containing_address(addr + APIC_MAPPING_SIZE);
    let range = PhysFrame::range_inclusive(start_frame, end_frame);
    let n_pages = range.len();
    let start_addr = APIC_START
      + APIC_NEXT_OFFSET.fetch_add(n_pages * 0x1000, Ordering::Relaxed);
    for (i, frame) in range.into_iter().enumerate() {
      let page =
        Page::from_start_address(start_addr + i as u64 * 0x1000).unwrap();

      serial_println!("mapping page {:?}", page);
      unsafe {
        mapper
          .mapper
          .map_to(
            page,
            frame,
            PageTableFlags::PRESENT
              | PageTableFlags::NO_CACHE
              | PageTableFlags::NO_EXECUTE
              | PageTableFlags::WRITE_THROUGH
              | PageTableFlags::WRITABLE,
            &mut mapper.allocator,
          )
          .unwrap_or_else(|e| {
            panic!(
              "couldn't map the page for IOAPIC {}: {e:?}",
              entry.io_apic_id
            )
          })
          .flush();
      }
    }

    let vaddr =
      start_addr + (addr.as_u64() - start_frame.start_address().as_u64());

    let mut ioapic = Self {
      base: vaddr,
      id: entry.io_apic_id,
      interrupt_base: entry.global_system_interrupt_base,
      irqs: [0; 32],
    };

    // initialise irqs to direct-mapped
    for i in 0..32 {
      ioapic.irqs[i] = i;
    }

    ioapic
  }

  pub fn override_irq(&mut self, irq: usize, overridden: usize) {
    self.irqs[irq] = overridden;
  }

  fn read(&self, reg: u32) -> u32 {
    unsafe {
      self.base.as_mut_ptr::<u32>().write_volatile(reg & 0xff);
      self.base.as_ptr::<u32>().byte_add(0x10).read_volatile()
    }
  }

  fn write(&self, reg: u32, value: u32) {
    unsafe {
      self.base.as_mut_ptr::<u32>().write_volatile(reg & 0xff);
      self
        .base
        .as_mut_ptr::<u32>()
        .byte_add(0x10)
        .write_volatile(value);
    }
  }

  fn get_entry(&self, index: usize) -> RedirectionEntry {
    let bot_half = self.read(0x10 + index as u32 * 2) as u64;
    let top_half = self.read(0x10 + index as u32 * 2 + 1) as u64;
    let combined = (top_half << 32) | bot_half;
    RedirectionEntry(combined)
  }

  fn set_entry(&self, index: usize, entry: RedirectionEntry) {
    let bot_half = (entry.0 & 0xffff_ffff) as u32;
    let top_half = (entry.0 >> 32) as u32;
    self.write(0x10 + index as u32 * 2, bot_half);
    self.write(0x10 + index as u32 * 2 + 1, top_half);
  }

  pub fn enable_pit(&mut self) -> pit::PIT {
    let mut entry = self.get_entry(self.irqs[GLOBAL_PIT]);
    entry.set_mask(false);
    entry.set_vector(crate::idt::IRQ_PIT);
    self.set_entry(self.irqs[GLOBAL_PIT], entry);

    let mut pit = unsafe { pit::PIT::get() };
    pit.init();
    pit
  }

  pub fn enable_rtc(&mut self) -> rtc::RTC {
    let mut rtc = unsafe { rtc::RTC::get() };
    let mut entry = self.get_entry(self.irqs[GLOBAL_RTC]);
    entry.set_mask(false);
    entry.set_vector(crate::idt::IRQ_RTC);
    self.set_entry(self.irqs[GLOBAL_RTC], entry);
    unsafe { rtc.enable_int() };
    rtc
  }

  pub fn enable_hpet(&mut self, on_irq: usize) {
    let mut entry = self.get_entry(self.irqs[on_irq]);
    entry.set_mask(false);
    entry.set_vector(crate::idt::IRQ_HPET);
    self.set_entry(self.irqs[on_irq], entry);
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RedirectionEntry(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeliveryMode {
  Normal = 0,
  LowPriority = 1,
  SystemManagement = 2,
  NonMaskable = 4,
  Init = 5,
  External = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DestinationMode {
  Physical = 0,
  Logical = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Polarity {
  ActiveHigh = 0,
  ActiveLow = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TriggerMode {
  Edge = 0,
  Level = 1,
}

impl RedirectionEntry {
  pub fn vector(&self) -> u8 {
    (self.0 & 0xff) as u8
  }

  pub fn delivery_mode(&self) -> DeliveryMode {
    unsafe { core::mem::transmute(((self.0 >> 8) & 0x3) as u8) }
  }

  // else is physical
  pub fn destination_mode(&self) -> DestinationMode {
    unsafe { core::mem::transmute(((self.0 >> 11) & 0x1) as u8) }
  }

  pub fn is_busy(&self) -> bool {
    ((self.0 >> 12) & 0x1) != 0
  }

  pub fn polarity(&self) -> Polarity {
    unsafe { core::mem::transmute(((self.0 >> 13) & 0x1) as u8) }
  }

  // else lapic sent EOI
  pub fn is_received(&self) -> bool {
    ((self.0 >> 14) & 0x1) != 0
  }

  pub fn trigger_mode(&self) -> TriggerMode {
    unsafe { core::mem::transmute(((self.0 >> 15) & 0x1) as u8) }
  }

  pub fn is_masked(&self) -> bool {
    ((self.0 >> 16) & 0x1) != 0
  }

  // not pictured: destination field

  pub fn set_mask(&mut self, masked: bool) {
    self.0 = (self.0 & !0x10000) | ((masked as u64) << 16);
  }

  pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
    self.0 = (self.0 & !0x100) | ((mode as u8 as u64) << 8);
  }

  pub fn set_trigger_mode(&mut self, mode: TriggerMode) {
    self.0 = (self.0 & !0x8000) | ((mode as u8 as u64) << 15);
  }

  pub fn set_polarity(&mut self, polarity: Polarity) {
    self.0 = (self.0 & !0x2000) | ((polarity as u8 as u64) << 13);
  }

  pub fn set_vector(&mut self, vector: u8) {
    self.0 = (self.0 & !0xff) | vector as u64;
  }
}
