use core::mem::MaybeUninit;

use bitfield::{Bit, BitMut, BitRange, BitRangeMut};
use bitvec::{BitArr, array::BitArray, bitarr, order::Lsb0, slice::BitSlice};
use const_for::const_for;
use x86_64::{
  PhysAddr, VirtAddr,
  registers::control::Cr3,
  structures::paging::{
    FrameAllocator, FrameDeallocator, OffsetPageTable, PageSize, PageTable,
    PhysFrame, Size1GiB, Size2MiB, Size4KiB,
    page_table::{FrameError, PageTableEntry},
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

// 128 GiB
const MAX_PHYS_MAP: u64 = 0x20_0000_0000;
const MAX_PHYS_MAP_PAGES: u64 = MAX_PHYS_MAP / crate::vmem::PAGE_SIZE;
/// Lowest level of the buddy allocator allocates in 4 page chunks
const MAX_BUDDY_BITS: usize = MAX_PHYS_MAP_PAGES as usize;
const BUDDY_LEVELS: usize = 16;
/// Holds index offsets to each of the starts of the levels
/// Also holds one past-the-end offset for convenience
const LEVEL_OFFSETS: [usize; BUDDY_LEVELS + 1] = const {
  let mut slice = [0; BUDDY_LEVELS + 1];

  const_for!(level in 1..BUDDY_LEVELS + 1 => {
    slice[level] = slice[level-1] + (MAX_BUDDY_BITS as usize >> level);
  });

  slice
};
/// Aliasing of LEVEL_OFFSETS[BUDDY_LEVELS], to give total length in bits
const TOTAL_BUDDY_BITS: usize = LEVEL_OFFSETS[BUDDY_LEVELS];
/// Appropriate level for 2MiB frames (for allocating huge frames)
const LEVEL_2MIB: usize = 9;

pub struct RuneFrameAllocator {
  buddy_bytes: BitArr!(for TOTAL_BUDDY_BITS),
  /*
  /// Marks the next usable chunk of pages in each level
  /// If, for any given level `l`, `next_in_level[l] == LEVEL_OFFSETS[l+1]`,
  /// there are no usable pages in that level
  next_in_level: [usize; BUDDY_LEVELS],
  */
}

// secretly hides that this is actually ~4MiB of memory
static mut FRAME_ALLOCATOR: RuneFrameAllocator =
  const { unsafe { core::mem::zeroed() } };

impl RuneFrameAllocator {
  /// Converts an absolute bit index to a pair of (index, level)
  /// The returned index is based off the level's offset
  fn absolute_to_relative(abs_index: usize) -> (usize, usize) {
    for level in 0..BUDDY_LEVELS {
      if abs_index < LEVEL_OFFSETS[level + 1] {
        return (abs_index - LEVEL_OFFSETS[level], level);
      }
    }

    unreachable!()
  }

  /// Converts an (index, level) pair to an absolute bit index
  #[inline]
  fn relative_to_absolute((index, level): (usize, usize)) -> usize {
    index + LEVEL_OFFSETS[level]
  }

  /// Converts an address into a *relative* index for the given level
  /// This will silently round down misaligned addresses
  #[inline]
  fn addr_to_index(addr: PhysAddr, level: usize) -> usize {
    (addr.as_u64() / crate::vmem::PAGE_SIZE) as usize >> level
  }

  /// Converts a level 0 index to the physical address of the start of its 4 page chunk
  #[inline]
  fn index_to_addr(index: usize) -> PhysAddr {
    PhysAddr::new(index as u64 * crate::vmem::PAGE_SIZE)
  }

  #[inline]
  fn relative_index_to_addr((index, level): (usize, usize)) -> PhysAddr {
    PhysAddr::new((index as u64 * crate::vmem::PAGE_SIZE) << level)
  }

  /// Converts a level 0 index into the physical frame it represents
  #[inline]
  fn index_to_frame(index: usize) -> PhysFrame {
    PhysFrame::containing_address(Self::index_to_addr(index))
  }

  pub fn new(memmap_entries: &[&MemmapEntry]) -> &'static mut Self {
    serial_println!(
      "{} bits required, {} bytes",
      TOTAL_BUDDY_BITS,
      TOTAL_BUDDY_BITS / 8
    );
    let allocator = unsafe {
      #[allow(static_mut_refs)]
      &mut FRAME_ALLOCATOR
    };

    'entries: for entry in memmap_entries
      .iter()
      .filter(|entry| entry.ty == MemmapType::Usable)
    {
      serial_println!("+++ seeing entry {:?}", entry);
      let physframe: PhysFrame<Size4KiB> =
        unsafe { PhysFrame::from_start_address_unchecked(entry.base) };
      let n_pages = entry.length / crate::vmem::PAGE_SIZE;

      let raw_frame_i =
        physframe.start_address().as_u64() / crate::vmem::PAGE_SIZE;
      let raw_frame_ei =
        (physframe + n_pages).start_address().as_u64() / crate::vmem::PAGE_SIZE;

      // go up and try to keep aligning
      let mut l_i = raw_frame_i as usize;
      let mut l_ei = raw_frame_ei as usize;
      for level in 0..BUDDY_LEVELS - 1 {
        let offset = LEVEL_OFFSETS[level];
        // check to see if there's any more chunks to store,
        // if not move on to the next entry
        if l_i == l_ei {
          serial_println!("+++ skipping entry on level {}", level);
          continue 'entries;
        }

        // if this isn't aligned to the next level, store this first page
        if l_i % 2 != 0 {
          allocator.buddy_bytes.set(offset + l_i, true);
          l_i += 1; // +1 to round upwards
        }
        l_i >>= 1;

        if l_ei % 2 != 0 {
          allocator.buddy_bytes.set(offset + l_ei - 1, true);
          // no need to subtract 1, shifting will take care of that
        }
        l_ei >>= 1;
      }

      // at this point, l_i, l_ei, and offset all refer to the topmost level
      // mark all of these chunks as usable
      serial_println!("++ marking top level regions {}..{} usable", l_i, l_ei);
      let offset = LEVEL_OFFSETS[BUDDY_LEVELS - 1];
      allocator.buddy_bytes[offset + l_i..offset + l_ei].fill(true);
    }

    allocator.debug();

    allocator
  }

  pub fn debug(&self) {
    for level in (0..BUDDY_LEVELS).rev() {
      serial_println!("+++ BUDDY LEVEL {} +++", level);
      for i in self.buddy_bytes[LEVEL_OFFSETS[level]..LEVEL_OFFSETS[level + 1]]
        .iter_ones()
      {
        serial_println!("{:?}", Self::relative_index_to_addr((i, level)));
      }
    }
  }

  fn allocate_frame_from_level(
    &mut self,
    min_level: usize,
  ) -> Option<PhysAddr> {
    let Some(abs_i) = self.buddy_bytes[LEVEL_OFFSETS[min_level]..].first_one()
    else {
      serial_println!(
        "CRITICAL ERROR: allocating a frame from level {} failed due to none available",
        min_level
      );
      return None;
    };
    let abs_i = abs_i + LEVEL_OFFSETS[min_level];

    let (bit_i, level) = Self::absolute_to_relative(abs_i);

    // this will end up being the absolute index of the chunk on level `min_level`
    let mut i = bit_i;
    for lower_level in (min_level..level).rev() {
      let lower_offset = LEVEL_OFFSETS[lower_level];
      let offset = LEVEL_OFFSETS[lower_level + 1];

      // split the chunk; mark this level as used
      self.buddy_bytes.set(i + offset, false);
      // widen the index by a level
      i <<= 1;
      // mark the other frame here as usable
      self.buddy_bytes.set(i + lower_offset + 1, true);
    }

    // mark this entry as used
    self.buddy_bytes.set(i + LEVEL_OFFSETS[min_level], false);

    let addr = Self::relative_index_to_addr((i, min_level));
    serial_println!("+++ allocated addr {:?} on level {}", addr, min_level);
    Some(addr)
  }

  fn merge_up_from(&mut self, (index, level): (usize, usize)) {
    let mut i = Self::relative_to_absolute((index, level));
    for level in level..BUDDY_LEVELS - 1 {
      // unset the first bit, so it gets rounded down to the start of the next even buddy pair
      i = i & !1;

      // if both members of the pair aren't usable, drop out now
      if !(self.buddy_bytes[i] && self.buddy_bytes[i+1]) {
        break;
      }

      // otherwise merge them up
      self.buddy_bytes.set(i, false);
      self.buddy_bytes.set(i+1, false);

      // find the next higher index
      let local_i = i - LEVEL_OFFSETS[level];
      i = (local_i >> 1) + LEVEL_OFFSETS[level + 1];

      self.buddy_bytes.set(i, true);
    }
  }
}

unsafe impl FrameAllocator<Size4KiB> for RuneFrameAllocator {
  #[inline]
  fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
    self
      .allocate_frame_from_level(0)
      .map(|addr| unsafe { PhysFrame::from_start_address_unchecked(addr) })
  }
}

unsafe impl FrameAllocator<Size2MiB> for RuneFrameAllocator {
  #[inline]
  fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
    self
      .allocate_frame_from_level(LEVEL_2MIB)
      .map(|addr| unsafe { PhysFrame::from_start_address_unchecked(addr) })
  }
}

impl FrameDeallocator<Size4KiB> for RuneFrameAllocator {
  #[inline]
  unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
    let index = Self::addr_to_index(frame.start_address(), 0);
    self.buddy_bytes.set(index, true);
    self.merge_up_from((index, 0));
  }
}

impl FrameDeallocator<Size2MiB> for RuneFrameAllocator {
  #[inline]
  unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size2MiB>) {
    let index = Self::addr_to_index(frame.start_address(), LEVEL_2MIB);
    self.buddy_bytes.set(index, true);
    self.merge_up_from((index, LEVEL_2MIB));
  }
}
