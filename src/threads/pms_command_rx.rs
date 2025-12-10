use crate::goose::packet_processor::PacketData;
use crate::os::linux_rt::pin_thread_to_core;
use crate::pcs::{AppIdIndex, MutablePcsData};
use crate::pms::types::PmsConfig;
use crossbeam_channel::Receiver;
use libc::sched_getcpu;
use log::{error, info, warn};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Spawns worker threads for processing GOOSE packets
///
/// Each worker thread:
/// - Is pinned to a unique CPU core for optimal performance
/// - Receives packets from a shared channel
/// - Decodes GOOSE frames
/// - Updates PCS commands based on received frames
///
/// # Arguments
/// * `packet_rx` - Receiver for incoming packets (LAN ID, PacketData)
/// * `appid_index` - Shared APPID index
/// * `mutable_data` - Shared mutable PCS data (DashMap provides internal concurrency)
/// * `num_workers` - Number of worker threads to spawn
///
/// # Returns
/// * `Vec<JoinHandle<()>>` - Vector of join handles for spawned threads
pub fn spawn_worker_threads(
    packet_rx: Receiver<(u16, PacketData)>,
    pms_config: &PmsConfig,
    num_workers: usize,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();

    // Use the actual num_workers parameter instead of hardcoded 2
    // Reserve core 0 for system, use cores 1 to num_workers for packet processing
    let worker_count = num_workers.min(num_cpus::get()).max(1);
    info!(
        "Spawning {} worker threads for packet processing",
        worker_count
    );

    for core_id in 1..worker_count {
        let rx = packet_rx.clone();

        let handle = thread::spawn(move || {
            // Pin thread to core
            if let Err(e) = pin_thread_to_core(core_id) {
                error!("Failed to pin worker thread to core {}: {}", core_id, e);
            } else {
                info!("Worker pinned to CPU: {}", unsafe { sched_getcpu() });
            }

            // Process packets from the channel
            while let Ok((lan_id, packet_data)) = rx.recv() {
                // REDUCED LOGGING: Too frequent, causes I/O contention
                // log::debug!("Worker on core {} received packet from LAN{}", core_id, lan_id);
                // Decode GOOSE frame
                let mut rx_header = Default::default();
                let mut rx_pdu = Default::default();

                if crate::goose::pdu::decodeGooseFrame(
                    &mut rx_header,
                    &mut rx_pdu,
                    &packet_data.data,
                    0,
                )
                .is_ok()
                {
                    //check if the APPID is included in the pms_command appid list
                    let appid = u16::from_be_bytes(rx_header.APPID);
                    if !pms_config.pms_command_appid_list.contains(&appid) {
                        warn!(
                            "Received GOOSE frame with unknown APPID 0x{:04X} from LAN{}",
                            appid, lan_id
                        );
                        continue;
                    }

                    // return 
                    // assign pms command to pcs according to pcs logical_id
                    pms_config.pms_command_pcs_mapping.
                    // Direct update without lock - DashMap provides per-entry locking
                    let updated = MutablePcsData::update_with_index(
                        &mutable_data_clone,
                        &appid_index_clone,
                        &rx_header,
                        &rx_pdu,
                        lan_id,
                    );
                    if !updated {
                        warn!(
                            "Failed to update PCS data for APPID {} from LAN{}",
                            u16::from_be_bytes(rx_header.APPID),
                            lan_id
                        );
                    }
                } else {
                    warn!("Failed to decode GOOSE frame from LAN{}", lan_id);
                }
            }

            info!("Worker thread on core {} shutting down", core_id);
        });
        handles.push(handle);
    }

    handles
}
