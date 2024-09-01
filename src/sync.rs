use core::cell::SyncUnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct LazyLock<T: 'static> {
  value: SyncUnsafeCell<MaybeUninit<T>>,
  initialiser: &'static dyn Fn() -> T,
  initialised: AtomicBool,
  initialising: AtomicBool,
}

impl<T> LazyLock<T> {
  pub const fn new(initialiser: &'static dyn Fn() -> T) -> Self {
    LazyLock {
      value: SyncUnsafeCell::new(MaybeUninit::uninit()),
      initialiser,
      initialised: AtomicBool::new(false),
      initialising: AtomicBool::new(false),
    }
  }
}

impl<T> core::ops::Deref for LazyLock<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    if self.initialised.load(Ordering::Relaxed) {
      unsafe { &*(&*self.value.get()).as_ptr() }
    } else {
      if self.initialising.load(Ordering::Acquire) {
        while !self.initialised.load(Ordering::Relaxed) {
          core::hint::spin_loop();
        }
        unsafe { &*(&*self.value.get()).as_ptr() }
      } else {
        self.initialising.store(true, Ordering::Release);
        let ret =
          unsafe { (&mut *self.value.get()).write((self.initialiser)()) };
        self.initialised.store(true, Ordering::Release);
        ret
      }
    }
  }
}

unsafe impl<T> Sync for LazyLock<T> {}

pub struct Mutex<T> {
  value: SyncUnsafeCell<T>,
  locked: AtomicBool,
}

impl<T> Mutex<T> {
  pub fn new(value: T) -> Self {
    Mutex {
      value: SyncUnsafeCell::new(value),
      locked: AtomicBool::new(false),
    }
  }

  pub fn lock(&self) -> MutexGuard<T> {
    while self.locked.load(Ordering::Acquire) {
      core::hint::spin_loop();
    }
    MutexGuard { lock: &self }
  }
}

unsafe impl<T> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
  lock: &'a Mutex<T>,
}

impl<'a, T> core::ops::Drop for MutexGuard<'a, T> {
  fn drop(&mut self) {
    self.lock.locked.store(false, Ordering::Release);
  }
}

impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.lock.value.get() }
  }
}

impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *self.lock.value.get() }
  }
}
