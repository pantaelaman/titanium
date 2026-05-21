use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crossbeam::epoch::Atomic;
use fixed::{FixedU64, traits::Fixed, types::extra::U24};
use x86_64::instructions::port::{Port, PortGeneric, WriteOnlyAccess};

use crate::{serial_print, serial_println, vmem};

pub struct PIT {
  chans: [Port<u8>; 3],
  cmd: PortGeneric<u8, WriteOnlyAccess>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Channel {
  Chan0 = 0,
  Chan1 = 1,
  Chan2 = 2,
  ReadBack = 3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessMode {
  LatchCount = 0,
  LoByte = 1,
  HiByte = 2,
  LoHiByte = 3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OperatingMode {
  ITC = 0,
  Oneshot = 1,
  RateGen = 2,
  SquareGen = 3,
  SoftwareStrobe = 4,
  HardwareStrobe = 5,
}

#[repr(transparent)]
struct TimerCounter {
  counter: AtomicU64,
}

/// Counter, in fixed point 40.24 notation
/// measures in microseconds
static TIMER_COUNTER: TimerCounter = TimerCounter {
  // casts to Fixed
  counter: AtomicU64::new(0),
};

const FP_WIDTH: usize = 24;
const PIT_NATIVE_FREQ: u64 = 1193182;
// standard reload value for timer functionality
// --- IMPORTANT: QEMU NEEDS THIS TO BE 600 OR ELSE IT FUCKS IT ALL UP
const PIT_TIMER_RLD: u16 = 600;

// fixed point 40.24
// calculated manually from PIT_NATIVE_FREQ / PIT_TIMER_RLD
// 502ish us per interrupt
const PIT_INTERVAL: u64 = 0x1f6_db68b1;

#[unsafe(naked)]
pub extern "C" fn pit_interrupt() -> ! {
  unsafe {
    core::arch::naked_asm! {
      "push rax",
      "push rbx",
      "pushf",
      "mov rax, {timer}",
      "mov rbx, {incr}",
      "add rax, rbx",
      "mov {timer}, rax",
      "mov rax, 0",
      "movabs [{eoi}], rax",
      "popf",
      "pop rbx",
      "pop rax",
      "iretq",
      timer = sym TIMER_COUNTER,
      incr = const PIT_INTERVAL,
      eoi = const vmem::LAPIC_EOI_ADDR.as_u64(),
    }
  }
}

pub extern "C" fn debug_timer() {
  let timer = TIMER_COUNTER.counter.load(Ordering::Relaxed);
  serial_println!(
    "current pit counter: {} ({:x}.{:6x})",
    timer >> FP_WIDTH,
    timer >> FP_WIDTH,
    timer & ((1 << FP_WIDTH) - 1)
  );
}

impl PIT {
  pub unsafe fn get() -> Self {
    Self {
      chans: [Port::new(0x40), Port::new(0x41), Port::new(0x42)],
      cmd: PortGeneric::new(0x43),
    }
  }

  #[inline]
  pub fn send_command(
    &mut self,
    channel: Channel,
    access_mode: AccessMode,
    op_mode: OperatingMode,
    use_bcd: bool,
  ) {
    let command = (channel as u8) << 6
      | (access_mode as u8) << 4
      | (op_mode as u8) << 1
      | (use_bcd as u8);
    unsafe { self.cmd.write(command) };
  }

  #[inline]
  fn latch(&mut self, channel: Channel) {
    self.send_command(
      channel,
      AccessMode::LatchCount,
      OperatingMode::ITC,
      false,
    );
  }

  pub fn init(&mut self) {
    x86_64::instructions::interrupts::disable();
    self.send_command(
      Channel::Chan0,
      AccessMode::LoHiByte,
      OperatingMode::SquareGen,
      false,
    );
    unsafe {
      self.chans[0].write((PIT_TIMER_RLD & 0xff) as u8);
      self.chans[0].write((PIT_TIMER_RLD >> 8 & 0xff) as u8);
    }
    x86_64::instructions::interrupts::enable();
  }

  pub fn sleep_us(&self, us: u32) {
    let ctr_val = (us as u64) << FP_WIDTH;
    //serial_println!("sleeping until {}", us);
    TIMER_COUNTER.counter.store(0, Ordering::Release);
    while TIMER_COUNTER.counter.load(Ordering::Acquire) < ctr_val {
      x86_64::instructions::hlt();
    }
  }
}
