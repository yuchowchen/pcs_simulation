// PCS Simulator Main Entry Point - GOOSE Publisher
// 
// Flow:
// 1. Initialize PCS data from pcs.csv
// 2. Load PCS type mappings from JSON
// 3. Initialize GOOSE frames for each PCS
// 4. Update frames with real-time data
// 5. Send frames to both LANs

use anyhow::Result;
use crossbeam_channel::{bounded};
use pcs_simulator::goose::buffer_pool::BufferPool;
use pcs_simulator::goose::packet_processor::PacketData;
use pcs_simulator::goose::pdu::encodeGooseFrame;
use pcs_simulator::network::setup_network_channels;
use pcs_simulator::pcs::{ProcessData, load_pcs_type_mappings, init_goose_frame_for_pcs, 
                         update_goose_frame_data, GooseFrame, PcsTypeMapping};
use pcs_simulator::threads::worker::spawn_worker_threads_enhanced;
use pcs_simulator::threads::validity::spawn_validity_thread;
use pcs_simulator::threads::RetransmitSignal;
use log::{error, info, warn};
use pnet_datalink::DataLinkSender;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

fn main() {
    if let Err(e) = run() {
        error!("Application error: {:?}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    info!("========================================");
    info!("PCS Simulator Starting - GOOSE Publisher");
    info!("========================================");

    // Step 1: Load configuration
    let config_paras = match pcs_simulator::os::start::start() {
        Ok(params) => {
            println!("✅ Application startup configuration successful");
            params
        }
        Err(e) => {
            eprintln!("ERROR: Failed to initialize application: {}", e);
            panic!("Cannot continue");
        }
    };

    let config_path = &config_paras.0;
    let nameplate_file = format!("{}pcs.csv", config_path);
    let mapping_file = format!("{}PCS_publisher_alldata_mapping.json", config_path);

    // Step 2: Initialize PCS data from pcs.csv
    info!("Step 1: Initializing PCS data from {}", nameplate_file);
    let mut process_data = ProcessData::default();
    process_data.init_from_nameplates(&nameplate_file)?;
    
    let (appid_index, mutable_data) = process_data.into_components();
    let appid_index = Arc::new(appid_index);
    let mutable_data = Arc::new(mutable_data);
    
    info!("✅ Initialized {} PCS in LAN1, {} PCS in LAN2", 
         mutable_data.pcs_all_lan1.len(),
         mutable_data.pcs_all_lan2.len());

    // Step 3: Load PCS type mappings from JSON
    info!("Step 2: Loading PCS type mappings from {}", mapping_file);
    let pcs_type_mappings = load_pcs_type_mappings(&mapping_file)?;
    info!("✅ Loaded {} PCS type mappings", pcs_type_mappings.len());

    // Step 4: Initialize GOOSE frames for each PCS
    info!("Step 3: Initializing GOOSE frames for each PCS");
    let goose_frames = initialize_goose_frames(&mutable_data, &pcs_type_mappings)?;
    info!("✅ Initialized {} GOOSE frames total", goose_frames.len());

    // Step 5: Setup network channels
    info!("Step 4: Setting up network channels");
    let mut network_channels = setup_network_channels(&config_paras.1)?;
    let (packet_tx, packet_rx) = bounded(16384);
    info!("✅ Network channels ready");

    // Step 6: Create update signal for instant publisher notification
    let update_signal = Arc::new(RetransmitSignal::new());
    
    // Step 7: Spawn worker threads for incoming GOOSE processing
    let num_workers = num_cpus::get();
    info!("Step 5: Spawning {} worker threads", num_workers);
    let worker_handles = spawn_worker_threads_enhanced(
        packet_rx,
        Arc::clone(&appid_index),
        Arc::clone(&mutable_data),
        num_workers,
        Some(Arc::clone(&update_signal)),
    );
    info!("✅ Worker threads spawned");

    // Step 8: Spawn validity checker thread
    info!("Step 6: Spawning validity checker thread");
    let validity_handle = spawn_validity_thread(
        Arc::clone(&mutable_data),
        config_paras.2, // validity interval
    );
    info!("✅ Validity checker spawned");

    // Step 9: Setup GOOSE receivers for both LANs
    info!("Step 7: Starting GOOSE receivers");
    let buffer_pool = Arc::new(BufferPool::new(8192));
    
    #[inline(always)]
    fn is_goose_packet(packet: &[u8]) -> bool {
        if packet.len() < 14 { return false; }
        (packet.len() >= 14 && packet[12] == 0x88 && packet[13] == 0xb8) ||
        (packet.len() >= 18 && packet[12] == 0x81 && packet[13] == 0x00 
           && packet[16] == 0x88 && packet[17] == 0xb8)
    }

    // LAN1 receiver
    let lan1_handle = network_channels.rx_lan1.take().map(|mut rx_lan1| {
        let packet_tx_lan1 = packet_tx.clone();
        let buffer_pool_lan1 = Arc::clone(&buffer_pool);
        std::thread::spawn(move || {
            info!("GOOSE LAN1 receiver started");
            while let Ok(packet) = rx_lan1.next() {
                if !is_goose_packet(&packet) { continue; }
                if let Some(mut pooled_buffer) = buffer_pool_lan1.acquire() {
                    pooled_buffer.copy_from_slice(&packet);
                    let _ = packet_tx_lan1.try_send((1, PacketData { data: pooled_buffer }));
                }
            }
        })
    });

    // LAN2 receiver
    let lan2_handle = network_channels.rx_lan2.take().map(|mut rx_lan2| {
        let packet_tx_lan2 = packet_tx.clone();
        let buffer_pool_lan2 = Arc::clone(&buffer_pool);
        std::thread::spawn(move || {
            info!("GOOSE LAN2 receiver started");
            while let Ok(packet) = rx_lan2.next() {
                if !is_goose_packet(&packet) { continue; }
                if let Some(mut pooled_buffer) = buffer_pool_lan2.acquire() {
                    pooled_buffer.copy_from_slice(&packet);
                    let _ = packet_tx_lan2.try_send((2, PacketData { data: pooled_buffer }));
                }
            }
        })
    });

    // Step 10: Spawn GOOSE publisher thread with update signal
    info!("Step 8: Starting GOOSE publisher thread");
    let publisher_handle = spawn_publisher_thread(
        goose_frames,
        pcs_type_mappings,
        Arc::clone(&mutable_data),
        network_channels.tx_lan1,
        network_channels.tx_lan2,
        Arc::clone(&update_signal),
    );
    info!("✅ GOOSE publisher started");

    // Collect all thread handles
    let mut handles = vec![validity_handle, publisher_handle];
    handles.extend(worker_handles);
    if let Some(h) = lan1_handle { handles.push(h); }
    if let Some(h) = lan2_handle { handles.push(h); }

    info!("========================================");
    info!("✅ PCS Simulator Running - {} threads", handles.len());
    info!("   - GOOSE Publisher: Active (10 Hz)");
    info!("   - GOOSE Receivers: LAN1 + LAN2");
    info!("   - Worker Threads: {}", num_workers);
    info!("   - Validity Checker: Active");
    info!("========================================");
    
    // Park main thread (wait for Ctrl+C or signal)
    std::thread::park();

    info!("Shutdown initiated");
    drop(packet_tx);
    for handle in handles { let _ = handle.join(); }

    info!("✅ PCS Simulator Stopped");
    Ok(())
}

/// Frame metadata for tracking retransmit timing
#[derive(Debug, Clone)]
struct FrameMetadata {
    frame: GooseFrame,
    last_update: Option<SystemTime>,  // When PCS data was last updated
    last_send: Option<Instant>,        // When frame was last sent (for timing)
    current_interval_ms: u64,
}

impl FrameMetadata {
    fn new(frame: GooseFrame) -> Self {
        Self {
            frame,
            last_update: None,
            last_send: None,
            current_interval_ms: 2, // Start with 2ms
        }
    }
}

/// Initialize GOOSE frames for all PCS units (unified for both LANs)
fn initialize_goose_frames(
    mutable_data: &Arc<pcs_simulator::pcs::MutablePcsData>,
    pcs_type_mappings: &HashMap<String, PcsTypeMapping>,
) -> Result<HashMap<u16, FrameMetadata>> {
    let mut frames = HashMap::new();
    
    // Initialize frames from LAN1 PCS data (will be sent to both LANs)
    for entry in mutable_data.pcs_all_lan1.iter() {
        let logical_id = *entry.key();
        let pcs_data = entry.value();
        
        if let Some(nameplate) = pcs_data.get_nameplate() {
            let pcs_type = nameplate.pcs_type.as_ref()
                .ok_or_else(|| anyhow::anyhow!("PCS logical_id {} missing pcs_type", logical_id))?;
            
            let type_mapping = pcs_type_mappings.get(pcs_type)
                .ok_or_else(|| anyhow::anyhow!("No mapping found for PCS type: {}", pcs_type))?;
            
            match init_goose_frame_for_pcs(nameplate, type_mapping) {
                Ok(frame) => {
                    frames.insert(logical_id, FrameMetadata::new(frame));
                    info!("Initialized unified GOOSE frame for PCS logical_id {}", logical_id);
                }
                Err(e) => {
                    warn!("Failed to initialize frame for PCS {}: {}", logical_id, e);
                }
            }
        }
    }
    
    // Note: LAN2 PCS should have same logical_ids as LAN1 (same devices on redundant network)
    // If LAN2 has different PCS units, add them here
    for entry in mutable_data.pcs_all_lan2.iter() {
        let logical_id = *entry.key();
        
        // Skip if already initialized from LAN1
        if frames.contains_key(&logical_id) {
            continue;
        }
        
        let pcs_data = entry.value();
        
        if let Some(nameplate) = pcs_data.get_nameplate() {
            let pcs_type = nameplate.pcs_type.as_ref()
                .ok_or_else(|| anyhow::anyhow!("PCS logical_id {} missing pcs_type", logical_id))?;
            
            let type_mapping = pcs_type_mappings.get(pcs_type)
                .ok_or_else(|| anyhow::anyhow!("No mapping found for PCS type: {}", pcs_type))?;
            
            match init_goose_frame_for_pcs(nameplate, type_mapping) {
                Ok(frame) => {
                    frames.insert(logical_id, FrameMetadata::new(frame));
                    info!("Initialized unified GOOSE frame for PCS logical_id {} (LAN2 only)", logical_id);
                }
                Err(e) => {
                    warn!("Failed to initialize frame for PCS {}: {}", logical_id, e);
                }
            }
        }
    }
    
    Ok(frames)
}

/// Spawn publisher thread with retransmit logic: 2ms, 4ms, 8ms, ... up to 5000ms
/// Uses high-precision Condvar notification for instant wakeup on data updates
fn spawn_publisher_thread(
    mut goose_frames: HashMap<u16, FrameMetadata>,
    pcs_type_mappings: HashMap<String, PcsTypeMapping>,
    mutable_data: Arc<pcs_simulator::pcs::MutablePcsData>,
    mut tx_lan1: Option<Box<dyn DataLinkSender>>,
    mut tx_lan2: Option<Box<dyn DataLinkSender>>,
    update_signal: Arc<RetransmitSignal>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        info!("GOOSE publisher thread started with retransmit logic (Condvar-based)");
        const MIN_INTERVAL_MS: u64 = 2;
        const MAX_INTERVAL_MS: u64 = 5000;
        let mut buffer = vec![0u8; 1600]; // Max ethernet frame size
        
        loop {
            let loop_start = Instant::now();
            let mut min_wait_time = Duration::from_millis(MAX_INTERVAL_MS);
            let now = Instant::now();
            
            // Process all frames and determine next wakeup time
            for (logical_id, metadata) in goose_frames.iter_mut() {
                // Check if data was updated (by comparing with PCS last_update)
                let mut data_was_updated = false;
                
                // Check LAN1 for updates
                if let Some(pcs_entry) = mutable_data.pcs_all_lan1.get(logical_id) {
                    let pcs_data = pcs_entry.value();
                    if let Some(pcs_last_update) = pcs_data.get_last_update() {
                        // Check if PCS was updated since our last frame update
                        if metadata.last_update.is_none() || 
                           pcs_last_update > metadata.last_update.unwrap() {
                            data_was_updated = true;
                        }
                    }
                }
                
                // Reset retransmit interval if data was updated
                if data_was_updated {
                    metadata.current_interval_ms = MIN_INTERVAL_MS;
                    metadata.last_update = Some(SystemTime::now());
                    info!("PCS {} data updated - reset retransmit interval to {}ms", 
                          logical_id, MIN_INTERVAL_MS);
                }
                
                // Check if it's time to send this frame
                let time_since_last_send = metadata.last_send
                    .map(|t| now.duration_since(t))
                    .unwrap_or(Duration::from_millis(metadata.current_interval_ms));
                
                let interval_duration = Duration::from_millis(metadata.current_interval_ms);
                
                if time_since_last_send >= interval_duration {
                    // Time to send this frame
                    
                    // Get PCS data for updating frame (prefer LAN1, fallback to LAN2)
                    let pcs_data_opt = mutable_data.pcs_all_lan1.get(logical_id)
                        .or_else(|| mutable_data.pcs_all_lan2.get(logical_id));
                    
                    if let Some(pcs_entry) = pcs_data_opt {
                        let pcs_data = pcs_entry.value();
                        
                        if let Some(nameplate) = pcs_data.get_nameplate() {
                            if let Some(pcs_type) = &nameplate.pcs_type {
                                if let Some(type_mapping) = pcs_type_mappings.get(pcs_type) {
                                    // Update frame with current PCS data
                                    if let Err(e) = update_goose_frame_data(&mut metadata.frame, &pcs_data, type_mapping) {
                                        warn!("Failed to update PCS {} frame: {}", logical_id, e);
                                        continue;
                                    }
                                    
                                    // Update sequence numbers
                                    if data_was_updated {
                                        metadata.frame.1.stNum = metadata.frame.1.stNum.wrapping_add(1);
                                        metadata.frame.1.sqNum = 0;
                                        info!("PCS {} stNum incremented to {}, sqNum reset to 0", 
                                              logical_id, metadata.frame.1.stNum);
                                    } else {
                                        metadata.frame.1.sqNum = metadata.frame.1.sqNum.wrapping_add(1);
                                    }
                                    
                                    // Encode GOOSE frame once
                                    buffer.fill(0);
                                    let mut header_copy = metadata.frame.0.clone();
                                    let frame_len = encodeGooseFrame(&mut header_copy, &metadata.frame.1, &mut buffer, 0);
                                    
                                    // Send same frame to both LAN1 and LAN2
                                    let mut sent_count = 0;
                                    
                                    if let Some(ref mut tx) = tx_lan1 {
                                        match tx.build_and_send(1, frame_len, &mut |packet| {
                                            packet[..frame_len].copy_from_slice(&buffer[..frame_len]);
                                        }) {
                                            Some(Ok(())) => { sent_count += 1; }
                                            Some(Err(e)) => {
                                                warn!("Failed to send PCS {} frame to LAN1: {}", logical_id, e);
                                            }
                                            None => {
                                                warn!("Failed to build PCS {} frame for LAN1", logical_id);
                                            }
                                        }
                                    }
                                    
                                    if let Some(ref mut tx) = tx_lan2 {
                                        match tx.build_and_send(1, frame_len, &mut |packet| {
                                            packet[..frame_len].copy_from_slice(&buffer[..frame_len]);
                                        }) {
                                            Some(Ok(())) => { sent_count += 1; }
                                            Some(Err(e)) => {
                                                warn!("Failed to send PCS {} frame to LAN2: {}", logical_id, e);
                                            }
                                            None => {
                                                warn!("Failed to build PCS {} frame for LAN2", logical_id);
                                            }
                                        }
                                    }
                                    
                                    if sent_count > 0 {
                                        info!("PCS {} frame sent to {} LAN(s) (stNum: {}, sqNum: {}, interval: {}ms)",
                                              logical_id, sent_count, metadata.frame.1.stNum, 
                                              metadata.frame.1.sqNum, metadata.current_interval_ms);
                                    }
                                    
                                    // Update send timestamp
                                    metadata.last_send = Some(now);
                                    
                                    // Double interval for next retransmit (if no new data)
                                    if !data_was_updated && metadata.current_interval_ms < MAX_INTERVAL_MS {
                                        metadata.current_interval_ms = (metadata.current_interval_ms * 2).min(MAX_INTERVAL_MS);
                                    }
                                }
                            }
                        }
                    }
                    
                    // Next send time is one interval from now
                    min_wait_time = min_wait_time.min(Duration::from_millis(metadata.current_interval_ms));
                } else {
                    // Not time to send yet, calculate remaining time
                    let remaining = interval_duration - time_since_last_send;
                    min_wait_time = min_wait_time.min(remaining);
                }
            }
            
            // High-precision wait with Condvar for instant wakeup
            // Returns true if new data arrived (signal_reset called), false if timeout
            let data_updated = update_signal.wait_timeout(min_wait_time);
            
            let actual_elapsed = loop_start.elapsed();
            
            if data_updated {
                info!("New PCS data received after {:?}, instant wakeup for processing", actual_elapsed);
            } else if actual_elapsed > min_wait_time + Duration::from_millis(1) {
                warn!("⏱️  Timing variance: target {:?}, actual {:?} (+{}µs)",
                      min_wait_time, actual_elapsed,
                      actual_elapsed.as_micros() as i64 - min_wait_time.as_micros() as i64);
            }
        }
    })
}
