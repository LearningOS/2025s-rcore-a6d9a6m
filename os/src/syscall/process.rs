//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    loader::get_app_data_by_name,
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};
use crate::config::PAGE_SIZE;
use crate::mm::{MapPermission, VirtAddr, VirtPageNum};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}


/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let va = VirtAddr(ts as usize);
    if let Some(pa) = get_page_from_vir(va) {
        let time_us = get_time_us();
        let tv = TimeVal {
            sec: time_us / 1_000_000,
            usec: time_us % 1_000_000,
        };
        let ts = pa.0 as *mut TimeVal;
        unsafe {
            *ts = tv;
        }
        0
    } else {
        -1
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap  IMPLEMENTED YET!");
    if start & (PAGE_SIZE - 1) != 0{
        return -1;
    }else if port & !0x7 != 0 || port & 0x7 == 0{
        return -1;
    }
    let permission= MapPermission::from_bits((port as u8) << 1).unwrap() | MapPermission::U;
    let current_task = current_task().unwrap();
    let memory_set = &mut current_task.inner_exclusive_access().memory_set;

    //check the pre vpn
    let start_vpn = VirtPageNum::from(VirtAddr(start).floor());
    let end_vpn = VirtPageNum::from(VirtAddr(start + len).ceil());
    for vpn in start_vpn.0 .. end_vpn.0 {
        if let Some(vpn) = memory_set.translate(VirtPageNum(vpn)) {
            if vpn.is_valid() {
                println!("mmap failed: address already mapped");
                return -1;
            }
        }
    }
    //map new space
    memory_set.insert_framed_area(VirtAddr::from(start) , VirtAddr::from(start+len),permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap TEST");
    let current_task = current_task().unwrap();
    let memory_set = &mut current_task.inner_exclusive_access().memory_set;
    if start & (PAGE_SIZE-1) != 0 {
        return -1;
    }
    let start_vpn = VirtPageNum::from(VirtAddr(start).floor());
    let end_vpn = VirtPageNum::from(VirtAddr(start + len).ceil());
    for vpn in start_vpn.0 .. end_vpn.0 {
        if let Some(vpn) = memory_set.translate(VirtPageNum(vpn)) {
            if !vpn.is_valid() {
                println!("mmap failed: address already mapped");
                return -1;
            }
        }
    }
    for area in memory_set.areas.iter_mut() {
        if area.vpn_range.get_start() == VirtAddr::from(start).floor() && area.vpn_range.get_end() == VirtAddr::from(start+len).ceil(){
            area.unmap(&mut memory_set.page_table);
        }
    }
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    trap_cx.x[10] = 0;


    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        new_task.exec(data);
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}
// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio >=2{
        _prio
    }else{
        -1
    }
}
