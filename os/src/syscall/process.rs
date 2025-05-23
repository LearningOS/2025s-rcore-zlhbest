//! Process management syscalls

use crate::{
    mm::{MapPermission, PageTable, PhysAddr, VirtAddr},
    task::{
        change_program_brk, current_task_call_count, current_task_memory_area,
        current_task_memory_area_remove, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

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
/// 完成get_time函数实现
/// 思路，先看原先是怎么实现的
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    // 这样实现是有问题的，这里的ts 获取的是用户的地址空间，应该放到用户空间上
    unsafe {
        // 这里就应该找到真正的物理地址, 这里是卡住了而不是报错，很奇怪
        // 获取到真实的物理地址
        let page_table = PageTable::from_token(current_user_token());
        let start = ts as usize;
        let start_va = VirtAddr::from(start);
        let vpn = start_va.floor();
        let offset = start_va.page_offset(); // 页内偏移
        let ppn = page_table.translate(vpn).unwrap().ppn();
        let par = PhysAddr::from(ppn);
        // 找到最终的地址
        let real_ts = (par.0 + offset) as *mut TimeVal;
        *real_ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let page_table = PageTable::from_token(current_user_token());
    // 考虑三种情况、
    match trace_request {
        0 => {
            // 这里是读数据，需要检查获取的数据权限是否正确，需要获取到页表项，获取页表项后检查权限
            let start_va = VirtAddr::from(id); // 获取虚拟地址
            let vpn = start_va.floor(); // 获取虚拟页号
            if let Some(pte) = page_table.translate(vpn) {
                if pte.user() && pte.readable() {
                    // 获取物理地址
                    let offset = start_va.page_offset(); // 页内偏移
                    let ppn = pte.ppn();
                    let par = PhysAddr::from(ppn);
                    let value = unsafe { core::ptr::read_volatile((par.0 + offset) as *const u8) };
                    return value as isize;
                } else {
                    return -1;
                }
            }
            -1
        }
        1 => {
            // 这里是读数据，需要检查获取的数据权限是否正确，需要获取到页表项，获取页表项后检查权限
            let start_va = VirtAddr::from(id); // 获取虚拟地址
            let vpn = start_va.floor(); // 获取虚拟页号
            if let Some(pte) = page_table.translate(vpn) {
                // 检查页表项的权限， 是否可写，是否是用户态可操作
                if pte.user() && pte.writable() {
                    // 获取物理地址
                    let offset = start_va.page_offset(); // 页内偏移
                    let ppn = pte.ppn();
                    let par = PhysAddr::from(ppn);
                    let target_address = (par.0 + offset) as *mut u8;
                    unsafe { target_address.write_volatile(data as u8) }
                    return 0;
                } else {
                    return -1;
                }
            }
            -1
        }
        2 => current_task_call_count(id) as isize,
        _ => unreachable!(),
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    let va_start = VirtAddr::from(start);
    if !va_start.aligned() {
        error!("kernel: sys_mmap NOT ALIGNED!");
        return -1;
    }
    if port & !0x7 != 0 || port & 0x7 == 0 {
        error!("kernel: sys_mmap NOT ALIGNED!");
        return -1;
    }
    // 首先需要设置U 用户态可访问的权限, 不然会报错
    let mut map_permission = MapPermission::U;
    if port & 0b0000_0001 != 0 {
        map_permission |= MapPermission::R;
    }
    if port & 0b0000_0010 != 0 {
        map_permission |= MapPermission::W;
    }
    if port & 0b0000_0100 != 0 {
        map_permission |= MapPermission::X;
    }
    // 设置U权限
    current_task_memory_area(start, start + len, map_permission)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let va_start = VirtAddr::from(start);
    if !va_start.aligned() {
        error!("kernel: sys_mmap NOT ALIGNED!");
        return -1;
    }
    // 这里找到area直接删除掉
    current_task_memory_area_remove(start, start + len)
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
