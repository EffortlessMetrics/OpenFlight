//! Single Producer Single Consumer ring buffer with drop-tail policy
//!
//! Provides bounded, lock-free communication channels that never block
//! the real-time producer thread.

use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;

/// Statistics for ring buffer operations
#[derive(Debug, Clone)]
pub struct RingStats {
    /// Total items produced
    pub produced: u64,
    /// Total items consumed
    pub consumed: u64,
    /// Total items dropped due to full buffer
    pub dropped: u64,
    /// Current buffer utilization (0.0 to 1.0)
    pub utilization: f64,
}

/// Single Producer Single Consumer ring buffer
pub struct SpscRing<T> {
    buffer: Vec<UnsafeCell<Option<T>>>,
    capacity: usize,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    stats: Arc<SpscRingStats>,
}

struct SpscRingStats {
    produced: AtomicU64,
    consumed: AtomicU64,
    dropped: AtomicU64,
}

impl<T> SpscRing<T> {
    /// Create new SPSC ring with power-of-2 capacity
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        assert!(capacity >= 2, "Capacity must be at least 2");

        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(UnsafeCell::new(None));
        }

        Self {
            buffer,
            capacity,
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            stats: Arc::new(SpscRingStats {
                produced: AtomicU64::new(0),
                consumed: AtomicU64::new(0),
                dropped: AtomicU64::new(0),
            }),
        }
    }

    /// Try to push item (producer side)
    /// Returns true if successful, false if dropped due to full buffer
    pub fn try_push(&self, item: T) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        let next_head = (head + 1) & self.mask;
        
        // Check if buffer is full
        if next_head == tail {
            // Buffer full - drop item (drop-tail policy)
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // Safety: We have exclusive access to this slot as producer
        unsafe {
            let slot = self.buffer.get_unchecked(head);
            *slot.get() = Some(item);
        }

        self.head.store(next_head, Ordering::Release);
        self.stats.produced.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Try to pop item (consumer side)
    pub fn try_pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        // Check if buffer is empty
        if tail == head {
            return None;
        }

        // Safety: We have exclusive access to this slot as consumer
        let item = unsafe {
            let slot = self.buffer.get_unchecked(tail);
            (*slot.get()).take()
        };

        let next_tail = (tail + 1) & self.mask;
        self.tail.store(next_tail, Ordering::Release);
        
        if item.is_some() {
            self.stats.consumed.fetch_add(1, Ordering::Relaxed);
        }
        
        item
    }

    /// Get current statistics
    pub fn stats(&self) -> RingStats {
        let produced = self.stats.produced.load(Ordering::Relaxed);
        let consumed = self.stats.consumed.load(Ordering::Relaxed);
        let dropped = self.stats.dropped.load(Ordering::Relaxed);
        
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        
        let used = if head >= tail {
            head - tail
        } else {
            self.capacity - tail + head
        };
        
        let utilization = used as f64 / self.capacity as f64;

        RingStats {
            produced,
            consumed,
            dropped,
            utilization,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.produced.store(0, Ordering::Relaxed);
        self.stats.consumed.store(0, Ordering::Relaxed);
        self.stats.dropped.store(0, Ordering::Relaxed);
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let next_head = (head + 1) & self.mask;
        next_head == tail
    }
}

unsafe impl<T: Send> Send for SpscRing<T> {}
unsafe impl<T: Send> Sync for SpscRing<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_basic_operations() {
        let ring = SpscRing::new(4);
        
        // Test push/pop
        assert!(ring.try_push(1));
        assert!(ring.try_push(2));
        assert!(ring.try_push(3));
        
        assert_eq!(ring.try_pop(), Some(1));
        assert_eq!(ring.try_pop(), Some(2));
        assert_eq!(ring.try_pop(), Some(3));
        assert_eq!(ring.try_pop(), None);
    }

    #[test]
    fn test_drop_tail_policy() {
        let ring = SpscRing::new(4);
        
        // Fill buffer (capacity - 1 due to ring buffer design)
        assert!(ring.try_push(1));
        assert!(ring.try_push(2));
        assert!(ring.try_push(3));
        
        // Next push should fail (drop-tail)
        assert!(!ring.try_push(4));
        
        let stats = ring.stats();
        assert_eq!(stats.dropped, 1);
    }

    #[test]
    fn test_concurrent_access() {
        let ring = Arc::new(SpscRing::new(1024));
        let ring_producer = ring.clone();
        let ring_consumer = ring.clone();
        
        let producer = thread::spawn(move || {
            for i in 0..10000 {
                while !ring_producer.try_push(i) {
                    thread::yield_now();
                }
            }
        });
        
        let consumer = thread::spawn(move || {
            let mut received = 0;
            while received < 10000 {
                if let Some(_) = ring_consumer.try_pop() {
                    received += 1;
                }
                thread::yield_now();
            }
        });
        
        producer.join().unwrap();
        consumer.join().unwrap();
        
        let stats = ring.stats();
        assert_eq!(stats.consumed, 10000);
    }

    #[test]
    fn test_overload_behavior() {
        let ring = Arc::new(SpscRing::new(8));
        let ring_producer = ring.clone();
        
        // Fast producer, no consumer
        let producer = thread::spawn(move || {
            for i in 0..1000 {
                ring_producer.try_push(i);
            }
        });
        
        producer.join().unwrap();
        
        let stats = ring.stats();
        // Should have dropped many items
        assert!(stats.dropped > 0);
        // Should not have blocked
        assert!(stats.produced + stats.dropped == 1000);
    }
}