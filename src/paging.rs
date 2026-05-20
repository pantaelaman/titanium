use x86_64::{
  PhysAddr, VirtAddr,
  registers::control::Cr3,
  structures::paging::{
    FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB, page_table::{FrameError, PageTableEntry}
  },
};

use crate::{
  limine::{MemmapEntry, MemmapType},
  serial_println,
};

pub unsafe fn active_l4_page_table(
  phys_offset: VirtAddr,
) -> &'static mut PageTable {
  let (l4_table_frame, _) = Cr3::read();

  let phys = l4_table_frame.start_address();
  let virt = phys_offset + phys.as_u64();
  let ptr = virt.as_mut_ptr::<PageTable>();

  serial_println!("Page table found at {:?} {:?}", virt, phys);

  unsafe { &mut *ptr }
}

pub unsafe fn init(phys_offset: VirtAddr) -> OffsetPageTable<'static> {
  unsafe {
    let l4_page_table = active_l4_page_table(phys_offset);
    OffsetPageTable::new(l4_page_table, phys_offset)
  }
}

pub struct RuneFrameAllocator<'m> {
  memmap_entries: &'m [&'m MemmapEntry],
  next: usize,
}

impl<'m> RuneFrameAllocator<'m> {
  pub fn new(memmap_entries: &'m [&'m MemmapEntry]) -> Self {
    Self {
      memmap_entries,
      next: 0,
    }
  }

  fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
    self
      .memmap_entries
      .iter()
      .filter(|entry| entry.ty == MemmapType::Usable)
      .flat_map(|entry| {
        (entry.base.as_u64()..entry.base.as_u64() + entry.length).step_by(4096)
      })
      .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
  }
}

unsafe impl<'m> FrameAllocator<Size4KiB> for RuneFrameAllocator<'m> {
  fn allocate_frame(&mut self) -> Option<PhysFrame> {
    let frame = self.usable_frames().nth(self.next)?;
    self.next += 1;
    Some(frame)
  }
}
