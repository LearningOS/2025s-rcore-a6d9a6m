//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore id
    pub semaphore_id: usize,
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(semaphore_id: usize, res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            semaphore_id,
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                let mut task_inner = task.inner_exclusive_access();
                if let Some((index, (_, count))) = task_inner.need.iter_mut().
                    enumerate().find(|(_, (id, _))| *id == self.semaphore_id) {
                    *count -= 1;
                    if *count <= 0 {
                        task_inner.need.remove(index);
                    } else {
                        panic!("semaphore unavailable, sem_id: {}", self.semaphore_id);
                    }
                }

                if let Some((_, count)) = task_inner.allocation.iter_mut()
                    .find(|(id, _)| *id == self.semaphore_id) {
                    *count += 1;
                } else {
                    task_inner.allocation.push((self.semaphore_id, 1));
                }
                drop(task_inner);
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;

        let cur_task = current_task().unwrap();
        let mut task_inner = cur_task.inner_exclusive_access();

        if inner.count < 0 {
            if let Some((_, sem_count)) = task_inner.need.iter_mut()
                .find(|(id, _)| *id == self.semaphore_id) {
                *sem_count += 1;
            } else {
                task_inner.need.push((self.semaphore_id, 1));
            }

            drop(task_inner);
            inner.wait_queue.push_back(cur_task);
            drop(inner);
            block_current_and_run_next();
        } else {
            if let Some((_, count)) = task_inner.allocation.iter_mut()
                .find(|(id, _)| *id == self.semaphore_id) {
                *count += 1;
            } else {
                task_inner.allocation.push((self.semaphore_id, 1));
            }
        }
    }
}