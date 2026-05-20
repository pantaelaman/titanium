use alloc::{
  boxed::Box,
  collections::{btree_map::BTreeMap, vec_deque::VecDeque},
  sync::Arc,
  task::Wake,
};
use core::{
  mem::MaybeUninit,
  pin::Pin,
  sync::atomic::{AtomicU64, Ordering},
  task::{Context, Poll, RawWaker, Waker},
};
use crossbeam::queue::ArrayQueue;

pub struct Executor {
  tasks: BTreeMap<TaskId, Task>,
  task_queue: Arc<ArrayQueue<TaskId>>,
  waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
  pub fn new() -> Self {
    let tasks = BTreeMap::new();
    let waker_cache = BTreeMap::new();
    let task_queue = ArrayQueue::new(100);
    x86_64::instructions::interrupts::int3();
    Self {
      tasks,
      waker_cache,
      task_queue: Arc::new(task_queue),
    }
  }

  pub fn spawn(&mut self, task: Task) {
    let task_id = task.id;
    let None = self.tasks.insert(task.id, task) else {
      panic!("attempted to reuse task id");
    };
    self.task_queue.push(task_id).expect("task queue full");
  }

  fn run_ready(&mut self) {
    while let Some(task_id) = self.task_queue.pop() {
      let task = match self.tasks.get_mut(&task_id) {
        Some(task) => task,
        None => continue,
      };

      let waker = self
        .waker_cache
        .entry(task_id)
        .or_insert_with(|| TaskWaker::new(task_id, self.task_queue.clone()));
      let mut context = Context::from_waker(waker);
      match task.poll(&mut context) {
        Poll::Ready(()) => {
          self.tasks.remove(&task_id);
          self.waker_cache.remove(&task_id);
        }
        Poll::Pending => {}
      }
    }
  }

  pub fn run(&mut self) -> ! {
    loop {
      self.run_ready();
    }
  }
}

pub struct Task {
  future: Pin<Box<dyn Future<Output = ()>>>,
  id: TaskId,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
struct TaskId {
  id: u64,
}

impl TaskId {
  fn new() -> Self {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    TaskId {
      id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
    }
  }
}

impl Task {
  pub fn new(future: impl Future<Output = ()> + 'static) -> Self {
    Self {
      future: Box::pin(future),
      id: TaskId::new(),
    }
  }

  fn poll(&mut self, context: &mut Context) -> Poll<()> {
    self.future.as_mut().poll(context)
  }
}

struct TaskWaker {
  task_id: TaskId,
  task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
  fn wake_task(&self) {
    self.task_queue.push(self.task_id).expect("task queue full");
  }

  fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
    Waker::from(Arc::new(Self {
      task_id,
      task_queue,
    }))
  }
}

impl Wake for TaskWaker {
  fn wake(self: Arc<Self>) {
    self.wake_task();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.wake_task();
  }
}

pub unsafe fn init() {}
