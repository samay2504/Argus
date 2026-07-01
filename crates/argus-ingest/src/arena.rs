#![allow(unsafe_code)]

//! A fixed-size bump allocator (arena) for zero-allocation tick processing.
//!
//! Designed to hold transient data structures generated during a tick's life cycle.
//! In the steady-state hot path, no heap allocations should occur; instead, memory
//! is carved out of the pre-allocated `TickArena`.

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;
use std::slice;

/// A bump-pointer arena allocator.
pub struct TickArena {
    start: NonNull<u8>,
    offset: usize,
    capacity: usize,
}

unsafe impl Send for TickArena {}
unsafe impl Sync for TickArena {}

impl TickArena {
    /// Creates a new arena with the specified capacity in bytes.
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 8).expect("Invalid arena layout");
        let ptr = unsafe { alloc(layout) };
        let start = NonNull::new(ptr).unwrap_or_else(|| {
            std::alloc::handle_alloc_error(layout);
        });

        Self {
            start,
            offset: 0,
            capacity,
        }
    }

    /// Allocates memory from the arena.
    ///
    /// # Panics
    ///
    /// Panics if the arena is out of memory.
    pub fn alloc_bytes(&mut self, size: usize, align: usize) -> &mut [u8] {
        let current_ptr = unsafe { self.start.as_ptr().add(self.offset) as usize };
        let align_offset = current_ptr.wrapping_add(align).wrapping_sub(1) & !(align.wrapping_sub(1));
        let padding = align_offset - current_ptr;

        if self.offset + padding + size > self.capacity {
            panic!("TickArena out of memory");
        }

        self.offset += padding;
        let result_ptr = unsafe { self.start.as_ptr().add(self.offset) };
        self.offset += size;

        unsafe { slice::from_raw_parts_mut(result_ptr, size) }
    }

    /// Resets the arena pointer to 0, allowing reuse of the memory.
    #[inline]
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

impl Drop for TickArena {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity, 8).unwrap();
        unsafe {
            dealloc(self.start.as_ptr(), layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc() {
        let mut arena = TickArena::new(1024);
        let slice = arena.alloc_bytes(16, 8);
        assert_eq!(slice.len(), 16);
        
        // Write to it
        slice.copy_from_slice(&[1u8; 16]);
        
        arena.reset();
        
        // Allocate again
        let slice2 = arena.alloc_bytes(32, 8);
        assert_eq!(slice2.len(), 32);
    }
}
