//! Synchronization and interior mutability primitives

// AI-assisted implementation; references: official rCore-Tutorial-Guide chapters 3–8.
mod condvar;
mod mutex;
mod semaphore;
mod up;

pub use condvar::Condvar;
pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::UPSafeCell;

mod deadlock;
pub use deadlock::DeadlockState;
