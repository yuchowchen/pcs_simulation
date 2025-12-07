/// High-precision retransmit signal using Condvar for instant wakeup
/// 
/// This replaces AtomicBool polling with condition variable for:
/// - Instant wakeup (no polling delay)
/// - Microsecond timing precision
/// - Lower CPU usage

use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub struct RetransmitSignal {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl RetransmitSignal {
    /// Create a new retransmit signal
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }
    
    /// Signal that new data has arrived (called by PLC receiver thread)
    /// This causes instant wakeup of the retransmit thread
    pub fn signal_reset(&self) {
        let mut reset = self.mutex.lock().unwrap();
        *reset = true;
        self.condvar.notify_one();  // Instant wakeup!
    }
    
    /// Wait for signal with timeout (called by retransmit thread)
    /// Returns true if new data arrived, false if timeout
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let mut reset = self.mutex.lock().unwrap();
        
        // If already signaled, return immediately
        if *reset {
            *reset = false;
            return true;
        }
        
        // Wait with timeout
        let mut result = self.condvar.wait_timeout(reset, timeout).unwrap();
        
        // Check if we were notified or timed out
        if *result.0 {
            *result.0 = false;  // Clear flag
            true  // New data arrived
        } else {
            false  // Timeout
        }
    }
}

impl Default for RetransmitSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_signal_wakeup() {
        let signal = Arc::new(RetransmitSignal::new());
        let signal_clone = Arc::clone(&signal);
        
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let result = signal_clone.wait_timeout(Duration::from_secs(5));
            let elapsed = start.elapsed();
            (result, elapsed)
        });
        
        // Signal after 10ms
        thread::sleep(Duration::from_millis(10));
        signal.signal_reset();
        
        let (result, elapsed) = handle.join().unwrap();
        assert!(result, "Should return true when signaled");
        assert!(elapsed.as_millis() < 50, "Should wake up quickly: {:?}", elapsed);
    }
    
    #[test]
    fn test_timeout() {
        let signal = RetransmitSignal::new();
        
        let start = Instant::now();
        let result = signal.wait_timeout(Duration::from_millis(10));
        let elapsed = start.elapsed();
        
        assert!(!result, "Should return false on timeout");
        assert!(elapsed.as_millis() >= 10, "Should wait for timeout");
        assert!(elapsed.as_millis() < 20, "Should not wait too long");
    }
}
