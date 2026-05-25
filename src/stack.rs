use core::mem::MaybeUninit;

use bitfield::{Bit, bitfield};
use x86_64::{PhysAddr, structures::paging::{Page, PageTableFlags, PhysFrame}};

use crate::MapperAllocator;

const STACKID_SCHEDULER: usize = 0;

const NUM_STACKS: usize = 8;

pub struct StackManager {
  stacks: [[PhysFrame; crate::vmem::PAGES_PER_STACK]; NUM_STACKS],
}

pub fn init(mapper: &mut MapperAllocator) -> StackManager {
  const NUM_PREALLOC_FRAMES: usize =
    crate::vmem::PAGES_PER_STACK * NUM_STACKS;

  const GUARD_FLAGS: PageTableFlags = PageTableFlags::GLOBAL;

  let frames: [PhysFrame; NUM_PREALLOC_FRAMES] = {
    let mut frames = [const { MaybeUninit::uninit() }; NUM_PREALLOC_FRAMES];

    for (i, frame) in mapper
      .allocator
      .allocate_many_frames(NUM_PREALLOC_FRAMES)
      .enumerate()
    {
      unsafe { frames[i].write(frame) };
    }

    unsafe { core::mem::transmute(frames) }
  };

  unsafe {
    mapper.map_to(
      Page::from_start_address_unchecked(crate::vmem::STACK_TOP_GUARD),
      PhysFrame::containing_address(PhysAddr::zero()),
      GUARD_FLAGS,
    );
    mapper.map_to(
      Page::from_start_address_unchecked(crate::vmem::STACK_BOT_GUARD),
      PhysFrame::containing_address(PhysAddr::zero()),
      GUARD_FLAGS,
    );
  }

  StackManager {
    stacks: unsafe { core::mem::transmute(frames) },
  }
}
