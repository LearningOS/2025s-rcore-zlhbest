use crate::config::DEADLOCK_ERROR_VALUE;
use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, current_task_tid};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task_tid()
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
        current_task_tid()
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
        process_inner.available[0][id] = 1;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.available[0].push(1);
        // 新增的数组也需要扩充出来, 这里扩充的是need数组
        process_inner
            .need
            .iter_mut()
            .for_each(|item| item[0].push(0));
        // 这里扩充的是分配数组
        process_inner
            .allocation
            .iter_mut()
            .for_each(|item| item[0].push(0));
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let tid = current_task_tid();
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    // 检测是否发生死锁
    // 首先需求进程加1
    process_inner.need[tid][0][mutex_id] += 1;
    drop(process_inner);
    if process.deadlock_detect(0) {
        return DEADLOCK_ERROR_VALUE;
    }
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    // 锁住以后再进行处理
    let process = current_process();
    process.inner_exclusive_access().allocation[tid][0][mutex_id] += 1;
    process.inner_exclusive_access().available[0][mutex_id] -= 1;
    process.inner_exclusive_access().need[tid][0][mutex_id] -= 1;
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let tid = current_task_tid();
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    let process = current_process();
    process.inner_exclusive_access().allocation[tid][0][mutex_id] -= 1;
    process.inner_exclusive_access().available[0][mutex_id] += 1;
    0
}
/// semaphore create syscall 这里的信号量是存在两种的，第一种是barrier类型的，第二类是资源型的信号量。需要进行区分
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task_tid()
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
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        // 这里应该使用的是res_count
        process_inner.available[1][id] = res_count;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        // 同理
        process_inner.available[1].push(res_count);
        // 新增的数组也需要扩充出来, 这里扩充的是need数组
        process_inner
            .need
            .iter_mut()
            .for_each(|item| item[1].push(0));
        // 这里扩充的是分配数组
        process_inner
            .allocation
            .iter_mut()
            .for_each(|item| item[1].push(0));
        process_inner.semaphore_list.len() - 1
    };
    drop(process_inner);
    id as isize
}
/// semaphore up syscall up是归还资源
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let tid = current_task_tid();
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    process.inner_exclusive_access().allocation[tid][1][sem_id] -= 1;
    process.inner_exclusive_access().available[1][sem_id] += 1;
    0
}
/// semaphore down syscall down
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let tid = current_task_tid();
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    // 只有资源型才进行分配检测
    let mut process_inner = process.inner_exclusive_access();
    process_inner.need[tid][1][sem_id] += 1;
    drop(process_inner);
    if process.deadlock_detect(1) {
        return DEADLOCK_ERROR_VALUE;
    }
    let sem = Arc::clone(
        process.inner_exclusive_access().semaphore_list[sem_id]
            .as_ref()
            .unwrap(),
    );
    sem.down();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocation[tid][1][sem_id] += 1;
    // 这里应该available-1 但是问题是不发生死锁不代表目前存在可用资源，
    if process_inner.available[1][sem_id] > 0 {
        process_inner.available[1][sem_id] -= 1;
    }
    process_inner.need[tid][1][sem_id] -= 1;
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task_tid()
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
        current_task_tid()
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
        current_task_tid()
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
///
/// 完成死锁检测
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    if enabled != 0 && enabled != 1 {
        return -1;
    }
    // 获取当前任务
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.enable_deadlock_detect = enabled != 0;
    drop(process_inner);
    drop(process);
    0
}
