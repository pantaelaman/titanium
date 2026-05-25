use x86_64::{PhysAddr, VirtAddr};

use super::*;

#[repr(C)]
struct MemmapRequest {
  header: RequestHeader,
}

impl MemmapRequest {
  pub const fn new() -> Self {
    Self {
      header: RequestHeader {
        common: LIMINE_COMMON_MAGIC,
        subid: [0x67cf3d9d378a806f, 0xe304acdfc50c3c62],
        revision: 0,
        response: core::ptr::null(),
      },
    }
  }
}

#[repr(C)]
struct MemmapResponse {
  header: ResponseHeader,
  entry_count: u64,
  entries: *const *const MemmapEntry,
}

impl MemmapResponse {
  pub fn entries(&self) -> &[&MemmapEntry] {
    unsafe {
      slice::from_raw_parts(
        self.entries as _,
        self.entry_count as usize,
      )
    }
  }
}

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum MemmapType {
  Usable = 0,
  Reserved = 1,
  AcpiReclaimable = 2,
  AcpiNvs = 3,
  BadMemory = 4,
  BootloaderReclaimable = 5,
  ExecutableAndModules = 6,
  Framebuffer = 7,
  ReservedMapped = 8,
}

#[derive(Debug)]
#[repr(C)]
pub struct MemmapEntry {
  pub base: PhysAddr,
  pub length: u64,
  pub ty: MemmapType,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMMAP_REQUEST: MemmapRequest = MemmapRequest::new();

pub fn memmap_entries() -> Option<&'static [&'static MemmapEntry]> {
  (!MEMMAP_REQUEST.header.response.is_null()).then(|| {
    unsafe { &*MEMMAP_REQUEST.header.response.cast::<MemmapResponse>() }.entries()
  })
}
