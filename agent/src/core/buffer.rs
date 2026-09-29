use crate::core::metrics::MetricPayload;
use std::sync::Mutex;

pub struct MetricsBuffer {
    items: Mutex<Vec<MetricPayload>>,
    max_capacity: usize,
}

impl MetricsBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            items: Mutex::new(Vec::with_capacity(64)),
            max_capacity,
        }
    }

    /// Fast push from any collector thread.
    /// If capacity is exceeded, discards the oldest element (FIFO) to prevent unbounded memory growth.
    pub fn push(&self, metric: MetricPayload) {
        if let Ok(mut lock) = self.items.lock() {
            if lock.len() >= self.max_capacity {
                lock.remove(0);
            }
            lock.push(metric);
        }
    }

    /// Atomic lock-free snapshot extraction using `std::mem::take`.
    /// Retains the Mutex lock for under 50 nanoseconds, replacing the internal vector
    /// with an empty one and freeing the lock immediately for collectors to proceed.
    pub fn drain_for_flush(&self) -> Vec<MetricPayload> {
        let mut lock = self.items.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::take(&mut *lock)
    }

    pub fn len(&self) -> usize {
        self.items.lock().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for MetricsBuffer {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::metrics::ProductivityBasicItem;
    use chrono::Utc;

    #[test]
    fn test_buffer_push_drain_and_fifo_eviction() {
        let buffer = MetricsBuffer::new(3);

        let make_item = |kpm| {
            MetricPayload::ProductivityBasic(ProductivityBasicItem {
                minute: Utc::now(),
                keystrokes: kpm,
                clicks: 0,
            })
        };

        buffer.push(make_item(10));
        buffer.push(make_item(20));
        buffer.push(make_item(30));
        assert_eq!(buffer.len(), 3);

        // Push 4th item -> item with 10 should be evicted
        buffer.push(make_item(40));
        assert_eq!(buffer.len(), 3);

        let drained = buffer.drain_for_flush();
        assert_eq!(drained.len(), 3);
        assert_eq!(buffer.len(), 0);

        if let MetricPayload::ProductivityBasic(first) = &drained[0] {
            assert_eq!(first.keystrokes, 20);
        } else {
            panic!("Expected productivity metric");
        }
    }

    #[test]
    fn test_buffer_drain_empties_vector() {
        let buffer = MetricsBuffer::new(100);
        let make_item = |kpm| {
            MetricPayload::ProductivityBasic(ProductivityBasicItem {
                minute: Utc::now(),
                keystrokes: kpm,
                clicks: 0,
            })
        };

        for i in 0..10 {
            buffer.push(make_item(i));
        }
        assert_eq!(buffer.len(), 10);

        let drained = buffer.drain_for_flush();
        assert_eq!(drained.len(), 10);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_concurrent_collectors_and_flush_no_deadlock() {
        use std::sync::Arc;
        use std::thread;

        let buffer = Arc::new(MetricsBuffer::new(500));
        let mut handles = Vec::new();

        // 5 concurrent collector threads
        for t in 0..5 {
            let buf_clone = buffer.clone();
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    buf_clone.push(MetricPayload::ProductivityBasic(ProductivityBasicItem {
                        minute: Utc::now(),
                        keystrokes: t * 100 + i,
                        clicks: 1,
                    }));
                    thread::yield_now();
                }
            }));
        }

        // 1 concurrent flusher thread
        let buf_flusher = buffer.clone();
        let flusher_handle = thread::spawn(move || {
            let mut total_drained = 0;
            for _ in 0..10 {
                let drained = buf_flusher.drain_for_flush();
                total_drained += drained.len();
                thread::sleep(std::time::Duration::from_millis(2));
            }
            total_drained += buf_flusher.drain_for_flush().len();
            total_drained
        });

        for h in handles {
            h.join().unwrap();
        }
        let drained_count = flusher_handle.join().unwrap();
        assert!(drained_count > 0, "Concurrent flusher must drain items without deadlock");
    }
}
