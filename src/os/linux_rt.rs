use anyhow::Result;
use libc::{
    clock_gettime, clock_nanosleep, cpu_set_t, mlockall, pthread_self,
    pthread_setaffinity_np, sched_param, sched_setscheduler, timespec,
    CLOCK_MONOTONIC, CPU_SET, CPU_ZERO, MCL_CURRENT, MCL_FUTURE, SCHED_FIFO, TIMER_ABSTIME,
};
use log::{error, info, warn};
use std::io;

/// Pin the current thread to a specific CPU core
pub fn pin_thread_to_core(core_id: usize) -> Result<()> {
    unsafe {
        let mut set: cpu_set_t = std::mem::zeroed();
        CPU_ZERO(&mut set);
        CPU_SET(core_id, &mut set);
        let tid = pthread_self();
        let res = pthread_setaffinity_np(tid, std::mem::size_of::<cpu_set_t>(), &set);
        if res != 0 {
            anyhow::bail!(
                "Failed to set thread affinity to core {}: {}",
                core_id,
                io::Error::last_os_error()
            );
        }
    }
    info!("Thread pinned to CPU core {}", core_id);
    Ok(())
}

/// Set real-time SCHED_FIFO priority (1-99, higher = more priority)
/// Requires CAP_SYS_NICE capability or root privileges
pub fn set_realtime_priority(priority: i32) -> Result<()> {
    if priority < 1 || priority > 99 {
        anyhow::bail!("Priority must be between 1 and 99, got {}", priority);
    }

    unsafe {
        let param = sched_param {
            sched_priority: priority,
        };
        let res = sched_setscheduler(0, SCHED_FIFO, &param);
        if res != 0 {
            anyhow::bail!(
                "Failed to set RT priority {}: {}",
                priority,
                io::Error::last_os_error()
            );
        }
    }
    info!("Real-time priority set to {}", priority);
    Ok(())
}

/// Lock all current and future memory pages to prevent swapping
/// Requires CAP_IPC_LOCK capability or root privileges
pub fn lock_memory() -> Result<()> {
    unsafe {
        let res = mlockall(MCL_CURRENT | MCL_FUTURE);
        if res != 0 {
            anyhow::bail!(
                "Failed to lock memory: {}",
                io::Error::last_os_error()
            );
        }
    }
    info!("Memory locked to prevent swapping");
    Ok(())
}

/// Pre-fault stack memory to ensure pages are resident in RAM
/// This prevents page faults during real-time execution
pub fn prefault_stack(size_bytes: usize) {
    let mut dummy = vec![0u8; size_bytes];
    // Touch every page (typically 4KB)
    for i in (0..size_bytes).step_by(4096) {
        dummy[i] = 1;
    }
    // Prevent compiler optimization from removing this code
    std::hint::black_box(dummy);
    info!("Pre-faulted {} bytes of stack memory", size_bytes);
}

/// Get current monotonic time (not affected by system time changes)
pub fn get_monotonic_time() -> Result<timespec> {
    unsafe {
        let mut ts: timespec = std::mem::zeroed();
        let res = clock_gettime(CLOCK_MONOTONIC, &mut ts);
        if res != 0 {
            anyhow::bail!(
                "Failed to get monotonic time: {}",
                io::Error::last_os_error()
            );
        }
        Ok(ts)
    }
}

/// Sleep until an absolute time using CLOCK_MONOTONIC
/// This is more accurate than relative sleep for periodic tasks
pub fn sleep_until(wake_time: timespec) -> Result<()> {
    unsafe {
        let res = clock_nanosleep(
            CLOCK_MONOTONIC,
            TIMER_ABSTIME,
            &wake_time,
            std::ptr::null_mut(),
        );
        if res != 0 && res != libc::EINTR {
            anyhow::bail!("clock_nanosleep failed with code: {}", res);
        }
    }
    Ok(())
}

/// Add nanoseconds to a timespec structure
/// Handles overflow into seconds correctly
pub fn timespec_add_ns(ts: &mut timespec, ns: i64) {
    ts.tv_nsec += ns;
    while ts.tv_nsec >= 1_000_000_000 {
        ts.tv_nsec -= 1_000_000_000;
        ts.tv_sec += 1;
    }
    while ts.tv_nsec < 0 {
        ts.tv_nsec += 1_000_000_000;
        ts.tv_sec -= 1;
    }
}

/// Calculate the difference between two timespecs in nanoseconds
pub fn timespec_diff_ns(start: &timespec, end: &timespec) -> i64 {
    let sec_diff = (end.tv_sec - start.tv_sec) as i64;
    let nsec_diff = (end.tv_nsec - start.tv_nsec) as i64;
    sec_diff * 1_000_000_000 + nsec_diff
}

/// Complete real-time initialization for the current thread
/// This combines all RT setup steps in the correct order
///
/// # Arguments
/// * `core_id` - CPU core to pin this thread to
/// * `priority` - SCHED_FIFO priority (1-99, higher = more important)
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with error description on failure
pub fn init_realtime_thread(core_id: usize, priority: i32) -> Result<()> {
    info!(
        "Initializing real-time thread: core={}, priority={}",
        core_id, priority
    );

    // Step 1: Lock memory first to prevent any paging
    if let Err(e) = lock_memory() {
        error!("Failed to lock memory: {}", e);
        warn!("Continuing without memory locking (may cause latency spikes)");
    }

    // Step 2: Pre-fault stack to ensure pages are resident
    prefault_stack(8 * 1024 * 1024); // 8MB stack

    // Step 3: Pin to CPU core
    pin_thread_to_core(core_id)?;

    // Step 4: Set real-time priority (must be last)
    if let Err(e) = set_realtime_priority(priority) {
        error!("Failed to set RT priority: {}", e);
        warn!("Continuing without RT priority (timing may not be deterministic)");
    }

    info!("Real-time thread initialization complete");
    Ok(())
}
