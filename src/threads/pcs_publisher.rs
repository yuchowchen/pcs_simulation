use crossbeam_channel::Receiver;
use log::{error, info, warn};
use pnet_datalink::DataLinkSender;
use std::thread::{self, JoinHandle};

/// Spawns the GOOSE sender thread that owns both LAN1 and LAN2 transmitters
/// 
/// This thread receives encoded GOOSE frames via a channel and sends them
/// via both LAN1 and LAN2 for redundancy. If one LAN fails, the other continues.
/// 
/// # Arguments
/// * `goose_rx` - Receiver for encoded GOOSE frames
/// * `tx_lan1` - Optional LAN1 transmitter
/// * `tx_lan2` - Optional LAN2 transmitter
/// 
/// # Returns
/// * `Option<JoinHandle<()>>` - Join handle if at least one transmitter is available
pub fn spawn_pcs_goose_publisher_thread(
    goose_rx: Receiver<Vec<u8>>,
    mut tx_lan1: Option<Box<dyn DataLinkSender>>,
    mut tx_lan2: Option<Box<dyn DataLinkSender>>,
) -> Option<JoinHandle<()>> {
    if tx_lan1.is_none() && tx_lan2.is_none() {
        warn!("Neither LAN1 nor LAN2 transmitter available, GOOSE publishing disabled");
        return None;
    }

    // Log which transmitters are available
    match (&tx_lan1, &tx_lan2) {
        (Some(_), Some(_)) => {
            info!("GOOSE sender: Both LAN1 and LAN2 transmitters available - full redundancy")
        }
        (Some(_), None) => warn!("GOOSE sender: Only LAN1 transmitter available"),
        (None, Some(_)) => warn!("GOOSE sender: Only LAN2 transmitter available"),
        (None, None) => {
            error!("GOOSE sender: Neither LAN1 nor LAN2 transmitter available (should not happen)");
        }
    }

    Some(thread::spawn(move || {
        info!("GOOSE sender thread started");
        while let Ok(frame_data) = goose_rx.recv() {
            let mut lan1_sent = false;
            let mut lan2_sent = false;

            // info!(
            //     "GOOSE sender thread received frame to send ({} bytes)",
            //     frame_data.len()
            // );

            // Send via LAN1 if available
            if let Some(ref mut tx1) = tx_lan1 {
                if let Some(result) = tx1.send_to(&frame_data, None) {
                    match result {
                        Ok(_) => {
                            lan1_sent = true;
                            // info!("PCS Control GOOSE frame sent via LAN1 ({} bytes)", frame_data.len());
                        }
                        Err(e) => {
                            error!("Failed to send GOOSE frame via LAN1: {:?}", e);
                        }
                    }
                } else {
                    error!("LAN1 send operation returned None");
                }
            }

            // Send via LAN2 if available
            if let Some(ref mut tx2) = tx_lan2 {
                if let Some(result) = tx2.send_to(&frame_data, None) {
                    match result {
                        Ok(_) => {
                            lan2_sent = true;
                            // info!("PCS Control GOOSE frame sent via LAN2 ({} bytes)", frame_data.len());
                        }
                        Err(e) => {
                            error!("Failed to send GOOSE frame via LAN2: {:?}", e);
                        }
                    }
                } else {
                    error!("LAN2 send operation returned None");
                }
            }

            // Log send status
            match (lan1_sent, lan2_sent) {
                (true, true) => {
                    info!(
                        "PCS Control GOOSE frame sent via both LAN1 and LAN2 ({} bytes)",
                        frame_data.len()
                    );
                }
                (true, false) => {
                    warn!(
                        "PCS Control GOOSE frame sent only via LAN1 ({} bytes)",
                        frame_data.len()
                    );
                }
                (false, true) => {
                    warn!(
                        "PCS Control GOOSE frame sent only via LAN2 ({} bytes)",
                        frame_data.len()
                    );
                }
                (false, false) => {
                    error!("Failed to send GOOSE frame via both LANs");
                }
            }
        }
        warn!("GOOSE sender thread stopped");
    }))
}
