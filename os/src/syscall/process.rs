//! Process management syscalls
use core::mem::size_of;

use crate::mm::{PTEFlags, translated_byte_buffer_user, VirtAddr};
use crate::timer::get_time_us;
use crate::task::{mmap, munmap, change_program_brk, current_user_token, exit_current_and_run_next, report_syscall, suspend_current_and_run_next};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    const BYTELEN: usize = size_of::<usize>() / size_of::<u8>();
    let us = get_time_us();
    let mut info = [0u8; BYTELEN * 2];
    info[0..BYTELEN].copy_from_slice(&(us / 1_000_000).to_ne_bytes());
    info[BYTELEN..BYTELEN * 2].copy_from_slice(&(us % 1_000_000).to_ne_bytes());

    let token = current_user_token();
    if let Some(target) = translated_byte_buffer_user(token,
                                                    _ts as *const u8,
                                                    BYTELEN * 2,
                                                PTEFlags::W) {
        let mut start = 0;
        for seg in target {
            seg.copy_from_slice(&info[start..start + seg.len()]);
            start = start + seg.len();
        }
        0
    } else {
        -1
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            let token = current_user_token();
            if let Some(target) = translated_byte_buffer_user(token,
                _id as *const u8, 
                1, PTEFlags::R) {
                target[0][0] as isize
            } else {
                -1
            }
        },
        1 => {
            let token = current_user_token();
            if let Some(mut target) = translated_byte_buffer_user(token,
                    _id as *const u8, 
                    1, PTEFlags::W) {
                target[0][0] = _data as u8;
                0
            } else {
                -1
            }
        },
        2 => {
            report_syscall(_id) as isize
        },
        _ => { return -1 }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap");
    if VirtAddr(_start).aligned() == false {
        error!("kernel: VirtAddr {:?} unaligned", _start);
        return -1;
    }
    if ((_port & !0x7) != 0) || ((_port & 0x7) == 0) {
        error!("kernel: invalid port request {:?}", _port);
        return -1;
    }
    mmap(_start, _len, _port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if VirtAddr(_start).aligned() == false {
        error!("kernel: VirtAddr {:?} unaligned", _start);
        return -1;
    }
    munmap(_start, _len)
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
