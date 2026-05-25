use super::*;

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct HPETFlags(u32);
  impl Debug;

  pub u16, pci_vendor_id, _ : 31, 16;
  pub can_use_legreg, _ : 15;
  pub u8, counter_size, _ : 13;
  pub u8, num_comparators, _ : 12, 8;
  pub u8, hardware_rev_id, _ : 7, 0;
}

#[repr(C, packed(1))]
pub struct HPET {
  header: Header,
  pub flags: HPETFlags,
  pub address: Address,
  pub hpet_number: u8,
  pub minimum_tick: u16,
  pub page_protection: u8,
}

impl core::fmt::Debug for HPET {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    use core::fmt::write;

    write!(
      f, "HPET {{ flags: {:?}, address: {:?}, hpet_number: {}, minimum_tick: {}, page_protection: {} }}",
      unsafe { core::ptr::addr_of!(self.flags).read_unaligned() },
      self.address,
      self.hpet_number,
      unsafe { core::ptr::addr_of!(self.minimum_tick).read_unaligned() },
      self.page_protection,
    )
  }
}

pub struct HPETHandle {
  pub(super) table: uacpi::uacpi_table,
}

impl HPETHandle {
  pub fn as_ref(&self) -> &HPET {
    let addr = VirtAddr::new(unsafe { self.table.__bindgen_anon_1.virt_addr } as u64);

    unsafe { &*addr.as_ptr() }
  }
}

impl core::ops::Drop for HPETHandle {
  fn drop(&mut self) {
    unsafe { uacpi::uacpi_table_unref(&mut self.table as *mut _) };
  }
}
