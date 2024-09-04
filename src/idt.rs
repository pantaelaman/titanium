use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{
  gdt,
  sync::LazyLock,
  vga_buffer::{Colour, ColourCode, WRITER},
};

static IDT: LazyLock<InterruptDescriptorTable> = LazyLock::new(&|| {
  let mut idt = InterruptDescriptorTable::new();
  idt.breakpoint.set_handler_fn(breakpoint_handler);
  unsafe {
    idt
      .double_fault
      .set_handler_fn(double_fault_handler)
      .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
  }
  idt
});

pub fn init() {
  IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
  WRITER
    .lock()
    .set_temp_colour_code(Colour::LightGreen.into());
  crate::println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
  WRITER.lock().release_temp_colour_code();
}

extern "x86-interrupt" fn double_fault_handler(
  stack_frame: InterruptStackFrame,
  _error_code: u64,
) -> ! {
  panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
