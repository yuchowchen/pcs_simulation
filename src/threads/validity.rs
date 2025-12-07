use crate::pcs::MutablePcsData;
use log::{info, warn};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Spawns the validity checking thread that periodically checks PCS validity
/// 
/// # Arguments
/// * `mutable_data` - Shared mutable PCS data (DashMap provides internal concurrency)
/// * `validity_interval_ms` - Interval in milliseconds between validity checks
/// 
/// # Returns
/// * `JoinHandle` for the spawned thread
pub fn spawn_validity_thread(
    mutable_data: Arc<MutablePcsData>,
    validity_interval_ms: u64,
) -> JoinHandle<()> {
    thread::spawn(move || {
        info!(
            "Validity checking thread started with interval: {} ms",
            validity_interval_ms
        );

        loop {
            // Sleep for the configured interval 400ms/2 = 200ms.
            thread::sleep(std::time::Duration::from_millis(validity_interval_ms));

            // Perform validity check on both LANs - DashMap allows concurrent access
            // No global lock needed - each PCS entry is locked individually
            let ((lan1_invalid, lan1_valid), (lan2_invalid, lan2_valid)) = mutable_data.check_validity_both_lans();

            // Log state transitions (newly invalid or newly valid)
            if !lan1_invalid.is_empty() {
                warn!(
                    "LAN1 VALIDITY TRANSITION: {} PCS units became invalid: {:?}",
                    lan1_invalid.len(),
                    lan1_invalid
                );
            }
            if !lan1_valid.is_empty() {
                info!(
                    "LAN1 VALIDITY TRANSITION: {} PCS units became valid (recovered): {:?}",
                    lan1_valid.len(),
                    lan1_valid
                );
            }
            if !lan2_invalid.is_empty() {
                warn!(
                    "LAN2 VALIDITY TRANSITION: {} PCS units became invalid: {:?}",
                    lan2_invalid.len(),
                    lan2_invalid
                );
            }
            if !lan2_valid.is_empty() {
                info!(
                    "LAN2 VALIDITY TRANSITION: {} PCS units became valid (recovered): {:?}",
                    lan2_valid.len(),
                    lan2_valid
                );
            }
            
            // Get overall validity statistics for monitoring
            let ((lan1_valid_count, lan1_invalid_count, lan1_total), 
                 (lan2_valid_count, lan2_invalid_count, lan2_total)) = mutable_data.get_validity_stats_both_lans();
            
            info!(
                "Validity check complete - LAN1: {}/{} valid ({} invalid), LAN2: {}/{} valid ({} invalid)",
                lan1_valid_count, lan1_total, lan1_invalid_count,
                lan2_valid_count, lan2_total, lan2_invalid_count
            );
        }
    })
}
