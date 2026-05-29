use bitfield::bitfield;
use x86_64::registers::model_specific::Msr;

use crate::serial_println;

#[unsafe(no_mangle)]
extern "C" fn fred_ring3() {
  unimplemented!("FRED calls from ring 3 are unsupported!");
}

#[unsafe(no_mangle)]
extern "C" fn fred_ring0() {
  unimplemented!("FRED calls from ring 0 are unsupported!");
}

unsafe extern "C" {
  #[link_name = "fred_entry_point"]
  #[link(name = "fred")]
  fn fred_entry_point();
}

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct FredConfig(u64);
  impl Debug;

  pub u8, csl, _ : 1, 0;
  pub will_decr_ssp, set_decr_ssp : 3;
  pub u8, n_lines_decr, _ : 8, 6;
  pub u8, sl_on_cpl0, set_sl_on_cpl0 : 10, 9;
  pub u64, handler_addr_upper, set_handler_addr_upper : 63, 12;
}

pub unsafe fn init() {
  let fred_config = Msr::new(0x1d4);
  unsafe {
    let curconfig = fred_config.read();
    serial_println!("FRED config: {:?}", curconfig);
    serial_println!("FRED entry: 0x{:08x}", fred_entry_point as *const () as u64);
  }
}
