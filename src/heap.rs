use core::ptr::NonNull;

use alloc::alloc::{GlobalAlloc, Layout};

use crate::{
  MapperAllocator, serial_println,
  vmem::{HEAP_SIZE, HEAP_START},
};
use x86_64::{
  VirtAddr,
  structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size2MiB,
    Size4KiB, mapper::MapToError,
  },
};

pub mod ll;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> =
  Locked::new(FixedSizeBlockAllocator::new());

pub unsafe fn init(mapper: &mut MapperAllocator) {
  mapper.page_region::<Size2MiB>(
    HEAP_START,
    HEAP_START + HEAP_SIZE,
    PageTableFlags::PRESENT
      | PageTableFlags::WRITABLE
      | crate::vmem::PAGE_FLAG_HEAP,
  ).expect("could not map heap, not enough memory");

  x86_64::instructions::tlb::flush_all();

  unsafe { ALLOCATOR.inner.lock().init(HEAP_START, HEAP_SIZE) };
}

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

struct ListNode {
  next: Option<&'static mut ListNode>,
}

struct FixedSizeBlockAllocator {
  list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
  fallback_alloc: linked_list_allocator::Heap,
}

struct Locked<T> {
  inner: spin::Mutex<T>,
}

impl<T> Locked<T> {
  pub const fn new(inner: T) -> Self {
    Self {
      inner: spin::Mutex::new(inner),
    }
  }
}

impl FixedSizeBlockAllocator {
  pub const fn new() -> Self {
    const EMPTY: Option<&'static mut ListNode> = None;
    FixedSizeBlockAllocator {
      list_heads: [EMPTY; BLOCK_SIZES.len()],
      fallback_alloc: linked_list_allocator::Heap::empty(),
    }
  }

  pub unsafe fn init(&mut self, heap_start: VirtAddr, heap_size: u64) {
    unsafe {
      self
        .fallback_alloc
        .init(heap_start.as_mut_ptr(), heap_size as usize)
    };
  }

  fn list_index(layout: &Layout) -> Option<usize> {
    let req_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= req_block_size)
  }

  fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
    match self.fallback_alloc.allocate_first_fit(layout) {
      Ok(ptr) => ptr.as_ptr(),
      Err(_) => core::ptr::null_mut(),
    }
  }
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let mut allocator = self.inner.lock();
    match FixedSizeBlockAllocator::list_index(&layout) {
      Some(index) => match allocator.list_heads[index].take() {
        Some(node) => {
          allocator.list_heads[index] = node.next.take();
          node as *mut ListNode as *mut u8
        }
        None => {
          let block_size = BLOCK_SIZES[index];
          let block_align = block_size;
          let layout =
            Layout::from_size_align(block_size, block_align).unwrap();
          allocator.fallback_alloc(layout)
        }
      },
      None => allocator.fallback_alloc(layout),
    }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.inner.lock();
    match FixedSizeBlockAllocator::list_index(&layout) {
      Some(index) => {
        let new_node = ListNode {
          next: allocator.list_heads[index].take(),
        };

        assert!(core::mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
        assert!(core::mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);

        let new_node_ptr = ptr.cast::<ListNode>();
        unsafe {
          new_node_ptr.write(new_node);
          allocator.list_heads[index] = Some(&mut *new_node_ptr);
        }
      }
      None => {
        let ptr = NonNull::new(ptr).unwrap();
        unsafe {
          allocator.fallback_alloc.deallocate(ptr, layout);
        }
      }
    }
  }
}
