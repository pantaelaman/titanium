use x86_64::{
  instructions::tables::load_tss,
  registers::segmentation::{Segment, CS},
  structures::{
    gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
    tss::TaskStateSegment,
  },
  VirtAddr,
};

use crate::sync::LazyLock;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

static GDT: LazyLock<(GlobalDescriptorTable, Selectors)> =
  LazyLock::new(&|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
    let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
    (
      gdt,
      Selectors {
        code_selector,
        tss_selector,
      },
    )
  });

static TSS: LazyLock<TaskStateSegment> = LazyLock::new(&|| {
  let mut tss = TaskStateSegment::new();
  tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
    const STACK_SIZE: usize = 4096 * 5;
    static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

    #[allow(unused_unsafe)]
    let stack_start = VirtAddr::from_ptr(unsafe { core::ptr::addr_of!(STACK) });
    let stack_end = stack_start + STACK_SIZE;
    stack_end
  };
  tss
});

struct Selectors {
  code_selector: SegmentSelector,
  tss_selector: SegmentSelector,
}

pub fn init() {
  GDT.0.load();
  unsafe {
    CS::set_reg(GDT.1.code_selector);
    load_tss(GDT.1.tss_selector);
  }
}
