#![allow(static_mut_refs)]
use core::sync::atomic::AtomicU64;

use lazy_static::lazy_static;
use x86_64::{
  PhysAddr, VirtAddr,
  structures::paging::{PageSize, PageTableFlags, Size2MiB, Size4KiB},
};

pub const LAPIC_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0000_0000);
pub const LAPIC_EOI_ADDR: VirtAddr =
  VirtAddr::new(LAPIC_ADDR.as_u64() + crate::hdint::LocalApic::EOI_OFFSET);

/// Heap ranges from 0x_1000_0000_0000 to 0x_1000_0100_0000 (16 MiB)
/// For future-proofing, all addresses up to 0x_1001_0000_0000 are reserved
/// (4GiB total)
pub const HEAP_START: VirtAddr = VirtAddr::new_truncate(0x_1000_0000_0000);
pub const HEAP_SIZE: u64 = 0x100_0000;

// one page above and below the stack for protection are also included
pub const STACK_TOP_GUARD: VirtAddr = VirtAddr::new_truncate(0x_1800_0002_0000);
pub const STACK_TOP: VirtAddr = VirtAddr::new_truncate(0x_1800_0001_f000);
pub const STACK_BOT: VirtAddr = VirtAddr::new_truncate(0x_1800_0000_1000);
pub const STACK_BOT_GUARD: VirtAddr = VirtAddr::new_truncate(0x_1800_0000_0000);
pub const PAGES_PER_STACK: usize = 14;
pub const PAGE_SIZE: u64 = Size4KiB::SIZE;
pub const HUGE_PAGE_SIZE: u64 = Size2MiB::SIZE;

pub const APIC_START: VirtAddr = VirtAddr::new_truncate(0x_beef_0000_0000);
pub const APIC_SIZE: u64 = 0x_0001_0000_0000;
pub static APIC_NEXT_OFFSET: AtomicU64 = AtomicU64::new(0);

pub const HPET_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0100_0000);

/// Set if the page is a guard page, which should never be present or writable
pub const PAGE_FLAG_GUARD: PageTableFlags = PageTableFlags::BIT_9;
pub const PAGE_FLAG_STACK: PageTableFlags = PageTableFlags::BIT_10;
pub const PAGE_FLAG_HEAP: PageTableFlags = PageTableFlags::BIT_11;

pub const HHDM_TOP_OFFSET: u64 = crate::paging::MAX_PHYS_MAP;

pub static mut HHDM_ADDR_INNER: VirtAddr = VirtAddr::zero();
pub static mut HHDM_TOP_ADDR_INNER: VirtAddr = VirtAddr::zero();
pub static mut RSDP_ADDR_INNER: PhysAddr = PhysAddr::zero();

// super duper unsafe
pub const HHDM_ADDR: &'static VirtAddr = unsafe { &HHDM_ADDR_INNER };
pub const HHDM_TOP_ADDR: &'static VirtAddr = unsafe { &HHDM_TOP_ADDR_INNER };
pub const RSDP_ADDR: &'static PhysAddr = unsafe { &RSDP_ADDR_INNER };

/// This function must only ever be called once, at the very start of the kernel
/// It loads various addresses from the bootloader so they can be used by the kernel
/// after bootloader reclaimable memory is released
pub unsafe fn init() {
  let hhdm_offset = crate::limine::hhdm_offset();
  let rsdp_addr = crate::limine::rsdp_addr();
  unsafe {
    HHDM_ADDR_INNER = VirtAddr::new(hhdm_offset);
    HHDM_TOP_ADDR_INNER = HHDM_ADDR_INNER + HHDM_TOP_OFFSET;
    RSDP_ADDR_INNER = PhysAddr::new(rsdp_addr.as_u64() - hhdm_offset);
  }
}

// offset over HHDM_TOP_ADDR for the bottom of the scheduler's stack
pub const SCHEDULER_STACK_BOTTOM_OFFSET: u64 = 0x_0000_0001_0000;
pub const SCHEDULER_STACK_SIZE: u64 = 4; // 4 4KiB pages
pub const SCHEDULER_STACK_WIDTH: u64 = SCHEDULER_STACK_SIZE + 2; // + 2 guard pages
pub const SCHEDULER_STACK_TOP_OFFSET: u64 =
  SCHEDULER_STACK_BOTTOM_OFFSET + (SCHEDULER_STACK_WIDTH * PAGE_SIZE);

pub const KERNEL_STACK_SPACE_BOTTOM_OFFSET: u64 = 0x_0000_0010_0000;

pub const LARGE_KERNEL_STACK_PAGES: u64 = 5;
pub const SMALL_KERNEL_STACK_PAGES: u64 = 1;
pub const LARGE_KERNEL_STACK_SIZE: u64 = LARGE_KERNEL_STACK_PAGES * PAGE_SIZE;
pub const SMALL_KERNEL_STACK_SIZE: u64 = SMALL_KERNEL_STACK_PAGES * PAGE_SIZE;

pub const N_LARGE_KERNEL_STACKS: u64 = 16;
pub const N_SMALL_KERNEL_STACKS: u64 = N_LARGE_KERNEL_STACKS * 2;
pub const FIRST_LARGE_KERNEL_STACK_TOP_OFFSET: u64 =
  KERNEL_STACK_SPACE_BOTTOM_OFFSET + PAGE_SIZE + LARGE_KERNEL_STACK_SIZE;
pub const FIRST_SMALL_KERNEL_STACK_TOP_OFFSET: u64 =
  KERNEL_STACK_SPACE_BOTTOM_OFFSET
    + (PAGE_SIZE + LARGE_KERNEL_STACK_SIZE) * N_LARGE_KERNEL_STACKS
    + PAGE_SIZE
    + SMALL_KERNEL_STACK_SIZE;

// total number of pages required
// the `+ 1`s are for the guard pages between the stacks
// each guard page can be shared by both of its neighbouring stacks
pub const KERNEL_STACK_SPACE_PAGES: u64 = const {
  (N_LARGE_KERNEL_STACKS * (LARGE_KERNEL_STACK_PAGES + 1))
    + (N_SMALL_KERNEL_STACKS * (SMALL_KERNEL_STACK_PAGES + 1))
    + 1
};
pub const KERNEL_STACK_SPACE_SIZE: u64 = KERNEL_STACK_SPACE_PAGES * PAGE_SIZE;

pub const KERNEL_STACK_SPACE_TOP_OFFSET: u64 =
  KERNEL_STACK_SPACE_BOTTOM_OFFSET + KERNEL_STACK_SPACE_SIZE;

#[unsafe(no_mangle)]
pub static SCHEDULER_SP_OFFSET: u64 =
  SCHEDULER_STACK_BOTTOM_OFFSET + (SCHEDULER_STACK_SIZE + 1) * PAGE_SIZE;
