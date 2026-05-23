#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::{alloc::Layout, ffi::c_void, sync::atomic::{Atomic, Ordering}};

use alloc::{boxed::Box};
use spin::Mutex;
use x86_64::{instructions::port::Port, registers::rflags::RFlags};

use crate::serial_println;
pub use uacpi::*;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_get_rsdp(
  out_rsdp_address: *mut uacpi_phys_addr,
) -> uacpi_status {
  unsafe {
    out_rsdp_address.write(crate::limine::rsdp_addr().as_u64());
  };
  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_map(
  addr: uacpi_phys_addr,
  len: uacpi_size,
) -> *mut ::core::ffi::c_void {
  (addr + crate::limine::hhdm_offset()) as _
}

#[doc = " Unmap a virtual memory range at 'addr' with a length of 'len' bytes.\n\n NOTE: 'addr' may be misaligned, see the comment above 'uacpi_kernel_map'.\n       Similar steps to uacpi_kernel_map can be taken to retrieve the\n       virtual address originally returned by the VMM for this mapping\n       as well as its true length."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_unmap(
  addr: *mut ::core::ffi::c_void,
  len: uacpi_size,
) {
}

pub unsafe extern "C" fn uacpi_kernel_log(
  arg1: uacpi_log_level,
  arg2: *const uacpi_char,
) {
  serial_println!("{}", core::ffi::CStr::from_ptr(arg2).display());
}

unsafe extern "C" {
  pub fn uacpi_kernel_pci_device_open(
    address: uacpi_pci_address,
    out_handle: *mut uacpi_handle,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_device_close(arg1: uacpi_handle);
}
unsafe extern "C" {
  #[doc = " Read & write the configuration space of a previously open PCI device."]
  pub fn uacpi_kernel_pci_read8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: *mut uacpi_u8,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_read16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: *mut uacpi_u16,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_read32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: *mut uacpi_u32,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_write8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: uacpi_u8,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_write16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: uacpi_u16,
  ) -> uacpi_status;
}
unsafe extern "C" {
  pub fn uacpi_kernel_pci_write32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: uacpi_u32,
  ) -> uacpi_status;
}

pub unsafe extern "C" fn uacpi_kernel_io_map(
  base: uacpi_io_addr,
  len: uacpi_size,
  out_handle: *mut uacpi_handle,
) -> uacpi_status {
  unsafe {
    out_handle.cast::<u32>().write(base as _);
  }

  UACPI_STATUS_OK
}

pub unsafe extern "C" fn uacpi_kernel_io_unmap(handle: uacpi_handle) {}

#[doc = " Read/Write the IO range mapped via uacpi_kernel_io_map\n at a 0-based 'offset' within the range.\n\n NOTE:\n The x86 architecture uses the in/out family of instructions\n to access the SystemIO address space.\n\n You are NOT allowed to break e.g. a 4-byte access into four 1-byte accesses.\n Hardware ALWAYS expects accesses to be of the exact width."]
pub fn uacpi_kernel_io_read8(
  arg1: uacpi_handle,
  offset: uacpi_size,
  out_value: *mut uacpi_u8,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    out_value.write(Port::<u8>::new(port_num as u16).read());
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub fn uacpi_kernel_io_read16(
  arg1: uacpi_handle,
  offset: uacpi_size,
  out_value: *mut uacpi_u16,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    out_value.write(Port::<u16>::new(port_num as u16).read());
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub fn uacpi_kernel_io_read32(
  arg1: uacpi_handle,
  offset: uacpi_size,
  out_value: *mut uacpi_u32,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    out_value.write(Port::<u32>::new(port_num as u16).read());
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_io_write8(
  arg1: uacpi_handle,
  offset: uacpi_size,
  in_value: uacpi_u8,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    Port::<u8>::new(port_num as u16).write(in_value);
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_io_write16(
  arg1: uacpi_handle,
  offset: uacpi_size,
  in_value: uacpi_u16,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    Port::<u16>::new(port_num as u16).write(in_value);
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_io_write32(
  arg1: uacpi_handle,
  offset: uacpi_size,
  in_value: uacpi_u32,
) -> uacpi_status {
  let port_num = unsafe { arg1.cast::<u32>().read() } as usize + offset;

  if port_num > u16::MAX as usize {
    return UACPI_STATUS_INVALID_ARGUMENT;
  }

  unsafe {
    Port::<u32>::new(port_num as u16).write(in_value);
  }

  UACPI_STATUS_OK
}

#[doc = " Allocate a block of memory of 'size' bytes.\n The contents of the allocated memory are unspecified."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_alloc(size: uacpi_size) -> *mut ::core::ffi::c_void {
  unimplemented!();
  alloc::alloc::alloc(Layout::from_size_align_unchecked(size, 8)).cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_free(mem: *mut ::core::ffi::c_void) {
#[unsafe(no_mangle)]
  unimplemented!();
}

#[doc = " Returns the number of nanosecond ticks elapsed since boot,\n strictly monotonic."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_get_nanoseconds_since_boot() -> uacpi_u64 {
  unimplemented!();
}

#[doc = " Spin for N microseconds."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_stall(usec: uacpi_u8) {
  crate::sys::sleep_local_us(usec as _);
}

#[doc = " Sleep for N milliseconds."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_sleep(msec: uacpi_u64) {
  unimplemented!()
}

#[doc = " Create/free an opaque non-recursive kernel mutex object."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_create_mutex() -> uacpi_handle {
  Box::into_raw(Box::new(Atomic::<bool>::new(false))).cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_free_mutex(arg1: uacpi_handle) {
  core::mem::drop(Box::from_raw(arg1.cast::<Atomic<bool>>()));
}

#[doc = " Create/free an opaque kernel (semaphore-like) event object."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_create_event() -> uacpi_handle {
  Box::into_raw(Box::new(Atomic::<u16>::new(0))).cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_free_event(arg1: uacpi_handle) {
  core::mem::drop(Box::from_raw(arg1.cast::<Atomic<u16>>()));
}

#[doc = " Returns a unique identifier of the currently executing thread.\n\n The returned thread id cannot be UACPI_THREAD_ID_NONE."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_get_thread_id() -> uacpi_thread_id {
  core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_disable_interrupts() -> uacpi_interrupt_state {
  let state = x86_64::registers::rflags::read();

  x86_64::instructions::interrupts::disable();

  state.bits()
}

#[doc = " Restore the state of the interrupt flags to the kernel-defined value provided\n in 'state'."]
#[unsafe(no_mangle)]
pub fn uacpi_kernel_restore_interrupts(state: uacpi_interrupt_state) {
  if RFlags::from_bits(state).unwrap().contains(RFlags::INTERRUPT_FLAG) {
    x86_64::instructions::interrupts::enable();
  }
}

#[doc = " Try to acquire the mutex with a millisecond timeout.\n\n The timeout value has the following meanings:\n 0x0000 - Attempt to acquire the mutex once, in a non-blocking manner\n 0x0001...0xFFFE - Attempt to acquire the mutex for at least 'timeout'\n                   milliseconds\n 0xFFFF - Infinite wait, block until the mutex is acquired\n\n The following are possible return values:\n 1. UACPI_STATUS_OK - successful acquire operation\n 2. UACPI_STATUS_TIMEOUT - timeout reached while attempting to acquire (or the\n                           single attempt to acquire was not successful for\n                           calls with timeout=0)\n 3. Any other value - signifies a host internal error and is treated as such"]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_acquire_mutex(
  arg1: uacpi_handle,
  arg2: uacpi_u16,
) -> uacpi_status {
  let mutex = unsafe { &*arg1.cast::<Atomic<bool>>() };
  if mutex.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
    UACPI_STATUS_OK
  } else {
    panic!("mutex already locked on single threaded code")
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_release_mutex(arg1: uacpi_handle) {
  let mutex = unsafe { &*arg1.cast::<Atomic<bool>>() };
  mutex.store(false, Ordering::Release);
}

#[doc = " Try to wait for an event (counter > 0) with a millisecond timeout.\n A timeout value of 0xFFFF implies infinite wait.\n\n The internal counter is decremented by 1 if wait was successful.\n\n A successful wait is indicated by returning UACPI_TRUE."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_wait_for_event(
  arg1: uacpi_handle,
  arg2: uacpi_u16,
) -> uacpi_bool {
  let semaphore = unsafe { &*arg1.cast::<Atomic<u16>>() };
  if semaphore.try_update(Ordering::AcqRel, Ordering::Acquire, |v| {
    (v > 0).then(|| v - 1)
  }).is_ok() {
    true
  } else {
    panic!("no event waiting on single threaded code");
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_signal_event(arg1: uacpi_handle) {
  let semaphore = unsafe { &*arg1.cast::<Atomic<u16>>() };
  semaphore.fetch_add(1, Ordering::Relaxed);
}

#[doc = " Reset the event counter to 0."]
#[unsafe(no_mangle)]
pub fn uacpi_kernel_reset_event(arg1: uacpi_handle) {
  let semaphore = unsafe { &*arg1.cast::<Atomic<u16>>() };
  semaphore.store(0, Ordering::Relaxed);
}

#[doc = " Handle a firmware request.\n\n Currently either a Breakpoint or Fatal operators."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_handle_firmware_request(
  arg1: *mut uacpi_firmware_request,
) -> uacpi_status {
  let req = &*arg1;
  match req.type_ as u32 {
    uacpi_firmware_request_type_UACPI_FIRMWARE_REQUEST_TYPE_BREAKPOINT => x86_64::instructions::interrupts::int3(),
    uacpi_firmware_request_type_UACPI_FIRMWARE_REQUEST_TYPE_FATAL => panic!("fatal request"),
    _ => unimplemented!()
  }

  UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_install_interrupt_handler(
  irq: uacpi_u32,
  arg1: uacpi_interrupt_handler,
  ctx: uacpi_handle,
  out_irq_handle: *mut uacpi_handle,
) -> uacpi_status {
  unimplemented!()
}

#[doc = " Uninstall an interrupt handler. 'irq_handle' is the value returned via\n 'out_irq_handle' during installation."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_uninstall_interrupt_handler(
  arg1: uacpi_interrupt_handler,
  irq_handle: uacpi_handle,
) -> uacpi_status {
  unimplemented!()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_create_spinlock() -> uacpi_handle {
  let spinlock = Box::into_raw(Box::new(Atomic::<bool>::new(false)));
  spinlock as _
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_free_spinlock(arg1: uacpi_handle) {
  core::mem::drop(Box::from_raw(arg1.cast::<Atomic<bool>>()));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_lock_spinlock(arg1: uacpi_handle) -> uacpi_cpu_flags {
  let flags = x86_64::registers::rflags::read();

  let spinlock = unsafe { &*arg1.cast::<Atomic<bool>>() };
  x86_64::instructions::interrupts::disable();

  if spinlock.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
    flags.bits()
  } else {
    panic!("spinlock unable to be unlocked in single-threaded, uninterrupted code");
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_unlock_spinlock(
  arg1: uacpi_handle,
  arg2: uacpi_cpu_flags,
) {
  let flags = RFlags::from_bits(arg2).unwrap();

  let spinlock = unsafe { &*arg1.cast::<Atomic<bool>>() };
  spinlock.store(false, Ordering::Release);

  if flags.contains(RFlags::INTERRUPT_FLAG) {
    x86_64::instructions::interrupts::enable();
  }
}

#[doc = " Schedules deferred work for execution.\n Might be invoked from an interrupt context."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_schedule_work(
  arg1: uacpi_work_type,
  arg2: uacpi_work_handler,
  ctx: uacpi_handle,
) -> uacpi_status {
  unimplemented!()
}

#[doc = " Waits for two types of work to finish:\n 1. All in-flight interrupts installed via uacpi_kernel_install_interrupt_handler\n 2. All work scheduled via uacpi_kernel_schedule_work\n\n Note that the waits must be done in this order specifically."]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_wait_for_work_completion() -> uacpi_status {
  unimplemented!()
}
