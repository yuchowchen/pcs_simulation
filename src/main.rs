// PCS Simulator Main Entry Point - Enhanced with Command Processing
// 
// This implements README.md requirement #5:
// "Each PCS will decode the received GOOSE commands and update the 
// active power and reactive power feedback according to setpoints accordingly"

use anyhow::Result;
use crossbeam_channel::{bounded};
use pcs_simulator::goose::buffer_pool::BufferPool;
use pcs_simulator::goose::packet_processor::PacketData;
use pcs_simulator::network::setup_network_channels;
use pcs_simulator::pcs::ProcessData;
use pcs_simulator::threads::*;
use pcs_simulator::threads::worker::spawn_worker_threads_enhanced;
use log::{error, info};

use std::sync::Arc;
fn main() {
    if let Err(e) = run() {
        error!("Application error: {:?}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    info!("========================================");
    info!("PCS Simulator Starting (Enhanced)");
    info!("========================================");

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


    // Initialize PCS data
    let nameplate_file = format!("{}pcs.csv", config_path);
    let mut process_data = ProcessData::default();
    process_data.init_from_nameplates(&nameplate_file)?;
    let (appid_index, mutable_data) = process_data.into_components();
    let mutable_data_shared = Arc::new(mutable_data);
    let appid_index_shared = Arc::new(appid_index);

    // Setup network
    let mut network_channels = setup_network_channels(&config_paras.1)?;
    let (packet_tx, packet_rx) = bounded(16384);

    // Spawn threads with enhanced workers
    let num_workers = num_cpus::get();
    let validity_interval_ms = config_paras.2;
    
    let validity_handle = spawn_validity_thread(Arc::clone(&mutable_data_shared), validity_interval_ms);

    // Use enhanced worker threads
    info!("Starting enhanced worker threads with command extraction");
    let worker_handles = spawn_worker_threads_enhanced(
        packet_rx,
        Arc::clone(&appid_index_shared),
        Arc::clone(&mutable_data_shared),
        num_workers,
    );

    // Network reception
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

    let mut handles = vec![validity_handle];
    handles.extend(worker_handles);
    if let Some(h) = lan1_handle { handles.push(h); }
    if let Some(h) = lan2_handle { handles.push(h); }

    info!("✅ PCS Simulator Running (Enhanced) - {} threads", handles.len());
    std::thread::park();

    info!("Shutdown initiated");
    drop(packet_tx);
    for handle in handles { let _ = handle.join(); }

    info!("✅ PCS Simulator Stopped");
    Ok(())
}
