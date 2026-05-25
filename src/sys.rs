pub fn sleep_local_us(us: u32) {
  unsafe {
    crate::hdint::sleep_local_us(us);
  }
}

pub fn get_ns() -> u64 {
  unsafe {
    crate::hdint::hpet::get_ns()
  }
}
