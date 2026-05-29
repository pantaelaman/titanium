use core::{
  arch::naked_asm,
  mem::MaybeUninit,
  sync::atomic::{Atomic, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
  task::Waker,
};

use alloc::boxed::Box;
use bitfield::bitfield;
use bitflags::{Flags, bitflags};
use crossbeam::queue::SegQueue;
use x86_64::{
  PrivilegeLevel, VirtAddr, registers::rflags::RFlags,
  structures::gdt::SegmentSelector,
};

use crate::{MapperAllocator, hdint::LocalApic, serial_println, vmem};

mod waker;

pub unsafe fn init(mapper: MapperAllocator<'static>) {
  #[allow(static_mut_refs)]
  unsafe {
    let mapper = MAPALL.write(mapper);

    crate::stack::make_scheduler_stack(mapper);

    x86_64::instructions::interrupts::disable();
    SCHEDULER_TASK.write(TaskInfo {
      sp: *vmem::HHDM_TOP_ADDR + vmem::SCHEDULER_SP_OFFSET,
      task_id: TASKID_SCHEDULER,
      flags: TaskFlags::default(),
      wake_on_end: core::ptr::null(),
    });
    x86_64::instructions::interrupts::enable();
  }
}

#[unsafe(naked)]
pub unsafe extern "C" fn jump_to_scheduler() -> ! {
  // calculates the correct stack pointer for the scheduler,
  // then jumps to the correct function (which must never return)
  core::arch::naked_asm! {
    "cli",
    "mov rax, {hhdm_addr}",
    "add rax, {sp_offset}",
    "mov rsp, rax",
    "jmp {jump}",
    hhdm_addr = sym vmem::HHDM_TOP_ADDR_INNER,
    sp_offset = const vmem::SCHEDULER_SP_OFFSET,
    jump = sym start_scheduler,
  }
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(0x100);
const MAX_TASK_ID: u64 = 0xffff_ffff_fffe;

#[track_caller]
pub fn spawn_task(
  task_ptr: fn(),
  options: TaskConfig,
  priority: u8,
  wake_on_end: Option<*const TaskInfo>,
) -> *const TaskInfo {
  assert!(options.contains(TaskConfig::KERNEL_TASK));
  assert!(priority < 8);
  let task_addr = VirtAddr::from_ptr(task_ptr as *const ());

  let task_raw_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
  assert!(task_raw_id <= MAX_TASK_ID);
  let task_id =
    TaskId(options.known_bits() | task_raw_id | (priority as u64 & 0x7) << 48);

  let stack_ptr =
    crate::stack::make_kernel_stack(options.contains(TaskConfig::SMALL_STACK))
      .expect("couldn't make kernel stack");

  let task_info = Box::leak(Box::new(TaskInfo {
    sp: stack_ptr,
    task_id,
    flags: TaskFlags::default(),
    wake_on_end: wake_on_end.unwrap_or(core::ptr::null()),
  }));

  let final_sp: u64;
  unsafe {
    core::arch::asm! {
      "mov rdx, rsp",
      "mov rsp, rsi",

      // push the address of the end_task function
      // so if the task terminates, it calls it safely
      "lea rax, {end_fn}",
      "push rax",

      // interrupt stack frame
      "mov rax, rsp",
      "push {ss}",
      "push rax",
      "pushfq",
      "push {cs}",
      "push rdi",

      // standard stack frame
      "push 0", // rbx
      "push 0", // rbp
      "push 0", // r12
      "push 0", // r13
      "push 0", // r14
      "push 0", // r15

      "mov rax, rsp",
      "mov rsp, rdx", // restore the stack pointer

      cs = const 0x08,
      ss = const 0x10,
      end_fn = sym end_task,
      in("rdi") task_addr.as_u64(),
      in("rsi") stack_ptr.as_u64(),
      lateout("rax") final_sp,
      clobber_abi("C"),
    };
  }

  task_info.sp = VirtAddr::new(final_sp);

  // queue task to be run by the scheduler asap
  SCHEDULER_STATE
    .task_queue
    .push(task_info as *const _ as usize);

  task_info as *const _
}

#[unsafe(naked)]
unsafe extern "C" fn end_task() {
  core::arch::naked_asm! {
    "cli",
    "lea rax, {schedptr}",
    "mov rdi, {curtaskptr}",
    "mov {curtaskptr}, rax",
    "call {finish_fn}",
    finish_fn = sym finish_stale_task,
    schedptr = sym SCHEDULER_TASK,
    curtaskptr = sym CURRENT_TASK_PTR,
  };
}

extern "C" fn finish_stale_task() {
  let task_ptr = STALE_TASK_PTR.load(Ordering::Relaxed);
  panic!("finishing task 0x{:016x}", task_ptr as usize);

  todo!()
}

#[derive(Debug)]
#[repr(u8)]
enum ResumeKind {
  /// Indicates the task is blocking on some other task
  Blocked = 0b00,
  /// Indicates the task cooperatively yielded, but should be queued for continuation
  Yielded = 0b01,
  /// Indicates the task was preemptively interrupted
  Preempted = 0b10,
  /// Indicates the task has been seen to completion and should be removed
  Complete = 0b11,
}

crate::bitfield_cenum_bitrange!(enum ResumeKind(u64 => u8));

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  struct ResumedState(u64);
  impl Debug;

  pub ResumeKind, resume_kind, _ : 1, 0;
}

impl ResumedState {
  pub fn yielded() -> Self {
    Self(ResumeKind::Yielded as u64)
  }
}

bitfield! {
  #[derive(Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  struct TaskId(u64);
  impl Debug;

  pub u8, priority, set_priority : 50, 48;
  pub u64, task_id, set_task_id : 47, 0;

  pub TaskConfig, as_flags, set_flags : 63, 56;
}

bitflags! {
  #[derive(Debug)]
  pub struct TaskConfig: u64 {
    const KERNEL_TASK = 1 << 63;
    const USES_FLOATS = 1 << 62;
    const SMALL_STACK = 1 << 61;
  }
}

impl TaskConfig {
  pub fn small_kernel() -> Self {
    Self::KERNEL_TASK | Self::SMALL_STACK
  }

  pub fn kernel() -> Self {
    Self::KERNEL_TASK
  }
}

crate::bitfield_bitflags_bitrange!(struct TaskConfig(u64));

bitflags! {
  #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
  #[repr(transparent)]
  pub struct TaskFlags: u64 {
    /// If true, the 16 gp registers and EFLAGS are on the top of the stack
    const INTERRUPTED = 1 << 0;
  }
}

const TASKID_IDLE: TaskId = TaskId(0x8007_0000_0000_0000);
/// The default task id of the scheduler.
/// The scheduler will never be scheduled itself, but it *does*
/// have a task info structure (loaded into static memory).
const TASKID_SCHEDULER: TaskId = TaskId(0x8000_ffff_ffff_ffff);

static mut SCHEDULER_TASK: MaybeUninit<TaskInfo> = MaybeUninit::uninit();

#[repr(C)]
pub struct TaskInfo {
  /// Last known stack pointer of the task
  /// This field is only updated when the scheduler has control
  sp: VirtAddr,
  /// A uniquely identifiable task id
  task_id: TaskId,
  /// Flags on the current state of the task
  flags: TaskFlags,
  /// Task to be woken on end (optionally, if not null)
  wake_on_end: *const TaskInfo,
}

/// Holds the pointer to the task info structure for the currently executing task
/// This will *always* be accurate to the current task, including the scheduler
/// This is the pointer that will be checked when an preempting interrupt is triggered
/// to see if the scheduler was preempted
pub static CURRENT_TASK_PTR: AtomicPtr<TaskInfo> = AtomicPtr::null();
/// This pointer shadows the current task pointer, and does not get updated
/// outside of the main code of the scheduler
/// The scheduler can use this to see what task was preempted
static STALE_TASK_PTR: AtomicPtr<TaskInfo> = AtomicPtr::null();

/// Loads both task pointers to point to the given task info
fn load_task_ptrs(info: &TaskInfo) {
  let ptr = unsafe { info as *const _ as *mut _ };
  STALE_TASK_PTR.store(ptr, Ordering::Relaxed);
  CURRENT_TASK_PTR.store(ptr, Ordering::Relaxed);
}

/// These are the required callee-saved registers
struct StandardStackFrame {
  rbx: u64,
  rbp: u64,
  r12: u64,
  r13: u64,
  r14: u64,
  r15: u64,
}

/// This kind of structure ensures that the top of the stack is identical
/// regardless of whether the function was interrupted or not
#[repr(C)]
struct InterruptedStackFrame {
  rflags: u64,
  rax: u64,
  rcx: u64,
  rdx: u64,
  rsi: u64,
  rdi: u64,
  r8: u64,
  r9: u64,
  r10: u64,
  r11: u64,
  standard: StandardStackFrame,
}

#[unsafe(naked)]
unsafe extern "C" fn scheduler_preempt() -> ! {
  #[unsafe(naked)]
  unsafe extern "C" fn scheduler_jump_out() {
    core::arch::naked_asm! {
      "cli",
      "lea rax, {schedptr}",
      "mov {curtaskptr}, rax",
      "mov rax, 0",
      "movabs [{eoi}], rax",
      "call {resume_fn}",
      resume_fn = sym resume_scheduler,
      eoi = const vmem::LAPIC_EOI_ADDR.as_u64(),
      schedptr = sym SCHEDULER_TASK,
      curtaskptr = sym CURRENT_TASK_PTR,
    };
  }

  core::arch::naked_asm! {
    "pushfq",
    "push rax",
    "push rcx",

    // check if we're interrupting the scheduler on accident
    "mov r15, [{curtask}]",
    "mov r15, [r15 + {taskid_offset}]",
    "mov r14, {schedid}",
    "cmp r14, r15", // does task id of current task equal scheduler task
    "je 2f",

    "push rdx",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    // end of preempted stack frame

    // start of standard stack frame
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",

    // setup arguments for call to [`resume_scheduler`]
    "mov rdi, rsp",
    "mov rsi, {flags}",
    // interrupt stack frame
    "push {ss}",
    "mov rax, {hhdm_addr}",
    "add rax, {sp_offset}",
    "push rax",
    "pushfq",
    "push {cs}",
    "lea rax, {jump_out}",
    "push rax",
    // consumes the isf
    "iretq",
    // in case we're interrupting the scheduler
    "2:",
    "pop rcx",
    "pop rax",
    "popfq",
    "cli",
    "mov rax, 0",
    "movabs [{eoi}], rax",
    "iretq",
    cs = const 0x08,
    ss = const 0x10,
    flags = const ResumeKind::Preempted as u64,
    hhdm_addr = sym vmem::HHDM_TOP_ADDR_INNER,
    sp_offset = const vmem::SCHEDULER_SP_OFFSET,
    jump_out = sym scheduler_jump_out,
    eoi = const vmem::LAPIC_EOI_ADDR.as_u64(),
    curtask = sym CURRENT_TASK_PTR,
    schedid = const TASKID_SCHEDULER.0,
    taskid_offset = const core::mem::offset_of!(TaskInfo, task_id),
  };
}

#[inline]
pub fn yield_task() {
  release_task(ResumedState::yielded());
}

fn release_task(flags: ResumedState) {
  #[unsafe(naked)]
  unsafe extern "C" fn scheduler_jump_out() {
    core::arch::naked_asm! {
      "cli",
      "lea rax, {schedptr}",
      "mov {curtaskptr}, rax",
      "call {resume_fn}",
      resume_fn = sym resume_scheduler,
      schedptr = sym SCHEDULER_TASK,
      curtaskptr = sym CURRENT_TASK_PTR,
    };
  }

  unsafe {
    core::arch::asm! {
      // don't get any interrupts when transferring to scheduler
      "cli",

      // push interrupt frame for returning to the current process
      "mov rax, rsp",
      "push {ss}",
      "push rax",
      "pushfq",
      "push {cs}",
      "lea rax, 2f",
      "push rax",

      // push standard stack frame
      "push rbx",
      "push rbp",
      "push r12",
      "push r13",
      "push r14",
      "push r15",

      // setup arguments for call to [`resume_scheduler`]
      "mov rdi, rsp",

      // interrupt stack frame
      "push {ss}",
      "lea rax, {schedptr}",
      "push [rax + {sp_offset}]",
      "pushfq",
      "push {cs}",
      "lea rax, {jump_out}",
      "push rax",

      "iretq",
      "2:",
      in("rsi") flags.0,
      sp_offset = const core::mem::offset_of!(TaskInfo, sp),
      schedptr = sym SCHEDULER_TASK,
      cs = const 0x08,
      ss = const 0x10,
      jump_out = sym scheduler_jump_out,
      clobber_abi("C"),
    };
  }
}

#[unsafe(naked)]
unsafe extern "C" fn resume_task(task_info: *mut TaskInfo) -> ! {
  unsafe {
    core::arch::naked_asm! {
      "cli",
      "mov rsp, [rdi + {sp_offset}]",
      "mov {curtaskptr}, rdi",
      "mov {staletaskptr}, rdi",
      "sti",

      // pop the standard stack frame unconditionally
      "pop r15",
      "pop r14",
      "pop r13",
      "pop r12",
      "pop rbp",
      "pop rbx",

      // check if flag 0 is set (task was preempted/interrupted)
      // if so we need to pop off the gp registers and EFLAGS
      "btr dword ptr [rdi + {flags_offset}], 0",
      "jnc 2f",

      // if so, pop the rest of the preempted stack frame
      "pop r11",
      "pop r10",
      "pop r9",
      "pop r8",
      "pop rdi",
      "pop rsi",
      "pop rdx",
      "pop rcx",
      "pop rax",
      "popfq",

      "2:",
      "iretq",
      flags_offset = const core::mem::offset_of!(TaskInfo, flags),
      sp_offset = const core::mem::offset_of!(TaskInfo, sp),
      curtaskptr = sym CURRENT_TASK_PTR,
      staletaskptr = sym STALE_TASK_PTR,
    };
  }
}

/// This function will be called when the scheduler is resumed for any reason
extern "C" fn resume_scheduler(
  released_stack: VirtAddr,
  flags: ResumedState,
) -> ! {
  unsafe { crate::hdint::halt_timer() };

  x86_64::instructions::interrupts::enable();
  x86_64::instructions::interrupts::int3();

  serial_println!("flags: {:?}", flags);
  serial_println!("Released stack on {:?}", released_stack);

  let task_ptr = STALE_TASK_PTR.load(Ordering::Relaxed);
  let task_info = unsafe { &mut *task_ptr };

  if matches!(flags.resume_kind(), ResumeKind::Preempted) {
    task_info.flags.insert(TaskFlags::INTERRUPTED);
  }

  if matches!(flags.resume_kind(), ResumeKind::Preempted | ResumeKind::Yielded) {
    // if this was preempted or it cooperatively yielded,
    // we need to requeue it for completion
    SCHEDULER_STATE.task_queue.push(task_ptr as usize);
  }

  // reset the stack appropriately
  task_info.sp = released_stack;

  run_scheduler()
}

fn run_scheduler() -> ! {
  serial_println!("running scheduler");
  if let Some(task_ptr) = SCHEDULER_STATE.task_queue.pop() {
    let task_ptr = task_ptr as *mut TaskInfo;
    let task_info = unsafe { &*task_ptr };
    serial_println!(
      "running task {:?} (at ptr 0x{:016x})",
      task_info.task_id,
      task_ptr as usize
    );
    unsafe { crate::hdint::trigger_local_us(1_000_000) };
    unsafe { resume_task(task_ptr as *mut TaskInfo) }
  } else {
    loop {
      x86_64::instructions::hlt();
    }
  }
}

pub static mut MAPALL: MaybeUninit<MapperAllocator> = MaybeUninit::uninit();

static SCHEDULER_STATE: SchedulerState = SchedulerState {
  task_queue: SegQueue::new(),
};

struct SchedulerState {
  /// Queue of tasks which should be woken
  /// For now, the queue is not divided by priority
  /// The `usize` is actually a `*mut TaskInfo`
  // TODO: set up priority-based queues
  task_queue: SegQueue<usize>,
}

extern "C" fn start_scheduler() -> ! {
  x86_64::instructions::interrupts::int3();
  x86_64::instructions::interrupts::disable();

  unsafe {
    let mut lapic = unsafe { LocalApic::get() };
    lapic.halt_timer();

    crate::idt::load_handler(
      crate::idt::IRQ_LAPIC_CLK,
      scheduler_preempt,
      true,
    );
  }

  #[allow(static_mut_refs)]
  load_task_ptrs(unsafe { SCHEDULER_TASK.assume_init_ref() });

  serial_println!(
    "top of scheduler stack: 0x{:016x}",
    *vmem::HHDM_TOP_ADDR + vmem::SCHEDULER_SP_OFFSET
  );

  serial_println!("Running scheduler!");

  x86_64::instructions::interrupts::enable();

  run_scheduler()
}
