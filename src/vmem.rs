use core::sync::atomic::AtomicU64;

use x86_64::VirtAddr;

pub const LAPIC_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0000_0000);
const LAPIC_EOI_OFFSET: u64 = 0xb0;
pub const LAPIC_EOI_ADDR: VirtAddr = VirtAddr::new(LAPIC_ADDR.as_u64() + crate::hdint::LocalApic::EOI_OFFSET);

pub const HEAP_START: VirtAddr = VirtAddr::new_truncate(0x_4444_4444_0000);
pub const HEAP_SIZE: u64 = 100 * 1024; // 100 KiB

pub const APIC_START: VirtAddr = VirtAddr::new_truncate(0x_beef_0000_0000);
pub const APIC_SIZE: u64 = 0x_0001_0000_0000;
pub static APIC_NEXT_OFFSET: AtomicU64 = AtomicU64::new(0);

pub const HPET_ADDR: VirtAddr = VirtAddr::new_truncate(0x_1f10_0100_0000);
