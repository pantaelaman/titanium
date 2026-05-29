use core::task::{RawWaker, RawWakerVTable, Waker};

static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

unsafe fn clone(task_ptr: *const ()) -> RawWaker {
  RawWaker::new(task_ptr, &RAW_WAKER_VTABLE)
}

unsafe fn wake(task_ptr: *const ()) {
  super::SCHEDULER_STATE.task_queue.push(task_ptr as usize);
}

unsafe fn wake_by_ref(task_ptr: *const ()) {
  super::SCHEDULER_STATE.task_queue.push(task_ptr as usize);
}

unsafe fn drop(task_ptr: *const ()) {}

pub fn waker_from_task_info(task_info: *const super::TaskInfo) -> Waker {
  unsafe {
    Waker::from_raw(RawWaker::new(task_info as _, &RAW_WAKER_VTABLE))
  }
}
