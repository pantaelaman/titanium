use super::*;

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct MADTFlags(u32);
  impl Debug;

  pub has_legacy_pics, _ : 0;
}

#[derive(Debug)]
#[repr(C)]
pub struct MADT {
  header: Header,
  lapic_addr: u32,
  flags: MADTFlags,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed(2))]
struct MADTEntryHeader {
  entry_type: u8,
  record_length: u8,
}

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct LocalApicFlags(u32);
  impl Debug;

  pub is_enabled, _ : 0;
  pub is_online_capable, _ : 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ApicIRQPolarity {
  NoOverride = 0b00,
  ActiveHi = 0b01,
  Reserved = 0b10,
  ActiveLo = 0b11,
}

crate::bitfield_cenum_bitrange!(enum ApicIRQPolarity(u16));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ApicIRQTrigger {
  NoOverride = 0b00,
  Edge = 0b01,
  Reserved = 0b10,
  Level = 0b11,
}

crate::bitfield_cenum_bitrange!(enum ApicIRQTrigger(u16));

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct ApicIRQFlags(u16);
  impl Debug;

  pub ApicIRQPolarity, polarity, _ : 1, 0;
  pub ApicIRQTrigger, trigger, _ : 3, 2;
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct LocalApicEntry {
  header: MADTEntryHeader,
  pub acpi_processor_id: u8,
  pub apic_id: u8,
  pub flags: LocalApicFlags,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct IOApicEntry {
  header: MADTEntryHeader,
  pub apic_id: u8,
  _reserved: u8,
  pub address: u32,
  pub gsi_base: u32,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct IOApicOverrideEntry {
  header: MADTEntryHeader,
  pub bus_source: u8,
  pub irq_source: u8,
  pub gsi: u32,
  pub flags: ApicIRQFlags,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct IOApicNonmaskableEntry {
  header: MADTEntryHeader,
  pub nmi_source: u8,
  _reserved: u8,
  pub flags: ApicIRQFlags,
  pub gsi: u32,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct LocalApicNonmaskableEntry {
  header: MADTEntryHeader,
  acpi_processor_id: u8,
  flags: ApicIRQFlags,
  lint: u8,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct LocalApicOverrideEntry {
  header: MADTEntryHeader,
  _reserved: u16,
  pub addr: PhysAddr,
}

#[derive(Debug)]
#[repr(C, packed(2))]
pub struct Localx2ApicEntry {
  header: MADTEntryHeader,
  pub x2apic_id: u32,
  pub flags: LocalApicFlags,
  pub acpi_id: u32,
}

#[derive(Debug)]
pub enum MADTEntry<'a> {
  LocalApic(&'a LocalApicEntry),
  IOApic(&'a IOApicEntry),
  IOApicOverride(&'a IOApicOverrideEntry),
  IOApicNonmaskable(&'a IOApicNonmaskableEntry),
  LocalApicNonmaskable(&'a LocalApicNonmaskableEntry),
  LocalApicOverride(&'a LocalApicOverrideEntry),
  Localx2Apic(&'a Localx2ApicEntry),
}

impl MADT {
  pub fn entries<'madt>(&'madt self) -> impl Iterator<Item = MADTEntry<'madt>> {
    core::iter::from_coroutine(
      #[coroutine]
      || unsafe {
        let mut ptr = (self as *const MADT).add(1).cast::<MADTEntryHeader>();
        let eptr = (self as *const MADT)
          .byte_add(self.header.length as usize)
          .cast();
        while ptr < eptr {
          let hdr = &*ptr;
          // pointer past the end of the entry's header
          yield match hdr.entry_type {
            0 => MADTEntry::LocalApic(&*ptr.cast::<LocalApicEntry>()),
            1 => MADTEntry::IOApic(&*ptr.cast::<IOApicEntry>()),
            2 => MADTEntry::IOApicOverride(&*ptr.cast::<IOApicOverrideEntry>()),
            3 => MADTEntry::IOApicNonmaskable(
              &*ptr.cast::<IOApicNonmaskableEntry>(),
            ),
            4 => MADTEntry::LocalApicNonmaskable(
              &*ptr.cast::<LocalApicNonmaskableEntry>(),
            ),
            5 => MADTEntry::LocalApicOverride(
              &*ptr.cast::<LocalApicOverrideEntry>(),
            ),
            9 => MADTEntry::Localx2Apic(&*ptr.cast::<Localx2ApicEntry>()),
            _ => unimplemented!(
              "unimplemented processor type of {}",
              hdr.entry_type
            ),
          };
          ptr = ptr.byte_add(hdr.record_length as usize);
        }
      },
    )
  }
}

pub struct MADTHandle {
  pub table: uacpi::uacpi_table,
}

impl MADTHandle {
  pub fn as_ref(&self) -> &MADT {
    let addr =
      VirtAddr::new(unsafe { self.table.__bindgen_anon_1.virt_addr } as u64);
    unsafe { &*addr.as_ptr::<MADT>() }
  }
}

impl core::ops::Drop for MADTHandle {
  fn drop(&mut self) {
    unsafe {
      uacpi::uacpi_table_unref(&mut self.table as _);
    }
  }
}
