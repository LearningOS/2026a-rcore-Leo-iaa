//! Per-process resource-request deadlock detection.
//! References: official Guide chapter 8 (Available, Allocation, Need).
//! AI-assisted implementation; resource kinds are checked independently as specified.
use alloc::{collections::BTreeMap, vec, vec::Vec};

/// One process's optional detector, maintained even while checking is disabled.
#[derive(Default)]
pub struct DeadlockState {
    /// Whether requests must pass the safety reduction.
    pub enabled: bool,
    /// Mutex resources, each initially containing one unit.
    pub mutexes: Resources,
    /// Semaphore resources with their creation-time counts.
    pub semaphores: Resources,
}

/// Available units and each thread's current allocations and pending requests.
#[derive(Default)]
pub struct Resources {
    available: Vec<usize>,
    allocation: BTreeMap<usize, Vec<usize>>,
    need: BTreeMap<usize, Vec<usize>>,
}

impl Resources {
    /// Register a resource, extending all existing thread rows.
    pub fn add_resource(&mut self, id: usize, count: usize) {
        if self.available.len() <= id {
            self.available.resize(id + 1, 0);
        }
        self.available[id] = count;
        for row in self.allocation.values_mut() {
            row.resize(self.available.len(), 0);
        }
        for row in self.need.values_mut() {
            row.resize(self.available.len(), 0);
        }
    }
    fn ensure_thread(&mut self, tid: usize) {
        let count = self.available.len();
        self.allocation.entry(tid).or_insert_with(|| vec![0; count]);
        self.need.entry(tid).or_insert_with(|| vec![0; count]);
    }
    /// Register a request; roll it back if it would leave an unreducible wait set.
    pub fn request(&mut self, tid: usize, id: usize, enabled: bool) -> bool {
        self.ensure_thread(tid);
        self.need.get_mut(&tid).unwrap()[id] += 1;
        if enabled && !self.safe() {
            self.need.get_mut(&tid).unwrap()[id] -= 1;
            false
        } else {
            true
        }
    }
    fn safe(&self) -> bool {
        let mut work = self.available.clone();
        let mut finished = BTreeMap::new();
        loop {
            let mut changed = false;
            for (&tid, allocated) in self.allocation.iter() {
                if finished.contains_key(&tid) {
                    continue;
                }
                if self.need[&tid]
                    .iter()
                    .zip(work.iter())
                    .all(|(need, free)| need <= free)
                {
                    for (free, held) in work.iter_mut().zip(allocated.iter()) {
                        *free += held;
                    }
                    finished.insert(tid, ());
                    changed = true;
                }
            }
            if !changed {
                return finished.len() == self.allocation.len();
            }
        }
    }
    /// Commit a successful acquisition after the underlying primitive returns.
    pub fn acquired(&mut self, tid: usize, id: usize) {
        self.ensure_thread(tid);
        self.need.get_mut(&tid).unwrap()[id] -= 1;
        self.available[id] -= 1;
        self.allocation.get_mut(&tid).unwrap()[id] += 1;
    }
    /// Publish a released unit before waking a possible waiter.
    pub fn released(&mut self, tid: usize, id: usize) {
        self.ensure_thread(tid);
        let held = &mut self.allocation.get_mut(&tid).unwrap()[id];
        *held = held.saturating_sub(1);
        self.available[id] += 1;
    }
    /// Forget a terminated thread; do not invent releases the primitive did not perform.
    pub fn remove_thread(&mut self, tid: usize) {
        self.allocation.remove(&tid);
        self.need.remove(&tid);
    }
}
