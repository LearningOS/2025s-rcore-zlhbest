//! Process management syscalls

use crate::{
    task::{current_task_call_count, exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
/// 实现yield函数，该函数的作用是暂停当前进程
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
/// 实现sys_trace 函数实现要求
/// 1、如果 trace_request 为 0，则 id 应被视作 *const u8 ，表示读取当前任务 id 地址处一个字节的无符号整数值。
/// 此时应忽略 data 参数。返回值为 id 地址处的值。
///  * 这里读取当前任务id地址的一个数值，还没有涉及地址空间，那么就通过偏移量搞定即可
/// 2、如果 trace_request 为 1，则 id 应被视作 *mut u8 ，表示写入 data （作为 u8，即只考虑最低位的一个字节）到该用户程序 id 地址处。返回值应为0。
/// 3、如果 trace_request 为 2，表示查询当前任务调用编号为 id 的系统调用的次数，返回值为这个调用次数。本次调用也计入统计 。
/// 第三个要求是全部的系统调用次数，而不是一个的。
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    // 首先根据trace_request判断
    match trace_request {
        0 => {
            // 这个地方应该是0x80440000开头才对，为什么是0x80227f0e 按理说不应该是存放在8044开头嘛？在这里不太理解
            // 可能出现的原因是 在编译的时候，编译器将确定好的变量放在了一起。 而不是全部放在程序内部
            let value = unsafe { core::ptr::read_volatile(id as *const u8) };
            value as isize
        }
        1 => {
            let target_address = id as *mut u8;
            unsafe { (target_address as *mut u8).write_volatile(data as u8) }
            0
        }
        2 => {
            // 获取这个任务的所有系统调用次数
            current_task_call_count(id) as isize
        }
        _ => unreachable!(),
    }
}
