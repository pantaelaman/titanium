use core::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::{AtomicBool, AtomicUsize}};

struct Slot<T> {
  cell: UnsafeCell<MaybeUninit<T>>,
  filled: AtomicUsize,
}

pub struct Queue<T, const CAPACITY: usize> {
  slots: [Slot<T>; CAPACITY],
}
