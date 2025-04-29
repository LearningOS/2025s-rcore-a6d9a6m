//! Process management syscalls

use crate::config::PAGE_SIZE;
use crate::mm::{get_page_from_vir, trace_read, trace_write, MapPermission, VirtAddr, VirtPageNum};
use crate::task::{change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TimeVal, TASK_MANAGER};
use crate::timer::get_time_us;

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
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

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            let data_ptr = id as *const u8;
            let data = trace_read(current_user_token(), data_ptr as usize);
            if let Some(has) = data {
                has as isize
            } else {
                -1
            }
        }
        1 => {
            let data_ptr = id as *mut u8;
            if trace_write(current_user_token(), data as u8, data_ptr as usize) {
                0
            } else {
                -1
            }

        }
        2 => {
            let inner =  TASK_MANAGER.inner.exclusive_access();
            let current = inner.current_task;
            let ans = inner.tasks[current].syscall_times[id];
            drop(inner);

            ans as isize

        }
        _ => {
            trace!("Unsupported trace request: {}", trace_request);
            -1
        }
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
    let mut inner =  TASK_MANAGER.inner.exclusive_access();
    let current = inner.current_task;
    let memory_set = &mut inner.tasks[current].memory_set;

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
    drop(inner);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap TEST");
    let mut inner =  TASK_MANAGER.inner.exclusive_access();
    let current = inner.current_task;
    let  memory_set = &mut inner.tasks[current].memory_set;
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
    drop(inner);
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
