use core::{
  mem::MaybeUninit,
  ops::DerefMut,
  ptr::NonNull,
  sync::atomic::{Atomic, Ordering},
};

use bitfield::bitfield;
use spin::Mutex;
use x86_64::{
  PhysAddr, VirtAddr,
  instructions::port::Port,
  structures::paging::{
    Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
  },
};

use crate::{paging, serial_println, uacpi};

pub mod madt;

pub struct UACPIContext {
  _private: core::marker::PhantomData<()>,
}

#[repr(C)]
struct Header {
  signature: [u8; 4],
  length: u32,
  revision: u8,
  checksum: u8,
  oemid: [u8; 6],
  oem_table_id: [u8; 8],
  oem_revision: u32,
  creator_id: u32,
  creator_revision: u32,
}

impl core::fmt::Debug for Header {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    use core::fmt::write;

    write!(
      f,
      "Header {{ signature: \"{}\", length: {}, revision: {}, checksum: {}, oemid: \"{}\", oem_table_id: \"{}\", oem_revision: {}, creator_id: {}, creator_revision: {} }}",
      unsafe { str::from_utf8_unchecked(&self.signature) },
      self.length,
      self.revision,
      self.checksum,
      unsafe { str::from_utf8_unchecked(&self.oemid) },
      unsafe { str::from_utf8_unchecked(&self.oem_table_id) },
      self.oem_revision,
      self.creator_id,
      self.creator_revision,
    )
  }
}

impl UACPIContext {
  pub fn madt(&self) -> madt::MADTHandle {
    let mut table: MaybeUninit<uacpi::uacpi_table> = MaybeUninit::uninit();

    unsafe {
      uacpi::uacpi_table_find_by_signature(
        uacpi::ACPI_MADT_SIGNATURE.as_ptr().cast(),
        table.as_mut_ptr(),
      );
    }

    madt::MADTHandle {
      table: unsafe { table.assume_init() },
    }
  }
}

pub unsafe fn init_acpi() -> Option<UACPIContext> {
  let status = unsafe { uacpi::uacpi_initialize(0) };
  (status == uacpi::UACPI_STATUS_OK).then_some(UACPIContext {
    _private: core::marker::PhantomData,
  })
}
