pub fn sleep_local_us(us: u32) {
  unsafe {
    crate::hdint::sleep_local_us(us);
  }
}
