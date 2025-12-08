/// Buffer pool for zero-allocation packet reception
/// 
/// This module provides a lock-free buffer pool that eliminates heap allocations
/// in the packet reception hot path. Buffers are pre-allocated and reused.

use crossbeam_queue::ArrayQueue;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// Maximum GOOSE packet size (including Ethernet header)
/// IEC 61850-8-1: Typical GOOSE frames are 100-500 bytes
pub const BUFFER_SIZE: usize = 1518; // Standard Ethernet MTU

/// Pooled buffer that automatically returns to pool when dropped
pub struct PooledBuffer {
    buffer: Vec<u8>,
    pool: Arc<ArrayQueue<Vec<u8>>>,
}

impl PooledBuffer {
    /// Get the actual data length (not buffer capacity)
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Set the length of valid data in the buffer
    pub fn set_len(&mut self, len: usize) {
        assert!(len <= BUFFER_SIZE, "Length exceeds buffer capacity");
        unsafe {
            self.buffer.set_len(len);
        }
    }

    /// Get mutable slice for writing data
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Copy data from slice into this buffer
    pub fn copy_from_slice(&mut self, data: &[u8]) {
        self.buffer.clear();
        self.buffer.extend_from_slice(data);
    }
}

impl Deref for PooledBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // Return buffer to pool (if pool has space)
        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.clear();
        // Reserve capacity for next use
        if buffer.capacity() < BUFFER_SIZE {
            buffer.reserve(BUFFER_SIZE - buffer.capacity());
        }
        let _ = self.pool.push(buffer); // Ignore if pool is full
    }
}

/// Lock-free buffer pool for packet reception
/// 
/// Uses crossbeam's lock-free ArrayQueue for allocation/deallocation
#[derive(Clone)]
pub struct BufferPool {
    queue: Arc<ArrayQueue<Vec<u8>>>,
}

impl BufferPool {
    /// Create a new buffer pool with specified capacity
    /// 
    /// # Arguments
    /// * `capacity` - Maximum number of buffers in the pool
    /// 
    /// # Example
    /// ```
    /// use pcs_simulator:goose::buffer_pool::BufferPool;
    /// 
    /// // Create pool with 8192 buffers (enough for 4096 channel + some margin)
    /// let pool = BufferPool::new(8192);
    /// ```
    pub fn new(capacity: usize) -> Self {
        let queue = Arc::new(ArrayQueue::new(capacity));
        
        // Pre-allocate buffers
        for _ in 0..capacity {
            let mut buffer = Vec::with_capacity(BUFFER_SIZE);
            unsafe {
                // Initialize to zeros (required for safety)
                buffer.set_len(BUFFER_SIZE);
            }
            buffer.fill(0);
            buffer.clear();
            let _ = queue.push(buffer);
        }

        Self { queue }
    }

    /// Get a buffer from the pool
    /// 
    /// Returns None if pool is exhausted (shouldn't happen in normal operation)
    pub fn acquire(&self) -> Option<PooledBuffer> {
        self.queue.pop().map(|buffer| PooledBuffer {
            buffer,
            pool: Arc::clone(&self.queue),
        })
    }

    /// Get pool statistics
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get pool capacity
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_basic() {
        let pool = BufferPool::new(10);
        assert_eq!(pool.len(), 10);
        assert_eq!(pool.capacity(), 10);

        let buffer = pool.acquire().expect("Should get buffer");
        assert_eq!(pool.len(), 9);
        assert!(buffer.len() == 0); // Buffer is cleared

        drop(buffer);
        // Buffer should be returned to pool
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(pool.len(), 10);
    }

    #[test]
    fn test_buffer_pool_exhaustion() {
        let pool = BufferPool::new(2);
        
        let _buf1 = pool.acquire().unwrap();
        let _buf2 = pool.acquire().unwrap();
        let buf3 = pool.acquire();
        
        assert!(buf3.is_none(), "Pool should be exhausted");
    }

    #[test]
    fn test_pooled_buffer_operations() {
        let pool = BufferPool::new(10);
        let mut buffer = pool.acquire().unwrap();

        // Test copy_from_slice
        let data = b"Hello, GOOSE!";
        buffer.copy_from_slice(data);
        assert_eq!(buffer.len(), data.len());
        assert_eq!(&buffer[..], data);

        // Test deref
        assert_eq!(buffer[0], b'H');
    }

    #[test]
    fn test_buffer_reuse() {
        let pool = BufferPool::new(1);
        
        {
            let mut buf = pool.acquire().unwrap();
            buf.copy_from_slice(b"test data");
            assert_eq!(pool.len(), 0);
        }
        
        // Buffer should be returned and cleared
        let buf = pool.acquire().unwrap();
        assert_eq!(buf.len(), 0);
        assert_eq!(pool.len(), 0);
    }
}
