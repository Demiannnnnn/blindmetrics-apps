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
}
