//! Useful objects and functions.

use std::ptr::{write_volatile, read_volatile};



/// A convenient zero-overhead wrapper for volatile fields that implement `Copy` and `Clone`.
///
/// This allows writing and reading volatile fields without resorting to `unsafe`.
///
/// An important difference with another popular implementation of `VolatileCell` is that interior
/// mutability is not allowed. Although it may at first appear logical to allow a volatile
/// `Send` and `Sync` type to provide interior mutability arguing that the value may anyway be
/// changed by another process, this may wrongly lead the user into believing that volatile
/// values can be used used for inter-thread communication (see A. D. Robinson's essay:
/// "Volatile: Almost Useless for Multi-Threaded Programming").
#[derive(Copy, Clone)]
#[repr(C)]
pub struct VolatileCell<T> {
    value: T,
}

impl<T> VolatileCell<T> {
    /// Creates a new `VolatileCell` containing the given value.
    pub fn new(value: T) -> VolatileCell<T> {
        VolatileCell {
            value: value,
        }
    }

    /// Returns a copy of the contained value.
    #[inline]
    pub fn get(&self) -> T {
        unsafe {
            read_volatile(&self.value as *const T)
        }
    }

    /// Sets the contained value.
    #[inline]
    pub fn set(&mut self, value: T) {
        unsafe {
            write_volatile(&mut self.value as *mut T, value);
        }
    }
}

