use std::sync::atomic::Ordering;

use portable_atomic::AtomicU64;

pub struct IdCounter {
    value: AtomicU64,
}

impl IdCounter {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(1),
        }
    }

    pub fn next(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for IdCounter {
    fn default() -> Self {
        Self::new()
    }
}
