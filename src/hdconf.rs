use x86_64::VirtAddr;

pub unsafe fn init(phys_offset: VirtAddr) {
  let rsdp_addr = phys_offset + crate::limine::rsdp_addr().as_u64();
}
