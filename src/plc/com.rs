use crate::pcs::process_data::{AppIdIndex, MutablePcsData};
// use crate::pcs::types::PublisherPcsData;
use crate::plc::types::{StPCSDataBytePosInAllDataCfg, StPCSImage, StPCSinfo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// Global atomic counter for lifecounter - thread-safe auto-increment
static LIFECOUNTER: AtomicU64 = AtomicU64::new(0);

/// Reset the lifecounter to a specific value (useful for testing or initialization)
pub fn reset_lifecounter(value: u64) {
    LIFECOUNTER.store(value, Ordering::SeqCst);
}

/// Get the current lifecounter value without incrementing
pub fn get_lifecounter() -> u64 {
    LIFECOUNTER.load(Ordering::SeqCst)
}

// from Arc<Mutex<MutablePcsData>> to fetch stPCSinfo struct fields values
pub fn get_stpcsimage(
    appidindex: &AppIdIndex,
    mutable_data: &MutablePcsData,
    cfg: &HashMap<String, (StPCSDataBytePosInAllDataCfg, Vec<u8>)>,
    qtyofpcs: usize,
) -> StPCSImage {
    let mut image = StPCSImage::default();
    info!("Generating stPCSImage with {} PCS entries", qtyofpcs);
    info!(
        "Mutable data contains {} PCS entries for LAN1 and {} for LAN2",
        mutable_data.pcs_all_lan1.len(),
        mutable_data.pcs_all_lan2.len()
    );
    // REMOVED: Logging entire mutable_data causes deadlock due to lock contention
    // info!("Mutable data is:{:?}", mutable_data);

    // Collect logical_ids first to minimize lock hold time
    let lan1_logical_ids: Vec<u16> = mutable_data
        .pcs_all_lan1
        .iter()
        .map(|entry| *entry.key())
        .collect();

    // Pre-allocate vector capacity for better performance (avoid reallocation)
    image.pcs_data_networka.reserve(lan1_logical_ids.len());

    // Process LAN1 entries with minimal lock hold time
    for logical_id in lan1_logical_ids {
        if let Some(pcs_ref) = mutable_data.pcs_all_lan1.get(&logical_id) {
            // Create struct directly in place - no reuse needed
            let mut info_lan1 = StPCSinfo::default();
            info_lan1.logical_id = logical_id;

            let pcstype = appidindex
                .get_type_from_logical_id_lan1(info_lan1.logical_id)
                .unwrap_or_default();

            // Pass reference, not holding lock during entire get_info call
            let _getinfo = info_lan1.get_info(
                1,
                info_lan1.logical_id,
                pcstype,
                cfg,
                &*pcs_ref, // Dereference to get &PublisherPcsData
            );
            // Lock is released here when pcs_ref goes out of scope

            // Push directly without clone - struct is moved into vector
            image.pcs_data_networka.push(info_lan1);
        }
    }

    // Collect LAN2 logical_ids
    let lan2_logical_ids: Vec<u16> = mutable_data
        .pcs_all_lan2
        .iter()
        .map(|entry| *entry.key())
        .collect();

    // Pre-allocate vector capacity for better performance (avoid reallocation)
    image.pcs_data_networkb.reserve(lan2_logical_ids.len());

    // Process LAN2 entries with minimal lock hold time
    for logical_id in lan2_logical_ids {
        if let Some(pcs_ref) = mutable_data.pcs_all_lan2.get(&logical_id) {
            // Create struct directly in place - no reuse needed
            let mut info_lan2 = StPCSinfo::default();
            info_lan2.logical_id = logical_id;

            let pcstype = appidindex
                .get_type_from_logical_id_lan2(info_lan2.logical_id)
                .unwrap_or_default();

            // Pass reference, not holding lock during entire get_info call
            let _getinfo = info_lan2.get_info(
                2,
                info_lan2.logical_id,
                pcstype,
                cfg,
                &*pcs_ref, // Dereference to get &PublisherPcsData
            );
            // Lock is released here when pcs_ref goes out of scope

            // Push directly without clone - struct is moved into vector
            image.pcs_data_networkb.push(info_lan2);
        }
    }

    image.number_of_pcs = qtyofpcs as u16;

    // Auto-increment lifecounter atomically (thread-safe)
    image.lifecounter = LIFECOUNTER.fetch_add(1, Ordering::SeqCst);

    image
}

//todo: using socket2 to call udp socket functions send pcs image to plc

use log::info;
use socket2::Socket;
// use std::net::SocketAddr;
use std::io;

/// Serialize stPCSImage to bytes for UDP transmission
///
/// Binary format:
/// - protocol (1 byte)
/// - number_of_pcs (2 bytes)
/// - lifecounter (8 bytes)
/// - spare (16 bytes)
/// - pcs_data_networkA + data
/// - pcs_data_networkB + data
fn serialize_stpcsimage(image: &StPCSImage) -> Vec<u8> {
    let mut buffer = Vec::new();

    // Header: protocol(1) + number_of_pcs(2) + lifecounter(8) + spare(16) = 27 bytes
    buffer.push(image.protocol);
    buffer.extend_from_slice(&image.number_of_pcs.to_le_bytes());
    buffer.extend_from_slice(&image.lifecounter.to_le_bytes());
    buffer.extend_from_slice(&image.spare);

    // Network A data - sort indices only (much cheaper than cloning structs)
    // PERFORMANCE: Sorting indices (8 bytes × N) vs cloning structs (49 bytes × N)
    // For 40 PCS: 320 bytes vs 1,960 bytes = 84% reduction
    let mut indices_a: Vec<usize> = (0..image.pcs_data_networka.len()).collect();
    indices_a.sort_unstable_by_key(|&i| image.pcs_data_networka[i].logical_id);
    for &idx in &indices_a {
        serialize_stpcsinfo(&mut buffer, &image.pcs_data_networka[idx]);
    }

    // Network B data - same optimization
    let mut indices_b: Vec<usize> = (0..image.pcs_data_networkb.len()).collect();
    indices_b.sort_unstable_by_key(|&i| image.pcs_data_networkb[i].logical_id);
    for &idx in &indices_b {
        serialize_stpcsinfo(&mut buffer, &image.pcs_data_networkb[idx]);
    }

    buffer
}

/// Serialize stPCSinfo to bytes
///
/// Binary format (49 bytes per PCS):
/// - logical_id (2 bytes)
/// - is_valid (1 byte)
/// - feed_line_id (1 byte)
/// - is_controllable (1 byte)
/// - pcs_realtime_active_power (4 bytes f32)
/// - pcs_realtime_reactive_power (4 bytes f32)
/// - pcs_maximum_charging_power (4 bytes f32)
/// - pcs_maximum_discharging_power (4 bytes f32)
/// - pcs_maximum_inductive_power (4 bytes f32)
/// - pcs_maximum_capacitive_power (4 bytes f32)
/// - SOC (4 bytes f32)
/// - spare (16 bytes)
fn serialize_stpcsinfo(buffer: &mut Vec<u8>, pcs: &StPCSinfo) {
    buffer.extend_from_slice(&pcs.logical_id.to_le_bytes());
    buffer.push(pcs.is_valid);
    buffer.push(pcs.feed_line_id);
    buffer.push(pcs.is_controllable);
    buffer.extend_from_slice(&pcs.pcs_realtime_active_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_realtime_reactive_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_maximum_charging_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_maximum_discharging_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_maximum_inductive_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_maximum_capacitive_power.to_le_bytes());
    buffer.extend_from_slice(&pcs.pcs_soc.to_le_bytes());
    buffer.extend_from_slice(&pcs.spare);
}

/// Send stPCSImage via UDP using pre-existing socket
///
/// Uses socket created during program initialization.
///
/// # Arguments
/// * `socket` - Reusable socket2::Socket (must be bound)
/// * `image` - Reference to stPCSImage to send
///
/// # Returns
/// * `Ok(usize)` - Number of bytes sent
/// * `Err(io::Error)` - Error if send fails
pub fn send_stpcsimage_udp(socket: &Socket, image: &StPCSImage) -> io::Result<usize> {
    let data = serialize_stpcsimage(image);

    log::debug!(
        "Sending pcs image to tc via reusable UDP socket: lifecounter={}, size={} bytes",
        image.lifecounter,
        data.len()
    );

    socket.send(&data)
}
