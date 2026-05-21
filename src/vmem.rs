use core::sync::atomic::AtomicU64;

use x86_64::VirtAddr;

pub const LAPIC_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0000_0000);
pub const LAPIC_EOI_ADDR: VirtAddr = VirtAddr::new(LAPIC_ADDR.as_u64() + crate::hdint::LocalApic::EOI_OFFSET);

/// Heap ranges from 0x_1000_0000_0000 to 0x_1000_0100_0000 (16 MiB)
/// For future-proofing, all addresses up to 0x_1001_0000_0000 are reserved
/// (4GiB total)
pub const HEAP_START: VirtAddr = VirtAddr::new_truncate(0x_1000_0000_0000);
pub const HEAP_SIZE: u64 = 0x100_0000;

pub const APIC_START: VirtAddr = VirtAddr::new_truncate(0x_beef_0000_0000);
pub const APIC_SIZE: u64 = 0x_0001_0000_0000;
pub static APIC_NEXT_OFFSET: AtomicU64 = AtomicU64::new(0);

pub const HPET_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0100_0000);
