//! Lock-free Arc swapping utilities.
//!
//! This module provides a tiny Arc swapper for shared state that is updated
//! by cloning and swapping, avoiding mutexes in hot paths.

use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Atomic Arc wrapper for clone-and-swap updates.
pub(crate) struct ArcShift<T> {
    ptr: AtomicPtr<T>,
    _marker: PhantomData<Arc<T>>,
}

impl<T> ArcShift<T> {
    /// Create a new `ArcShift` from an owned value.
    pub(crate) fn new(value: T) -> Self {
        let arc = Arc::new(value);
        let ptr = Arc::into_raw(arc) as *mut T;
        Self {
            ptr: AtomicPtr::new(ptr),
            _marker: PhantomData,
        }
    }

    /// Load the current value as an `Arc`.
    pub(crate) fn load(&self) -> Arc<T> {
        let ptr = self.ptr.load(Ordering::Acquire);
        debug_assert!(!ptr.is_null(), "ArcShift pointer must not be null");
        // SAFETY: `ptr` was created from `Arc::into_raw` and is still stored
        // in the atomic. We increment the strong count before creating a
        // new `Arc` to ensure the allocation stays alive.
        unsafe {
            Arc::increment_strong_count(ptr);
            Arc::from_raw(ptr)
        }
    }

    /// Update the value by cloning and swapping.
    pub(crate) fn update<F>(&self, f: F)
    where
        F: Fn(&T) -> T,
    {
        loop {
            let current = self.load();
            let next = Arc::new(f(&current));
            let next_ptr = Arc::into_raw(next) as *mut T;
            let current_ptr = Arc::as_ptr(&current) as *mut T;
            match self.ptr.compare_exchange(
                current_ptr,
                next_ptr,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(old_ptr) => {
                    // SAFETY: `old_ptr` was previously created from `Arc::into_raw`.
                    unsafe {
                        drop(Arc::from_raw(old_ptr));
                    }
                    break;
                }
                Err(_) => {
                    // SAFETY: `next_ptr` was created from `Arc::into_raw` and has
                    // not been stored, so we must drop it to avoid leaking.
                    unsafe {
                        drop(Arc::from_raw(next_ptr));
                    }
                }
            }
        }
    }
}

impl<T> Drop for ArcShift<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Acquire);
        if !ptr.is_null() {
            // SAFETY: `ptr` was created from `Arc::into_raw`.
            unsafe {
                drop(Arc::from_raw(ptr));
            }
        }
    }
}
