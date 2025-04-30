use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    if process_inner.deadlock_detection && mutex.is_locking() {
        return -0xDEAD;
    }
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(id, res_count)));
        id
    } else {
        let id = process_inner.semaphore_list.len();
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(id, res_count))));
        id
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let cur_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        cur_tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let deadlock_detection = process_inner.deadlock_detection;
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    if deadlock_detection {
        println!("deadlock detection os/src/syscall/sync.rs:{}", 168);
        let mut work = Vec::new();
        for sem in &process_inner.semaphore_list {
            if let Some(sem) = sem {
                let sid = sem.as_ref().sem_id;
                let mut count = sem.as_ref().inner.exclusive_access().count;
                count = count.max(0);
                work.push((sid, count));
            }
        }
        println!("deadlock detection os/src/syscall/sync.rs:{}", 178);
        println!("start print works");
        for w in work.iter() {
            println!("sid: {}, count: {}", w.0, w.1);
        }
        println!("end print works");
        println!("deadlock detection os/src/syscall/sync.rs:{}", 183);
        let mut need = Vec::new();
        let mut allocation = Vec::new();
        let mut finish = Vec::new();
        for task in &process_inner.tasks {
            if task.is_none() {
                continue;
            }
            let mut task_allocation = Vec::new();
            let mut task_need = Vec::new();
            let task = Arc::clone(task.as_ref().unwrap());
            let task_inner = task.inner_exclusive_access();
            if task_inner.res.is_none() {
                continue;
            }
            let tid = task_inner.res.as_ref().unwrap().tid;
            for sem_allocation in &task_inner.allocation {
                let sid = sem_allocation.0;
                let count = sem_allocation.1;
                println!("task: {}, sid: {}, count: {}", tid, sid, count);
                task_allocation.push((sid, count));
            }
            for sem_need in &task_inner.need {
                let sid = sem_need.0;
                let count = sem_need.1;
                println!("task: {}, sid: {}, count: {}", tid, sid, count);
                task_need.push((sid, count));
            }
            if tid == cur_tid {
                task_need.push((sem_id, 1));
            }
            allocation.push((tid, task_allocation));
            need.push((tid, task_need));
            finish.push((tid, false));
        }

        let mut flag = true;
        while flag {
            flag = false;
            for (tid, finished) in &mut finish {
                if *finished {
                    continue;
                }
                let (_, task_need) = need.iter().find(|(tid_, _)| *tid_ == *tid).unwrap();
                let mut can_finish_task = true;
                for (sid, count) in task_need {
                    if !can_finish_task {
                        break;
                    }
                    let (_, sem_count) = work.iter().find(|(sid_, _)| *sid_ == *sid).unwrap();
                    if *sem_count < *count {
                        can_finish_task = false;
                        break;
                    }
                }
                if !can_finish_task {
                    continue;
                }
                let task_allocation = allocation.iter().find(|(tid_, _)| *tid_ == *tid).map(|(_, a)| a);
                if task_allocation.is_some() {
                    let task_allocation = task_allocation.unwrap();
                    for (sid, alloc_count) in task_allocation {
                        let (_, work_count) = work.iter_mut().find(|(sid_, _)| *sid_ == *sid).unwrap();
                        *work_count += *alloc_count;
                    }
                }
                *finished = true;
                flag = true;
            }
        }
        for (_, finished) in finish {
            if !finished {
                return -0xDEAD;
            }
        }
    }
    drop(process_inner);
    sem.down();
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    if _enabled == 1 {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.deadlock_detection = true;
        return 0;
    } else if _enabled == 0 {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.deadlock_detection = false;
        return 0;
    } else {
        return -1;
    }
}
