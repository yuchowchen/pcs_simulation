// deprecated file: src/goose/packet_processor.rs
// 2025-10-16 

use crate::goose::buffer_pool::PooledBuffer;
// use crate::goose::pdu::decodeGooseFrame;
// use crate::pcs::types::PublisherPcsData;
// use std::sync::{Arc, Mutex};
// use log::{info, warn};

/// Packet data container using pooled buffer (zero-allocation)
pub struct PacketData {
    pub data: PooledBuffer,
}


