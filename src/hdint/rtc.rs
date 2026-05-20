use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::serial_print;

pub struct RTC {
  reg: PortWriteOnly<u8>,
  data: Port<u8>,
}

pub static TICKS_SINCE_BOOT: AtomicU64 = AtomicU64::new(0);

pub extern "C" fn rtc_interrupt() {
  TICKS_SINCE_BOOT.fetch_add(1, Ordering::Relaxed);

  // directly handle reading from register C so we can accept another timer interrupt
  unsafe {
    core::arch::asm! {
      "mov al, 0x0c",
      "out 0x70, al",
      "in ax, 0x71",
    };
  }
}

impl RTC {
  pub unsafe fn get() -> Self {
    Self {
      reg: PortWriteOnly::new(0x70),
      data: Port::new(0x71),
    }
  }

  pub unsafe fn enable_int(&mut self) {
    x86_64::instructions::interrupts::disable();
    unsafe {
      self.reg.write(0x8B);
      let data = self.data.read();
      self.reg.write(0x8B);
      self.data.write(data | 0x40);
    }
    x86_64::instructions::interrupts::enable();
  }

  pub fn set_rate(&mut self, rate: u8) {
    assert!(rate > 2 && rate < 16);
    x86_64::instructions::interrupts::disable();
    TICKS_SINCE_BOOT.store(0, Ordering::Relaxed);
    unsafe {
      self.reg.write(0x8A);
      let data = self.data.read();
      self.reg.write(0x8A);
      self.data.write((data & 0xf0) | (rate & 0x0f));
    }
    x86_64::instructions::interrupts::enable();
  }
}
