use x86_64::{VirtAddr, align_up};

struct ListNode {
  size: u64,
  next: Option<&'static mut ListNode>,
}

impl ListNode {
  const fn new(size: u64) -> Self {
    Self { size, next: None }
  }

  fn start_addr(&self) -> VirtAddr {
    VirtAddr::from_ptr(self as *const _)
  }

  fn end_addr(&self) -> VirtAddr {
    self.start_addr() + self.size
  }
}

pub struct LinkedListAllocator {
  head: ListNode,
}

impl LinkedListAllocator {
  pub const fn new() -> Self {
    Self {
      head: ListNode::new(0)
    }
  }

  pub unsafe fn init(&mut self, heap_start: VirtAddr, heap_size: u64) {
    unsafe {
      self.add_free_region(heap_start, heap_size)
    }
  }

  unsafe fn add_free_region(&mut self, addr: VirtAddr, size: u64) {
    assert_eq!(align_up(addr.as_u64(), core::mem::align_of::<ListNode>() as u64), addr.as_u64());
    assert!(size >= core::mem::size_of::<ListNode>() as u64);

    let mut node = ListNode::new(size);
    node.next = self.head.next.take();
    let node_ptr = addr.as_mut_ptr::<ListNode>();
    unsafe {
      node_ptr.write(node);
      self.head.next = Some(&mut *node_ptr);
    }
  }

  fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<VirtAddr, ()> {
    let alloc_start = align_up(region.start_addr().as_u64(), align as u64);
    let alloc_end = alloc_start.checked_add(size as u64).ok_or(())?;

    if alloc_end > region.end_addr().as_u64() {
      return Err(())
    }

    let excess = region.end_addr().as_u64() - alloc_end;
    if excess > 0 && excess < core::mem::size_of::<ListNode>() as u64 {
      return Err(())
    }

    Ok(VirtAddr::new(alloc_start))
  }

  fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, VirtAddr)> {
    let mut current = &mut self.head;
    while let Some(ref mut region) = current.next {
      if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
        let next = region.next.take();
        let ret = Some((current.next.take().unwrap(), alloc_start));
        current.next = next;
        return ret;
      } else {
        current = current.next.as_mut().unwrap();
      }
    }

    None
  }
}
