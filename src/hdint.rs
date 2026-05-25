use core::sync::atomic::{
  AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering,
};

use crate::{
  MapperAllocator, idt, serial_print,
  vmem::{self, APIC_NEXT_OFFSET, APIC_START, LAPIC_ADDR},
};
use bitfield::bitfield;
use fixed::{FixedU64, types::extra::U16};
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

pub mod hpet;
pub mod pit;
pub mod rtc;

pub const GLOBAL_PIT: usize = 0;
pub const GLOBAL_RTC: usize = 8;
pub const GLOBAL_KBD: usize = 1;

static TICKS_PER_MS: AtomicU32 = AtomicU32::new(0);

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

  pub const TIMER_MODE_ONESHOT: u32 = 0b00 << 17;
  pub const TIMER_MODE_PERIODIC: u32 = 0b01 << 17;
  pub const TIMER_MODE_DEADLINE: u32 = 0b10 << 17;

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

  pub unsafe fn setup_timer_pit(&self, pit: &mut pit::PIT) {
    // divide by 8; see Intel Vol 3A 10.5.4
    self.write(Self::TMRDIV_OFFSET, 0x2);
    self.write(Self::TMRINITCNT_OFFSET, 0xffff_ffff);
    pit.sleep_us(1_000);
    self.write(Self::LVT_TIMER_OFFSET, Self::LVT_INT_MASKED);
    let ticks_per_ms = 0xffff_ffff - self.read(Self::TMRCURRCNT_OFFSET);
    TICKS_PER_MS.store(ticks_per_ms, Ordering::Relaxed);
  }
}

bitfield! {
  #[repr(transparent)]
  struct TimerFlags(u8);
  impl Debug;

  pub is_timing, set_timing: 0;
}

static IS_SLEEPING: AtomicBool = AtomicBool::new(false);

// actually of type TimerFlags
static TIMER_FLAGS: AtomicU8 = AtomicU8::new(0);

pub unsafe fn sleep_local_us(us: u32) {
  assert!(
    IS_SLEEPING
      .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
      .is_ok()
  );
  let mut lapic = unsafe { LocalApic::get() };
  lapic.write(LocalApic::LVT_TIMER_OFFSET, LocalApic::LVT_INT_MASKED);

  IS_SLEEPING.store(true, Ordering::Release);
  TIMER_FLAGS.fetch_or(1, Ordering::Relaxed);

  let ticks_per_ms = TICKS_PER_MS.load(Ordering::Relaxed);
  // ensure the timer is set up properly
  assert!(ticks_per_ms > 0);
  // TODO: handle multi-sleeps, i.e. sleeps of longer than 2^32 - 1 ticks
  lapic.write(
    LocalApic::TMRINITCNT_OFFSET,
    (ticks_per_ms / 1000) * us as u32,
  );
  lapic.write(
    LocalApic::LVT_TIMER_OFFSET,
    idt::IRQ_LAPIC_CLK as u32 | LocalApic::TIMER_MODE_ONESHOT,
  );

  while TIMER_FLAGS.load(Ordering::Acquire) & 0x1 != 0 {
    x86_64::instructions::hlt();
  }

  IS_SLEEPING.store(false, Ordering::Release);
}

/// Eventually, this interrupt will trigger *something* to happen
/// in the scheduler perhaps?
#[unsafe(naked)]
pub extern "C" fn lapic_clk_interrupt() -> ! {
  unsafe {
    core::arch::naked_asm! {
      "push rax",
      "mov al, {flags}",
      "and al, {flags_done}",
      "mov {flags}, al",
      "mov rax, 0",
      "movabs [{eoi}], rax",
      "pop rax",
      "iretq",
      eoi = const vmem::LAPIC_EOI_ADDR.as_u64(),
      flags_done = const !1u8,
      flags = sym TIMER_FLAGS,
    };
  }
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
    entry: &crate::acpi::madt::IOApicEntry,
    mapper: &mut MapperAllocator,
  ) -> Self {
    let addr = PhysAddr::new(entry.address as u64);
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
            mapper.allocator,
          )
          .unwrap_or_else(|e| {
            panic!(
              "couldn't map the page for IOAPIC {}: {e:?}",
              entry.apic_id
            )
          })
          .flush();
      }
    }

    let vaddr =
      start_addr + (addr.as_u64() - start_frame.start_address().as_u64());

    let mut ioapic = Self {
      base: vaddr,
      id: entry.apic_id,
      interrupt_base: entry.gsi_base,
      irqs: [0; 32],
    };

    // initialise irqs to direct-mapped
    for i in 0..32 {
      ioapic.irqs[i] = i;
    }

    ioapic
  }

  pub fn register_override(&mut self, ovr: &crate::acpi::madt::IOApicOverrideEntry) {
    use crate::acpi::madt::{ApicIRQPolarity, ApicIRQTrigger};

    self.irqs[ovr.irq_source as usize] = ovr.gsi as usize;
    let mut redentry = self.get_entry(ovr.gsi as usize);
    match ovr.flags.polarity() {
      ApicIRQPolarity::NoOverride => {},
      ApicIRQPolarity::ActiveLo => redentry.set_active_low(true),
      ApicIRQPolarity::ActiveHi => redentry.set_active_low(false),
      ApicIRQPolarity::Reserved => unreachable!()
    }
    match ovr.flags.trigger() {
      ApicIRQTrigger::NoOverride => {},
      ApicIRQTrigger::Level => redentry.set_level_triggered(true),
      ApicIRQTrigger::Edge => redentry.set_level_triggered(false),
      ApicIRQTrigger::Reserved => unreachable!()
    }
    self.set_entry(ovr.gsi as usize, redentry);
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

  pub fn init_pit(&mut self) -> pit::PIT {
    self.enable_pit();

    let mut pit = unsafe { pit::PIT::get() };
    pit.init();
    pit
  }

  pub fn enable_pit(&mut self) {
    let mut entry = self.get_entry(self.irqs[GLOBAL_PIT]);
    serial_println!("pit entry ({}) on {}", GLOBAL_PIT, self.irqs[GLOBAL_PIT]);
    entry.set_mask(false);
    entry.set_vector(crate::idt::IRQ_PIT);
    self.set_entry(self.irqs[GLOBAL_PIT], entry);
  }

  pub fn disable_pit(&mut self) {
    let mut entry = self.get_entry(self.irqs[GLOBAL_PIT]);
    entry.set_mask(false);
    self.set_entry(self.irqs[GLOBAL_PIT], entry);
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

  pub fn init_hpet(&mut self, table: &crate::acpi::hpet::HPET, mapper: &mut MapperAllocator) -> hpet::HPET {
    let mut hpet = hpet::HPET::new(table, mapper);
    serial_println!("HPET Table: {:?}", table);
    serial_println!("HPET: {:?}", hpet);

    hpet.disable();

    let timer = hpet.timer(0);
    let on_irq = timer.config.irq_capabilities().lowest_one().expect("HPET has no irq abilities") as usize;

    let mut entry = self.get_entry(self.irqs[on_irq]);
    entry.set_mask(true);
    entry.set_vector(crate::idt::IRQ_HPET);
    self.set_entry(self.irqs[on_irq], entry);

    hpet.disable_timer(0);

    hpet
  }
}

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

crate::bitfield_cenum_bitrange!(enum DeliveryMode(u64 => u8));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DestinationMode {
  Physical = 0,
  Logical = 1,
}

crate::bitfield_cenum_bitrange!(enum DestinationMode(u64 => u8));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Polarity {
  ActiveHigh = 0,
  ActiveLow = 1,
}

crate::bitfield_cenum_bitrange!(enum Polarity(u64 => u8));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TriggerMode {
  Edge = 0,
  Level = 1,
}

crate::bitfield_cenum_bitrange!(enum TriggerMode(u64 => u8));

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct RedirectionEntry(u64);
  impl Debug;

  pub u8, vector, set_vector : 7, 0;
  pub DeliveryMode, delivery_mode, set_delivery_mode : 10, 8;
  pub DestinationMode, destination_mode, set_destination_mode : 11;
  pub is_waiting, _ : 12;
  pub is_active_low, set_active_low: 13;
  pub is_accepted, _ : 14;
  pub is_level_triggered, set_level_triggered : 15;
  pub is_masked, set_mask : 16;
  pub u8, destination, set_destination : 63, 56;
}
