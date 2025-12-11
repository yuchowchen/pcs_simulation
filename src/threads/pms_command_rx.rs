use crate::goose::packet_processor::PacketData;
use crate::os::linux_rt::pin_thread_to_core;
use crate::pcs;
use crate::pms::types::PmsGooseCmdSubscriber;
use crate::pms::types::PmsConfig;
use crossbeam_channel::Receiver;
use dashmap::DashMap;
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
/// * `pms_config` - Shared PMS configuration (Arc for thread-safe sharing)
/// * `pms_subscribers` - Shared PMS GOOSE command subscribers (DashMap provides internal concurrency)
/// * `num_workers` - Number of worker threads to spawn
///
/// # Returns
/// * `Vec<JoinHandle<()>>` - Vector of join handles for spawned threads
pub fn spawn_worker_threads(
    packet_rx: Receiver<(u16, PacketData)>,
    pms_config: Arc<PmsConfig>,
    pms_subscribers: Arc<DashMap<u16, PmsGooseCmdSubscriber>>,
    pcs_goose_publishers: Arc<DashMap<u16, pcs::types::PcsGoosePublisher>>,
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

    for core_id in 1..=worker_count {
        let rx = packet_rx.clone();
        let pms_config = Arc::clone(&pms_config);
        let pms_subscribers = Arc::clone(&pms_subscribers);

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
                    // Check if the APPID is included in the pms_command appid list
                    let appid = u16::from_be_bytes(rx_header.APPID);
                    if !pms_config.pms_command_appid_list.contains(&appid) {
                        warn!(
                            "Received GOOSE frame with unknown APPID 0x{:04X} from LAN{}",
                            appid, lan_id
                        );
                        continue;
                    }

                    // Find the corresponding pms command subscriber for this appid
                    if let Some(mut pms_entry) = pms_subscribers.get_mut(&appid) {
                        // IEC 61850-8-1 GOOSE freshness validation with restart detection:
                        // 
                        // A frame is newer if:
                        //   1. stNum (state number) is greater, OR
                        //   2. stNum is equal AND sqNum (sequence number) is greater, OR
                        //   3. Sender restart detected (stNum dropped significantly, suggesting reset to 0)
                        //
                        // Restart detection: If rx_stNum < current_stNum by large margin (e.g., > 100),
                        // assume sender restarted and accept the new frame.
                        let current_stnum = pms_entry.goosepdu.stNum;
                        let current_sqnum = pms_entry.goosepdu.sqNum;
                        let current_confrev = pms_entry.goosepdu.confRev;
                        let rx_stnum = rx_pdu.stNum;
                        let rx_sqnum = rx_pdu.sqNum;
                        let rx_confrev = rx_pdu.confRev;

                        // Detect sender restart: stNum went backwards significantly
                        const RESTART_THRESHOLD: u32 = 100;
                        let is_restart = current_stnum > RESTART_THRESHOLD && 
                                        rx_stnum < current_stnum && 
                                        (current_stnum - rx_stnum) > RESTART_THRESHOLD;

                        // Configuration revision changed also indicates restart/reconfiguration
                        let is_reconfig = rx_confrev != current_confrev;

                        let is_newer = (rx_stnum > current_stnum) || 
                                       (rx_stnum == current_stnum && rx_sqnum > current_sqnum) ||
                                       is_restart ||
                                       is_reconfig;

                        if !is_newer {
                            // Old or duplicate frame - ignore to prevent overwriting newer data
                            // REDUCED LOGGING: Too frequent
                            // log::trace!(
                            //     "Ignoring stale GOOSE frame APPID 0x{:04X} LAN{}: rx(st:{},sq:{}) <= current(st:{},sq:{})",
                            //     appid, lan_id, rx_stnum, rx_sqnum, current_stnum, current_sqnum
                            // );
                            continue;
                        }

                        // Frame is newer - update stored PDU
                        if is_restart {
                            info!(
                                "GOOSE sender RESTART detected APPID 0x{:04X} LAN{}: stNum dropped {} → {} (confRev:{} → {})",
                                appid, lan_id, current_stnum, rx_stnum, current_confrev, rx_confrev
                            );
                        } else if is_reconfig {
                            info!(
                                "GOOSE RECONFIGURATION detected APPID 0x{:04X} LAN{}: confRev changed {} → {} (stNum:{} → {})",
                                appid, lan_id, current_confrev, rx_confrev, current_stnum, rx_stnum
                            );
                        } else {
                            info!(
                                "Received new GOOSE command APPID 0x{:04X} LAN{}: (st:{},sq:{}) > (st:{},sq:{})",
                                appid, lan_id, rx_stnum, rx_sqnum, current_stnum, current_sqnum
                            );
                        }
                        
                        pms_entry.goosepdu = rx_pdu.clone();
                        pms_entry.last_update_time = Some(std::time::SystemTime::now());

                        // Get the list of PCS that should receive this command
                        if let Some(pcs_list) = pms_config.pms_command_pcs_mapping.get(&appid) {
                            // Process command data for each PCS in the list
                            // TODO: Parse allData and update PCS command values
                            info!(
                                "Command for APPID 0x{:04X} affects {} PCS units: {:?}",
                                appid, pcs_list.len(), pcs_list
                            );
                            
                            // Extract command data from GOOSE allData
                            // The allData structure should contain:
                            // - Boolean enable flags (active/reactive power control)
                            // - Float setpoint values (active/reactive power)
                            // This needs to be implemented based on the actual GOOSE data structure

                            rx_pdu.allData.iter().for_each(|data| {
                                // Placeholder: Log data types received
                                info!(
                                    "Received GOOSE allData item of type {:?} for APPID 0x{:04X}",
                                    data., appid
                                );
                                // Actual parsing and PCS command updates go here
                            });         
                            
                        } else {
                            warn!(
                                "No PCS mapping found for APPID 0x{:04X} from LAN{}",
                                appid, lan_id
                            );
                        }
                    } else {
                        warn!(
                            "No PMS command subscriber found for APPID 0x{:04X} from LAN{}",
                            appid, lan_id
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
