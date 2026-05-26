use x86_64::structures::paging::{
  Mapper, Page, PageTableFlags,
  mapper::{MapToError, MapperFlush},
  page::{NotGiantPageSize, PageRange},
};

use super::*;

/// Mapper-allocator pair. Lifetime will probably be `'static`.
/// Provides functionality for mapping memory generically and specifically,
/// see `map_to` (a wrapper around `mapper.map_to`) and `page_region`, which
/// will dynamically select frames and map them to the requested page range,
/// providing no guarantees about the physical layout of the memory.
pub struct MapperAllocator<'a> {
  pub mapper: OffsetPageTable<'a>,
  pub allocator: &'a mut RuneFrameAllocator,
}

impl<'a> MapperAllocator<'a> {
  #[inline]
  pub unsafe fn map_to(
    &mut self,
    page: Page,
    phys_frame: PhysFrame,
    flags: PageTableFlags,
  ) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>> {
    unsafe { self.mapper.map_to(page, phys_frame, flags, self.allocator) }
  }

  /// Ensure a certain region of memory is mapped with `flags`
  /// This is more optimised than calling [`allocate_frame`] and [`map_to`] several times
  /// This function DOES NOT flush the TLB, so you will have to do that yourself!
  /// If this function errors, the virtual memory may be partially mapped
  #[track_caller]
  pub fn page_region<S: PageSize + NotGiantPageSize>(
    &mut self,
    start: VirtAddr,
    end: VirtAddr,
    flags: PageTableFlags,
  ) -> Result<(), ()>
  where
    OffsetPageTable<'a>: Mapper<S>,
  {
    let start = start.align_down(S::SIZE);
    let end = end.align_up(S::SIZE);

    // how many frames of the requested size are there
    let n_frames = (end - start) / S::SIZE;

    // what's the minimum number of contiguous minimal frames we can request at a time
    // to be able to legally fulfill functionality
    let minimum_unit = (S::SIZE / crate::vmem::PAGE_SIZE) as usize;

    // how many frames of the minimum size are there
    // we need this since `self.allocator.get_contiguous` requires a number in *minimal* size
    let n_underlying_frames = (end - start) / crate::vmem::PAGE_SIZE;

    let mut remaining_frames = n_underlying_frames as usize;
    let mut current_page =
      unsafe { Page::<S>::from_start_address_unchecked(start) };
    while let Some((addr, n_minimal_mapped)) = self
      .allocator
      .pick_off_contiguous(remaining_frames, minimum_unit)
    {
      let n_mapped = n_minimal_mapped / minimum_unit;
      remaining_frames -= n_minimal_mapped;

      let first_mapped_frame =
        unsafe { PhysFrame::<S>::from_start_address_unchecked(addr) };
      for i in 0..n_mapped {
        unsafe {
          self.mapper.map_to(
            current_page,
            first_mapped_frame + i as u64,
            flags,
            self.allocator,
          );
        }
        current_page += 1;
      }

      if remaining_frames == 0 {
        return Ok(());
      }
    }

    // remaining_frames != 0
    Err(())
  }
}
