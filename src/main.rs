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
use pcs_simulator::pcs::nameplate::load_nameplates_from_csv;
use pcs_simulator::pcs::{load_pcs_type_mappings, init_goose_frame_for_pcs, 
                          GooseFrame, PcsTypeMapping};
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

    let pcs_config = load_nameplates_from_csv(&nameplate_file)?;
    let pcs_type_config = load_pcs_type_mappings(&mapping_file);
    // unwrap pcs_type_config
    let pcs_type_config = match pcs_type_config {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load PCS type mappings: {:?}", e);
            return Err(e);
        }
    };

    // for test 
    // log::info!("Loaded {:? } PCS nameplates", pcs_config);
    // log::info!("Loaded {:? } PCS type mappings", pcs_type_config);

    //iterator pcs_config to initialize goose frames
    for nameplate in pcs_config {
        //get pcs type from nameplate
        let pcs_type = match &nameplate.pcs_type {
            Some(pt) => pt,
            None => {
                warn!("PCS nameplate missing pcs_type, skipping...");
                continue;
            }
        };
        log::info!("Initializing GOOSE frame for PCS Type: {:?}", pcs_type);
        // get PcsTypeMapping from pcs_type_config
        let pcs_type_mapping = match pcs_type_config.get(pcs_type) {
            Some(mapping) => mapping,
            None => {
                warn!("PCS Type: {:?} not found in PCS type mappings, skipping...", pcs_type);
                continue;
            }
        };
        log::info!("Found PCS Type Mapping: {:?}", pcs_type_mapping);
        // initialize goose frame for pcs
        let _frame = init_goose_frame_for_pcs(&nameplate, pcs_type_mapping);
        log::info!("✅ Initialized GOOSE frame for PCS ID: {:?}", nameplate.logical_id);
        log::info!("init frame: {:?}", _frame);
        
    }

    Ok(())
  
}




