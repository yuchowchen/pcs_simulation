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
use log::{error, info, warn};
use pnet_datalink::DataLinkSender;
use std::collections::HashMap;
use std::sync::Arc;

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

    // Step 6: Spawn worker threads for incoming GOOSE processing
    let num_workers = num_cpus::get();
    info!("Step 5: Spawning {} worker threads", num_workers);
    let worker_handles = spawn_worker_threads_enhanced(
        packet_rx,
        Arc::clone(&appid_index),
        Arc::clone(&mutable_data),
        num_workers,
    );
    info!("✅ Worker threads spawned");

    // Step 7: Spawn validity checker thread
    info!("Step 6: Spawning validity checker thread");
    let validity_handle = spawn_validity_thread(
        Arc::clone(&mutable_data),
        config_paras.2, // validity interval
    );
    info!("✅ Validity checker spawned");

    // Step 8: Setup GOOSE receivers for both LANs
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

    // Step 9: Spawn GOOSE publisher thread
    info!("Step 8: Starting GOOSE publisher thread");
    let publisher_handle = spawn_publisher_thread(
        goose_frames,
        pcs_type_mappings,
        Arc::clone(&mutable_data),
        network_channels.tx_lan1,
        network_channels.tx_lan2,
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

/// Initialize GOOSE frames for all PCS units in both LANs
fn initialize_goose_frames(
    mutable_data: &Arc<pcs_simulator::pcs::MutablePcsData>,
    pcs_type_mappings: &HashMap<String, PcsTypeMapping>,
) -> Result<HashMap<(u8, u16), GooseFrame>> {
    let mut frames = HashMap::new();
    
    // Initialize frames for LAN1
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
                    frames.insert((1, logical_id), frame);
                }
                Err(e) => {
                    warn!("Failed to initialize frame for LAN1 PCS {}: {}", logical_id, e);
                }
            }
        }
    }
    
    // Initialize frames for LAN2
    for entry in mutable_data.pcs_all_lan2.iter() {
        let logical_id = *entry.key();
        let pcs_data = entry.value();
        
        if let Some(nameplate) = pcs_data.get_nameplate() {
            let pcs_type = nameplate.pcs_type.as_ref()
                .ok_or_else(|| anyhow::anyhow!("PCS logical_id {} missing pcs_type", logical_id))?;
            
            let type_mapping = pcs_type_mappings.get(pcs_type)
                .ok_or_else(|| anyhow::anyhow!("No mapping found for PCS type: {}", pcs_type))?;
            
            match init_goose_frame_for_pcs(nameplate, type_mapping) {
                Ok(frame) => {
                    frames.insert((2, logical_id), frame);
                }
                Err(e) => {
                    warn!("Failed to initialize frame for LAN2 PCS {}: {}", logical_id, e);
                }
            }
        }
    }
    
    Ok(frames)
}

/// Spawn publisher thread that periodically sends GOOSE frames
fn spawn_publisher_thread(
    mut goose_frames: HashMap<(u8, u16), GooseFrame>,
    pcs_type_mappings: HashMap<String, PcsTypeMapping>,
    mutable_data: Arc<pcs_simulator::pcs::MutablePcsData>,
    mut tx_lan1: Option<Box<dyn DataLinkSender>>,
    mut tx_lan2: Option<Box<dyn DataLinkSender>>,
) -> std::thread::JoinHandle<()> {
    use std::time::Duration;
    
    std::thread::spawn(move || {
        info!("GOOSE publisher thread started");
        let publish_interval = Duration::from_millis(100); // 10 Hz publishing rate
        let mut buffer = vec![0u8; 1600]; // Max ethernet frame size
        
        loop {
            std::thread::sleep(publish_interval);
            
            // Update and send LAN1 frames
            for entry in mutable_data.pcs_all_lan1.iter() {
                let logical_id = *entry.key();
                let pcs_data = entry.value();
                
                if let Some(frame) = goose_frames.get_mut(&(1, logical_id)) {
                    if let Some(nameplate) = pcs_data.get_nameplate() {
                        if let Some(pcs_type) = &nameplate.pcs_type {
                            if let Some(type_mapping) = pcs_type_mappings.get(pcs_type) {
                                // Update frame with current PCS data
                                if let Err(e) = update_goose_frame_data(frame, &pcs_data, type_mapping) {
                                    warn!("Failed to update LAN1 PCS {} frame: {}", logical_id, e);
                                    continue;
                                }
                                
                                // Encode GOOSE frame
                                buffer.fill(0);
                                let mut header_copy = frame.0.clone();
                                let frame_len = encodeGooseFrame(&mut header_copy, &frame.1, &mut buffer, 0);
                                
                                // Send on LAN1
                                if let Some(ref mut tx) = tx_lan1 {
                                    match tx.build_and_send(1, frame_len, &mut |packet| {
                                        packet[..frame_len].copy_from_slice(&buffer[..frame_len]);
                                    }) {
                                        Some(Ok(())) => { /* Sent successfully */ }
                                        Some(Err(e)) => {
                                            warn!("Failed to send LAN1 PCS {} frame: {}", logical_id, e);
                                        }
                                        None => {
                                            warn!("Failed to build LAN1 PCS {} frame", logical_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Update and send LAN2 frames
            for entry in mutable_data.pcs_all_lan2.iter() {
                let logical_id = *entry.key();
                let pcs_data = entry.value();
                
                if let Some(frame) = goose_frames.get_mut(&(2, logical_id)) {
                    if let Some(nameplate) = pcs_data.get_nameplate() {
                        if let Some(pcs_type) = &nameplate.pcs_type {
                            if let Some(type_mapping) = pcs_type_mappings.get(pcs_type) {
                                // Update frame with current PCS data
                                if let Err(e) = update_goose_frame_data(frame, &pcs_data, type_mapping) {
                                    warn!("Failed to update LAN2 PCS {} frame: {}", logical_id, e);
                                    continue;
                                }
                                
                                // Encode GOOSE frame
                                buffer.fill(0);
                                let mut header_copy = frame.0.clone();
                                let frame_len = encodeGooseFrame(&mut header_copy, &frame.1, &mut buffer, 0);
                                
                                // Send on LAN2
                                if let Some(ref mut tx) = tx_lan2 {
                                    match tx.build_and_send(1, frame_len, &mut |packet| {
                                        packet[..frame_len].copy_from_slice(&buffer[..frame_len]);
                                    }) {
                                        Some(Ok(())) => { /* Sent successfully */ }
                                        Some(Err(e)) => {
                                            warn!("Failed to send LAN2 PCS {} frame: {}", logical_id, e);
                                        }
                                        None => {
                                            warn!("Failed to build LAN2 PCS {} frame", logical_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
