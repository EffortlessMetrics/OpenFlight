// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Zero-allocation ring buffer for RT-adjacent recording.
//!
//! The [`RingBuffer`] is backed by a fixed-size array that is fully allocated
//! at construction time. All mutations are O(1) with **no heap allocation**,
//! making it safe for use on or near the 250 Hz RT spine.

/// Default ring buffer capacity (must be a power of two for fast modulo).
pub const DEFAULT_RING_CAPACITY: usize = 4096;

/// A fixed-capacity, single-producer ring buffer.
///
/// When the buffer is full, [`push`](RingBuffer::push) overwrites the oldest
/// entry. The buffer uses a const-generic size parameter so the backing store
/// lives entirely on the stack (or inside the owning struct).
pub struct RingBuffer<T: Copy, const N: usize> {
    buf: [T; N],
    /// Next write position (always < N).
    head: usize,
    /// Number of valid entries (≤ N).
    len: usize,
}

impl<T: Copy, const N: usize> RingBuffer<T, N> {
    /// Create a new ring buffer filled with `init`.
    ///
    /// This is the only allocation; all subsequent operations are O(1).
    ///
    /// # Compile-time guarantee
    /// Instantiation with `N == 0` is a compile error.
    pub fn new(init: T) -> Self {
        // Compile-time check: N must be > 0 to avoid division-by-zero in `% N`.
        const { assert!(N > 0, "RingBuffer capacity N must be greater than zero") };
        Self {
            buf: [init; N],
            head: 0,
            len: 0,
        }
    }

    /// Push a value, overwriting the oldest entry when full.
    #[inline]
    pub fn push(&mut self, value: T) {
        self.buf[self.head] = value;
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Drain up to `min(len, dst.len())` entries into `dst`, oldest first.
    /// Returns the number of entries actually drained. Remaining entries are
    /// preserved.
    pub fn drain_to(&mut self, dst: &mut [T]) -> usize {
        let count = self.len.min(dst.len());
        let start = self.oldest_index();
        for (i, slot) in dst.iter_mut().enumerate().take(count) {
            *slot = self.buf[(start + i) % N];
        }
        self.len -= count;
        count
    }

    /// Number of valid entries currently stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the buffer contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total capacity of the buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        N
    }

    /// Peek at the oldest entry without removing it.
    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        Some(&self.buf[self.oldest_index()])
    }

    /// Peek at the newest entry without removing it.
    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let idx = (self.head + N - 1) % N;
        Some(&self.buf[idx])
    }

    /// Iterate over valid entries in chronological order (oldest first).
    pub fn iter(&self) -> RingBufferIter<'_, T, N> {
        RingBufferIter {
            buf: &self.buf,
            pos: self.oldest_index(),
            remaining: self.len,
        }
    }

    /// Index of the oldest valid entry.
    fn oldest_index(&self) -> usize {
        (self.head + N - self.len) % N
    }

    /// Clear the buffer without deallocating.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

/// Iterator over ring buffer entries in chronological order.
pub struct RingBufferIter<'a, T, const N: usize> {
    buf: &'a [T; N],
    pos: usize,
    remaining: usize,
}

impl<'a, T: Copy, const N: usize> Iterator for RingBufferIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let item = &self.buf[self.pos];
        self.pos = (self.pos + 1) % N;
        self.remaining -= 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T: Copy, const N: usize> ExactSizeIterator for RingBufferIter<'_, T, N> {}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let rb = RingBuffer::<u32, 8>::new(0);
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.capacity(), 8);
    }

    #[test]
    fn push_increases_len() {
        let mut rb = RingBuffer::<u32, 8>::new(0);
        rb.push(1);
        rb.push(2);
        assert_eq!(rb.len(), 2);
        assert!(!rb.is_empty());
    }

    #[test]
    fn push_wraps_and_overwrites_oldest() {
        let mut rb = RingBuffer::<u32, 4>::new(0);
        for i in 0..6u32 {
            rb.push(i);
        }
        // capacity is 4, wrote 6, so oldest 0,1 are gone
        assert_eq!(rb.len(), 4);
        let items: Vec<u32> = rb.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4, 5]);
    }

    #[test]
    fn drain_preserves_order() {
        let mut rb = RingBuffer::<u32, 8>::new(0);
        for i in 0..5u32 {
            rb.push(i);
        }
        let mut dst = [0u32; 8];
        let n = rb.drain_to(&mut dst);
        assert_eq!(n, 5);
        assert_eq!(&dst[..5], &[0, 1, 2, 3, 4]);
        assert!(rb.is_empty());
    }

    #[test]
    fn drain_after_wrap_preserves_order() {
        let mut rb = RingBuffer::<u32, 4>::new(0);
        for i in 0..6u32 {
            rb.push(i);
        }
        let mut dst = [0u32; 4];
        let n = rb.drain_to(&mut dst);
        assert_eq!(n, 4);
        assert_eq!(dst, [2, 3, 4, 5]);
        assert!(rb.is_empty());
    }

    #[test]
    fn drain_into_smaller_dst() {
        let mut rb = RingBuffer::<u32, 8>::new(0);
        for i in 0..5u32 {
            rb.push(i);
        }
        let mut dst = [0u32; 3];
        let n = rb.drain_to(&mut dst);
        assert_eq!(n, 3);
        assert_eq!(dst, [0, 1, 2]);
        // Remaining entries are preserved
        assert_eq!(rb.len(), 2);
        let remaining: Vec<u32> = rb.iter().copied().collect();
        assert_eq!(remaining, vec![3, 4]);
    }

    #[test]
    fn oldest_and_newest() {
        let mut rb = RingBuffer::<u32, 4>::new(0);
        assert!(rb.oldest().is_none());
        assert!(rb.newest().is_none());

        rb.push(10);
        rb.push(20);
        rb.push(30);
        assert_eq!(*rb.oldest().unwrap(), 10);
        assert_eq!(*rb.newest().unwrap(), 30);

        // Overflow
        rb.push(40);
        rb.push(50);
        assert_eq!(*rb.oldest().unwrap(), 20);
        assert_eq!(*rb.newest().unwrap(), 50);
    }

    #[test]
    fn clear_resets() {
        let mut rb = RingBuffer::<u32, 8>::new(0);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn iter_exact_size() {
        let mut rb = RingBuffer::<u32, 8>::new(0);
        for i in 0..5u32 {
            rb.push(i);
        }
        let iter = rb.iter();
        assert_eq!(iter.len(), 5);
    }

    #[test]
    fn at_exact_capacity() {
        let mut rb = RingBuffer::<u32, 4>::new(0);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 4);
        assert_eq!(rb.capacity(), 4);
        let items: Vec<u32> = rb.iter().copied().collect();
        assert_eq!(items, vec![1, 2, 3, 4]);
    }

    #[test]
    fn large_overflow_preserves_newest() {
        let mut rb = RingBuffer::<u32, 4>::new(0);
        for i in 0..10_000u32 {
            rb.push(i);
        }
        assert_eq!(rb.len(), 4);
        let items: Vec<u32> = rb.iter().copied().collect();
        assert_eq!(items, vec![9996, 9997, 9998, 9999]);
    }

    #[test]
    fn default_capacity_constant() {
        assert_eq!(DEFAULT_RING_CAPACITY, 4096);
    }

    #[test]
    fn push_single_then_drain() {
        let mut rb = RingBuffer::<u64, 4096>::new(0);
        rb.push(42);
        let mut dst = [0u64; 4096];
        let n = rb.drain_to(&mut dst);
        assert_eq!(n, 1);
        assert_eq!(dst[0], 42);
    }

    #[test]
    fn iter_empty_buffer() {
        let rb = RingBuffer::<u32, 8>::new(0);
        assert_eq!(rb.iter().count(), 0);
    }
}
