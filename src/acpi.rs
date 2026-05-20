use core::{
  mem::MaybeUninit,
  ops::DerefMut,
  ptr::NonNull,
  sync::atomic::{AtomicU64, Ordering},
};

use acpi::{Handler, PhysicalMapping};
use alloc::sync::Arc;
use spin::Mutex;
use x86_64::{
  PhysAddr, VirtAddr,
  instructions::port::Port,
  structures::paging::{
    Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
  },
};

use crate::{
  paging, serial_println,
};

pub struct ACPIHandler {}

impl<'a> Handler for &'a ACPIHandler {
  unsafe fn map_physical_region<T>(
    &self,
    physical_address: usize,
    size: usize,
  ) -> acpi::PhysicalMapping<Self, T> {
    // physical regions are already mapped in the higher half (for now)
    PhysicalMapping {
      physical_start: physical_address,
      virtual_start: unsafe { NonNull::new_unchecked((physical_address + crate::limine::hhdm_offset() as usize) as *mut T) },
      region_length: size,
      mapped_length: size,
      handler: &self,
    }
  }

  // do nothing, everything is just hhid map anyways
  fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {}

  fn read_u8(&self, address: usize) -> u8 {
    unsafe { core::ptr::read_volatile(address as *const u8) }
  }
  fn read_u16(&self, address: usize) -> u16 {
    unsafe { core::ptr::read_volatile(address as *const u16) }
  }
  fn read_u32(&self, address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
  }
  fn read_u64(&self, address: usize) -> u64 {
    unsafe { core::ptr::read_volatile(address as *const u64) }
  }

  fn write_u8(&self, address: usize, value: u8) {
    unsafe { core::ptr::write_volatile(address as *mut u8, value) }
  }
  fn write_u16(&self, address: usize, value: u16) {
    unsafe { core::ptr::write_volatile(address as *mut u16, value) }
  }
  fn write_u32(&self, address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
  }
  fn write_u64(&self, address: usize, value: u64) {
    unsafe { core::ptr::write_volatile(address as *mut u64, value) }
  }

  fn read_pci_u8(&self, address: acpi::PciAddress, offset: u16) -> u8 {
    unimplemented!()
  }
  fn read_pci_u16(&self, address: acpi::PciAddress, offset: u16) -> u16 {
    unimplemented!()
  }
  fn read_pci_u32(&self, address: acpi::PciAddress, offset: u16) -> u32 {
    unimplemented!()
  }

  fn write_pci_u8(&self, address: acpi::PciAddress, offset: u16, value: u8) {
    unimplemented!()
  }
  fn write_pci_u16(&self, address: acpi::PciAddress, offset: u16, value: u16) {
    unimplemented!()
  }
  fn write_pci_u32(&self, address: acpi::PciAddress, offset: u16, value: u32) {
    unimplemented!()
  }

  fn read_io_u8(&self, port: u16) -> u8 {
    unsafe { Port::new(port).read() }
  }
  fn read_io_u16(&self, port: u16) -> u16 {
    unsafe { Port::new(port).read() }
  }
  fn read_io_u32(&self, port: u16) -> u32 {
    unsafe { Port::new(port).read() }
  }

  fn write_io_u8(&self, port: u16, value: u8) {
    unsafe {
      Port::new(port).write(value);
    }
  }
  fn write_io_u16(&self, port: u16, value: u16) {
    unsafe {
      Port::new(port).write(value);
    }
  }
  fn write_io_u32(&self, port: u16, value: u32) {
    unsafe {
      Port::new(port).write(value);
    }
  }

  fn nanos_since_boot(&self) -> u64 {
    unimplemented!()
  }

  fn stall(&self, microseconds: u64) {
    unimplemented!()
  }

  fn sleep(&self, milliseconds: u64) {
    unimplemented!()
  }

  fn create_mutex(&self) -> acpi::Handle {
    unimplemented!()
  }
  fn acquire(
    &self,
    mutex: acpi::Handle,
    timeout: u16,
  ) -> Result<(), acpi::aml::AmlError> {
    unimplemented!()
  }
  fn release(&self, mutex: acpi::Handle) {
    unimplemented!()
  }
}

static mut PCI_ADDR: Port<u32> = Port::new(0xcf8);
static mut PCI_DATA: Port<u32> = Port::new(0xcfc);

static mut ACPI_HANDLER: MaybeUninit<ACPIHandler> = MaybeUninit::uninit();

pub unsafe fn init() -> acpi::AcpiTables<&'static ACPIHandler> {
  unsafe {
    #[allow(static_mut_refs)]
    ACPI_HANDLER.write(ACPIHandler {});
  };

  let rsdp_addr = PhysAddr::new(
    (crate::limine::rsdp_addr() - crate::limine::hhdm_offset()).as_u64(),
  );
  serial_println!("rsdp at {:?}", rsdp_addr);

  unsafe {
    acpi::AcpiTables::from_rsdp(
      #[allow(static_mut_refs)]
      ACPI_HANDLER.assume_init_ref(),
      rsdp_addr.as_u64() as usize,
    )
    .unwrap()
  }
}
