use std::sync::{Mutex, MutexGuard, PoisonError};

/// A lock that a panic on another thread does not take down: the guard comes
/// back with whatever the panicking thread left.
pub trait Locked<T: ?Sized> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
