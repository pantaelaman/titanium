use core::mem::MaybeUninit;

use x86_64::{PrivilegeLevel, registers::segmentation::Segment, structures::{gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector}, tss::TaskStateSegment}};

static KERNEL_TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: MaybeUninit<GlobalDescriptorTable> = MaybeUninit::uninit();

const KERNEL_CS_INDEX: u16 = 1;
const KERNEL_DS_INDEX: u16 = 2;
const DATA_CS_INDEX: u16 = 3;
const DATA_DS_INDEX: u16 = 4;
const TSS_INDEX: u16 = 5;

pub unsafe fn init() {
  let gdt = unsafe {
    #[allow(static_mut_refs)]
    GDT.write(GlobalDescriptorTable::new())
  };

  gdt.append(Descriptor::kernel_code_segment());
  gdt.append(Descriptor::kernel_data_segment());
  gdt.append(Descriptor::user_code_segment());
  gdt.append(Descriptor::user_data_segment());
  gdt.append(Descriptor::tss_segment(&KERNEL_TSS));

  unsafe {
    x86_64::instructions::interrupts::disable();
    gdt.load();
    x86_64::registers::segmentation::CS::set_reg(SegmentSelector::new(KERNEL_CS_INDEX, PrivilegeLevel::Ring0));
    x86_64::registers::segmentation::DS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX, PrivilegeLevel::Ring0));
    x86_64::registers::segmentation::SS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX, PrivilegeLevel::Ring0));
    x86_64::registers::segmentation::GS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX, PrivilegeLevel::Ring0));
    x86_64::registers::segmentation::ES::set_reg(SegmentSelector::new(KERNEL_DS_INDEX, PrivilegeLevel::Ring0));
    x86_64::registers::segmentation::FS::set_reg(SegmentSelector::new(KERNEL_DS_INDEX, PrivilegeLevel::Ring0));
    x86_64::instructions::tables::load_tss(SegmentSelector::new(TSS_INDEX, PrivilegeLevel::Ring0));
    x86_64::instructions::interrupts::enable();
  }
}
