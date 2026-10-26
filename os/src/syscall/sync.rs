// AI-assisted implementation; references: official rCore-Tutorial-Guide chapters 3–8.
use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
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
    let process = current_process();
    let mutex: Arc<dyn Mutex> = if blocking {
        Arc::new(MutexBlocking::new())
    } else {
        Arc::new(MutexSpin::new())
    };
    let mut inner = process.inner_exclusive_access();
    let id = inner
        .mutex_list
        .iter()
        .position(Option::is_none)
        .unwrap_or(inner.mutex_list.len());
    if id == inner.mutex_list.len() {
        inner.mutex_list.push(Some(mutex));
    } else {
        inner.mutex_list[id] = Some(mutex);
    }
    inner.deadlock.mutexes.add_resource(id, 1);
    id as isize
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let tid = current_tid();
    let mut inner = process.inner_exclusive_access();
    let mutex = match inner.mutex_list.get(mutex_id).and_then(|item| item.clone()) {
        Some(item) => item,
        None => return -1,
    };
    let enabled = inner.deadlock.enabled;
    if !inner.deadlock.mutexes.request(tid, mutex_id, enabled) {
        return -0xdead;
    }
    drop(inner);
    mutex.lock();
    process
        .inner_exclusive_access()
        .deadlock
        .mutexes
        .acquired(tid, mutex_id);
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let tid = current_tid();
    let mut inner = process.inner_exclusive_access();
    let mutex = match inner.mutex_list.get(mutex_id).and_then(|item| item.clone()) {
        Some(item) => item,
        None => return -1,
    };
    inner.deadlock.mutexes.released(tid, mutex_id);
    drop(inner);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    if res_count > isize::MAX as usize {
        return -1;
    }
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    let id = inner
        .semaphore_list
        .iter()
        .position(Option::is_none)
        .unwrap_or(inner.semaphore_list.len());
    let semaphore = Some(Arc::new(Semaphore::new(res_count)));
    if id == inner.semaphore_list.len() {
        inner.semaphore_list.push(semaphore);
    } else {
        inner.semaphore_list[id] = semaphore;
    }
    inner.deadlock.semaphores.add_resource(id, res_count);
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let tid = current_tid();
    let mut inner = process.inner_exclusive_access();
    let sem = match inner
        .semaphore_list
        .get(sem_id)
        .and_then(|item| item.clone())
    {
        Some(item) => item,
        None => return -1,
    };
    inner.deadlock.semaphores.released(tid, sem_id);
    drop(inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let tid = current_tid();
    let mut inner = process.inner_exclusive_access();
    let sem = match inner
        .semaphore_list
        .get(sem_id)
        .and_then(|item| item.clone())
    {
        Some(item) => item,
        None => return -1,
    };
    let enabled = inner.deadlock.enabled;
    if !inner.deadlock.semaphores.request(tid, sem_id, enabled) {
        return -0xdead;
    }
    drop(inner);
    sem.down();
    process
        .inner_exclusive_access()
        .deadlock
        .semaphores
        .acquired(tid, sem_id);
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
    let process = current_process();
    let tid = current_tid();
    let mut inner = process.inner_exclusive_access();
    let condvar = match inner
        .condvar_list
        .get(condvar_id)
        .and_then(|item| item.clone())
    {
        Some(item) => item,
        None => return -1,
    };
    let mutex = match inner.mutex_list.get(mutex_id).and_then(|item| item.clone()) {
        Some(item) => item,
        None => return -1,
    };
    // Condvar::wait releases then reacquires the mutex internally; account for both.
    inner.deadlock.mutexes.released(tid, mutex_id);
    inner.deadlock.mutexes.request(tid, mutex_id, false);
    drop(inner);
    condvar.wait(mutex);
    process
        .inner_exclusive_access()
        .deadlock
        .mutexes
        .acquired(tid, mutex_id);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    if enabled > 1 {
        return -1;
    }
    current_process().inner_exclusive_access().deadlock.enabled = enabled == 1;
    0
}

fn current_tid() -> usize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid
}
