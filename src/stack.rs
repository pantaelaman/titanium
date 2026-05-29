use core::mem::MaybeUninit;

use bitfield::{Bit, bitfield};
use bitvec::{BitArr, bitarr, order::Lsb0};
use x86_64::{
  PhysAddr, VirtAddr,
  structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{MapperAllocator, serial_println, vmem};

/// Creates the stack for the scheduler at its designated location
pub unsafe fn make_scheduler_stack(mapper: &mut MapperAllocator) {
  let bottom = *vmem::HHDM_TOP_ADDR + vmem::SCHEDULER_STACK_BOTTOM_OFFSET;
  let top = *vmem::HHDM_TOP_ADDR + vmem::SCHEDULER_STACK_TOP_OFFSET;

  mapper
    .page_region::<Size4KiB>(
      bottom + vmem::PAGE_SIZE,
      top - vmem::PAGE_SIZE,
      PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | vmem::PAGE_FLAG_STACK,
    )
    .expect("couldn't map memory for the scheduler's stack");

  // map guard pages
  // this prevents UB on overflow/underflow (causes page fault instead)
  unsafe {
    mapper.mapper.map_to(
      Page::<Size4KiB>::from_start_address_unchecked(bottom),
      PhysFrame::from_start_address_unchecked(PhysAddr::zero()),
      vmem::PAGE_FLAG_GUARD,
      mapper.allocator,
    );
    mapper.mapper.map_to(
      Page::<Size4KiB>::from_start_address_unchecked(top) - 1,
      PhysFrame::from_start_address_unchecked(PhysAddr::zero()),
      vmem::PAGE_FLAG_GUARD,
      mapper.allocator,
    );
  }
}

struct StackManager {
  kernel_large_stacks: BitArr!(for vmem::N_LARGE_KERNEL_STACKS as usize, in u16),
  kernel_small_stacks: BitArr!(for vmem::N_SMALL_KERNEL_STACKS as usize, in u16),
}

impl StackManager {
  fn map_small_stack(&mut self) -> Result<VirtAddr, ()> {
    x86_64::instructions::interrupts::disable();
    let index = self.kernel_small_stacks.first_one().ok_or(())?;
    self.kernel_small_stacks.set(index, false);

    #[allow(static_mut_refs)]
    let mapper = unsafe { crate::scheduler::MAPALL.assume_init_mut() };

    let end = *vmem::HHDM_ADDR
      + vmem::FIRST_SMALL_KERNEL_STACK_TOP_OFFSET
      + (vmem::SMALL_KERNEL_STACK_SIZE + vmem::PAGE_SIZE) * index as u64;
    let start = end - vmem::SMALL_KERNEL_STACK_SIZE;

    serial_println!("mapping small stack from {:?} to {:?}", end, start);

    mapper.page_region::<Size4KiB>(
      start,
      end,
      PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | vmem::PAGE_FLAG_STACK,
    );

    x86_64::instructions::interrupts::enable();

    Ok(end)
  }

  fn map_large_stack(&mut self) -> Result<VirtAddr, ()> {
    x86_64::instructions::interrupts::disable();
    let index = self.kernel_large_stacks.first_one().ok_or(())?;
    self.kernel_large_stacks.set(index, false);

    #[allow(static_mut_refs)]
    let mapper = unsafe { crate::scheduler::MAPALL.assume_init_mut() };

    let end = *vmem::HHDM_ADDR
      + vmem::FIRST_LARGE_KERNEL_STACK_TOP_OFFSET
      + (vmem::LARGE_KERNEL_STACK_SIZE + vmem::PAGE_SIZE) * index as u64;
    let start = end - vmem::LARGE_KERNEL_STACK_SIZE;

    mapper.page_region::<Size4KiB>(
      start,
      end,
      PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | vmem::PAGE_FLAG_STACK,
    );

    x86_64::instructions::interrupts::enable();

    Ok(end)
  }
}

impl Default for StackManager {
  fn default() -> Self {
    Self {
      kernel_large_stacks: bitarr![u16, Lsb0; 1; vmem::N_LARGE_KERNEL_STACKS as usize],
      kernel_small_stacks: bitarr![u16, Lsb0; 1; vmem::N_SMALL_KERNEL_STACKS as usize],
    }
  }
}

static mut STACK_MANAGER: MaybeUninit<StackManager> = MaybeUninit::uninit();

pub fn init(mapper: &mut MapperAllocator) {
  let bottom_addr = *vmem::HHDM_ADDR + vmem::KERNEL_STACK_SPACE_BOTTOM_OFFSET;
  let mut curpage =
    unsafe { Page::<Size4KiB>::from_start_address_unchecked(bottom_addr) };
  let null_frame = unsafe {
    PhysFrame::<Size4KiB>::from_start_address_unchecked(PhysAddr::zero())
  };

  for _ in 0..vmem::N_LARGE_KERNEL_STACKS {
    unsafe {
      mapper.map_to(
        curpage,
        null_frame,
        vmem::PAGE_FLAG_GUARD | vmem::PAGE_FLAG_STACK,
      );
    }

    // increment by 1 large kernel stack + 1 guard page
    curpage += 1 + vmem::LARGE_KERNEL_STACK_PAGES;

    serial_println!(
      "%% LARGE kernel stack at 0x{:016x}",
      curpage.start_address()
    );
  }

  // range inclusive here so that we have a final top guard
  for _ in 0..=vmem::N_SMALL_KERNEL_STACKS {
    unsafe {
      mapper.map_to(
        curpage,
        null_frame,
        vmem::PAGE_FLAG_GUARD | vmem::PAGE_FLAG_STACK,
      );
    }

    curpage += 1 + vmem::SMALL_KERNEL_STACK_PAGES;

    serial_println!(
      "%% SMALL kernel stack at 0x{:016x}",
      curpage.start_address()
    );
  }

  #[allow(static_mut_refs)]
  unsafe {
    STACK_MANAGER.write(StackManager::default());
  }

  serial_println!("%% IGNORE LAST SMALL STACK");
}

/// Creates a kernel stack, suitable for kernel processes in the upper half
/// If `small_stack` is set, the stack is set to one page (4 KiB) of usable space
/// Otherwise, it will have four pages (16 KiB)
pub fn make_kernel_stack(small_stack: bool) -> Result<VirtAddr, ()> {
  #[allow(static_mut_refs)]
  let mut stack_manager = unsafe { STACK_MANAGER.assume_init_mut() };

  if small_stack {
    stack_manager.map_small_stack()
  } else {
    stack_manager.map_large_stack()
  }
}

/// Represents a user stack which is yet to be mapped
pub struct UserStackMap {
  _private: (),
}

impl UserStackMap {
  /// Flush the mapping, making it active to a userspace page
  pub fn flush(self, mapper: &mut MapperAllocator) {
    unimplemented!()
  }
}

/// Create a stack suitable for a user-space application
/// The resulting stack must be flushed into existence with the [`UserStackMap::flush`]
/// function.
pub fn make_user_stack() -> UserStackMap {
  unimplemented!()
}
