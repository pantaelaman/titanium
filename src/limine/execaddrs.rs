use x86_64::{PhysAddr, VirtAddr};

use super::*;

#[repr(C)]
struct ExecAddrRequest {
  header: RequestHeader,
}

impl ExecAddrRequest {
  pub const fn new() -> Self {
    Self {
      header: RequestHeader {
        common: LIMINE_COMMON_MAGIC,
        subid: [0x71ba76863cc55f63, 0xb2644a48c516a487],
        revision: 0,
        response: core::ptr::null(),
      },
    }
  }
}

#[repr(C)]
struct ExecAddrResponse {
  header: ResponseHeader,
  phys_base: PhysAddr,
  virt_base: VirtAddr,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static EXEC_ADDR_REQUEST: ExecAddrRequest = ExecAddrRequest::new();

pub fn exec_phys_addr() -> PhysAddr {
  unsafe { &*EXEC_ADDR_REQUEST.header.response.cast::<ExecAddrResponse>() }.phys_base
}

pub fn exec_virt_addr() -> VirtAddr {
  unsafe { &*EXEC_ADDR_REQUEST.header.response.cast::<ExecAddrResponse>() }.virt_base
}
