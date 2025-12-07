// deprecated file: src/goose/packet_processor.rs
// 2025-10-16 

use crate::goose::buffer_pool::PooledBuffer;
use crate::goose::pdu::decodeGooseFrame;
use crate::pcs::types::SubscriberPCSData;
use std::sync::{Arc, Mutex};
use log::{info, warn};

/// Packet data container using pooled buffer (zero-allocation)
pub struct PacketData {
    pub data: PooledBuffer,
}

pub fn process_rx_packet(pcs_data_pool: Arc<Mutex<std::collections::HashMap<u16, SubscriberPCSData>>>, (lan_id, packet): (u16, PacketData)) {
    // Replace with your actual processing logic
    let mut rx_header = Default::default();
    let mut rx_pdu = Default::default();
    let result = decodeGooseFrame(&mut rx_header, &mut rx_pdu, &packet.data, 0);
    if result.is_ok() {
        info!("=======lan id is :{}=======================================", lan_id);
        info!("decode header {:?}", rx_header);
        info!("decode PDU {:?}", rx_pdu);
        info!("===============================");
        let appid = u16::from_be_bytes(rx_header.APPID);
        let mut pcs_data_map = match pcs_data_pool.lock() {
            Ok(map) => map,
            Err(poisoned) => {
                warn!("Mutex was poisoned, recovering data: {}", poisoned);
                poisoned.into_inner()
            }
        };
        for (_pcs_id, pcs) in pcs_data_map.iter_mut() {
            if let Some(nameplate_appid) = pcs.nameplate_appid() {
                if nameplate_appid == appid {
                    pcs.update_from_goose(&rx_pdu, lan_id);
                    info!("Matched PCS ID: {}, Updated with GOOSE data", pcs.pcs_id());
                }
            }
        }   
    }


}
