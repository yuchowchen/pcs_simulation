// Enhanced Worker Threads with Command Extraction
// Implements README requirement #5

use crate::goose::packet_processor::PacketData;
use crate::os::linux_rt::pin_thread_to_core;
use crate::pcs::{AppIdIndex, MutablePcsData};
use crossbeam_channel::Receiver;
use libc::sched_getcpu;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub fn spawn_worker_threads_enhanced(
    packet_rx: Receiver<(u16, PacketData)>,
    appid_index: Arc<AppIdIndex>,
    mutable_data: Arc<MutablePcsData>,
    num_workers: usize,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();
    let worker_count = num_workers.min(num_cpus::get()).max(3);
    info!("Spawning {} enhanced worker threads with command extraction", worker_count);
    
    for core_id in 1..worker_count {
        let rx = packet_rx.clone();
        let appid_index_clone = Arc::clone(&appid_index);
        let mutable_data_clone = Arc::clone(&mutable_data);

        let handle = thread::spawn(move || {
            if let Err(e) = pin_thread_to_core(core_id) {
                error!("Failed to pin worker to core {}: {}", core_id, e);
            } else {
                info!("Enhanced worker pinned to CPU: {}", unsafe { sched_getcpu() });
            }

            while let Ok((lan_id, packet_data)) = rx.recv() {
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
                    let appid = u16::from_be_bytes(rx_header.APPID);
                    
                    let updated = MutablePcsData::update_with_index(
                        &mutable_data_clone,
                        &appid_index_clone,
                        &rx_header,
                        &rx_pdu,
                        lan_id,
                    );
                    
                    if !updated {
                        warn!("Failed to update PCS data for APPID 0x{:04X} from LAN{}", appid, lan_id);
                        continue;
                    }
                    
                    // COMMAND EXTRACTION
                    info!("Received command GOOSE: APPID 0x{:04X}, LAN{}", appid, lan_id);
                    
                    // Get the PCS logical_id for this APPID
                    let pcs_info = match lan_id {
                        1 => appid_index_clone.appid_to_logical_lan1.get(&appid),
                        2 => appid_index_clone.appid_to_logical_lan2.get(&appid),
                        _ => {
                            warn!("Invalid LAN ID: {}", lan_id);
                            continue;
                        }
                    };
                    
                    if let Some(&(logical_id, ref _pcs_type)) = pcs_info {
                        // For command frames, extract and apply commands
                        // TODO: Load subscriber mapping to get numberOfPcs and proper indexing
                        let pcs_index = 0; // Simplified - assumes single PCS per APPID
                        let total_pcs_count = 1; // Will be loaded from subscriber mapping
                        
                        let result = match lan_id {
                            1 => {
                                if let Some(mut pcs_entry) = mutable_data_clone.pcs_all_lan1.get_mut(&logical_id) {
                                    pcs_entry.extract_and_apply_commands(pcs_index, total_pcs_count)
                                } else {
                                    Err(anyhow::anyhow!("PCS logical_id {} not found in LAN1", logical_id))
                                }
                            }
                            2 => {
                                if let Some(mut pcs_entry) = mutable_data_clone.pcs_all_lan2.get_mut(&logical_id) {
                                    pcs_entry.extract_and_apply_commands(pcs_index, total_pcs_count)
                                } else {
                                    Err(anyhow::anyhow!("PCS logical_id {} not found in LAN2", logical_id))
                                }
                            }
                            _ => Err(anyhow::anyhow!("Invalid LAN ID: {}", lan_id)),
                        };
                        
                        match result {
                            Ok(_) => {
                                debug!("Successfully extracted commands for PCS logical_id {} from LAN{}", logical_id, lan_id);
                            }
                            Err(e) => {
                                warn!("Failed to extract commands for PCS logical_id {}: {}", logical_id, e);
                            }
                        }
                    }
                } else {
                    warn!("Failed to decode GOOSE frame from LAN{}", lan_id);
                }
            }

            info!("Enhanced worker thread on core {} shutting down", core_id);
        });
        handles.push(handle);
    }

    handles
}
