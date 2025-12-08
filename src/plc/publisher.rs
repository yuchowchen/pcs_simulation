//(ethher_head,goose_pdu) type definition for goose_publisher
use anyhow::{Context, Result};
use log::{error, info};

type PlcPublisherGooseFrame = (EthernetHeader, IECGoosePdu);

// use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::{Arc, RwLock};
// use std::thread;
// use std::time::{Duration, Instant};

// receiving data from PLC and pubish to GOOSE publisher
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StPlcCmdPub {
    pub pcs_logical_id: u16,
    pub protocol: u8,
    pub pcs_active_power: f32,
    pub pcs_reactive_power: f32,
    pub spare: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct StPlcCmdAll {
    pub protocol: u8,
    pub nanotimer: u64, // timer in nanoseconds since epoch if send timer diff needed to check .
    pub number_of_pcs: u16,
    pub spare: [u8; 16],
    pub pcs_cmds: Vec<StPlcCmdPub>,
}

impl Default for StPlcCmdPub {
    fn default() -> Self {
        StPlcCmdPub {
            pcs_logical_id: 0,
            protocol: 0,
            pcs_active_power: 0.0,
            pcs_reactive_power: 0.0,
            spare: [0; 16],
        }
    }
}

impl Default for StPlcCmdAll {
    fn default() -> Self {
        StPlcCmdAll {
            protocol: 20,
            nanotimer: 0,
            number_of_pcs: 0,
            spare: [0; 16],
            pcs_cmds: Vec::new(),
        }
    }
}

use std::io;

use crate::goose::pdu::getTimeMs;
use crate::goose::types::{EthernetHeader, IECData, IECGoosePdu};

/// Fast deserialization of UDP datagram to StPlcCmdAll
///
/// Assumes data is correctly formatted in little-endian:
/// - protocol (1 byte)
/// - nanotimer (8 bytes)
/// - number_of_pcs (2 bytes)
/// - spare (16 bytes)
/// - pcs_cmds count (2 bytes)
/// - pcs_cmds data (each StPlcCmdPub is 27 bytes)
///
/// # Arguments
/// * `data` - Raw UDP datagram bytes
///
/// # Returns
/// * `Ok(StPlcCmdAll)` - Deserialized command structure
/// * `Err(io::Error)` - If data is too short or malformed
///
/// # Example
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// # let datagram = vec![0u8; 100];
/// # use pcs_simulator:plc::publisher::deserialize_stplccmdall;
/// let cmd = deserialize_stplccmdall(&datagram)?;
/// println!("Received {} PCS commands", cmd.number_of_pcs);
/// # Ok(())
/// # }
/// ```
pub fn deserialize_stplccmdall(data: &[u8]) -> io::Result<StPlcCmdAll> {
    const HEADER_SIZE: usize = 1 + 8 + 2 + 16; // protocol + nanotimer + number_of_pcs + spare
    const PCS_CMD_SIZE: usize = 1 + 2 + 4 + 4 + 16; // StPlcCmdPub size = 27 bytes

    if data.len() < HEADER_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Data too short: {} bytes, need at least {}",
                data.len(),
                HEADER_SIZE
            ),
        ));
    }

    let mut offset = 0;

    // Parse header - bounds check first
    if data.len() < offset + 1 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Data too short to read protocol field",
        ));
    }
    let protocol = data[offset];
    offset += 1;

    // Validate protocol
    if protocol != 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid protocol: expected 20, got {}", protocol),
        ));
    }

    // Bounds check for nanotimer (8 bytes)
    if data.len() < offset + 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Data too short to read nanotimer field",
        ));
    }
    let nanotimer = u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ]);
    offset += 8;

    // Bounds check for number_of_pcs (2 bytes)
    if data.len() < offset + 2 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Data too short to read number_of_pcs field",
        ));
    }
    let number_of_pcs = u16::from_le_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    // Bounds check for spare (16 bytes)
    if data.len() < offset + 16 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Data too short to read spare field",
        ));
    }
    let mut spare = [0u8; 16];
    spare.copy_from_slice(&data[offset..offset + 16]);
    offset += 16;

    // let pcs_count = u16::from_le_bytes([data[offset], data[offset + 1]]);
    // offset += 2;

    // Validate size
    let expected_size = HEADER_SIZE + (number_of_pcs as usize * PCS_CMD_SIZE);
    if data.len() < expected_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Data too short: {} bytes, expected {} for {} PCS commands",
                data.len(),
                expected_size,
                number_of_pcs
            ),
        ));
    }

    // Parse PCS commands
    let mut pcs_cmds = Vec::with_capacity(number_of_pcs as usize);
    for i in 0..number_of_pcs {
        // Bounds check for each PCS command (27 bytes total)
        if data.len() < offset + PCS_CMD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Data too short to read PCS command {} at offset {}", i, offset),
            ));
        }
        
        let pcs_logical_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;

        let cmd_protocol = data[offset];
        offset += 1;

        let pcs_active_power = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let pcs_reactive_power = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let mut cmd_spare = [0u8; 16];
        cmd_spare.copy_from_slice(&data[offset..offset + 16]);
        offset += 16;

        pcs_cmds.push(StPlcCmdPub {
            pcs_logical_id,
            protocol: cmd_protocol,
            pcs_active_power,
            pcs_reactive_power,
            spare: cmd_spare,
        });
    }
    // info!("Parsed PCS command is: {:?}", pcs_cmds);
    Ok(StPlcCmdAll {
        protocol,
        nanotimer,
        number_of_pcs,
        spare,
        pcs_cmds,
    })
}

/// Ultra-fast unsafe deserialization (zero validation, direct memory cast)
///
/// **WARNING**: Only use if you 100% trust the data source!
/// No validation, assumes perfect alignment and format.
///
/// # Safety
/// Caller must ensure:
/// - Data is correctly formatted
/// - Proper alignment
/// - Valid length
///
/// # Arguments
/// * `data` - Raw UDP datagram bytes
///
/// # Returns
/// * `Ok(StPlcCmdAll)` - Deserialized command
/// * `Err(io::Error)` - If data is impossibly short
pub unsafe fn deserialize_stplccmdall_unsafe(data: &[u8]) -> io::Result<StPlcCmdAll> {
    const HEADER_SIZE: usize = 29; // 1 + 8 + 2 + 16 + 2

    if data.len() < HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Data too short"));
    }

    let protocol = *data.get_unchecked(0);

    // Validate protocol
    if protocol != 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid protocol: expected 20, got {}", protocol),
        ));
    }

    let nanotimer = u64::from_le_bytes([
        *data.get_unchecked(1),
        *data.get_unchecked(2),
        *data.get_unchecked(3),
        *data.get_unchecked(4),
        *data.get_unchecked(5),
        *data.get_unchecked(6),
        *data.get_unchecked(7),
        *data.get_unchecked(8),
    ]);

    let number_of_pcs = u16::from_le_bytes([*data.get_unchecked(9), *data.get_unchecked(10)]);

    let mut spare = [0u8; 16];
    std::ptr::copy_nonoverlapping(data.as_ptr().add(11), spare.as_mut_ptr(), 16);

    let pcs_count = u16::from_le_bytes([*data.get_unchecked(27), *data.get_unchecked(28)]);

    let mut pcs_cmds = Vec::with_capacity(pcs_count as usize);
    let mut offset = HEADER_SIZE;

    for _ in 0..pcs_count {
        let cmd_protocol = *data.get_unchecked(offset);
        let pcs_logical_id = u16::from_le_bytes([
            *data.get_unchecked(offset + 1),
            *data.get_unchecked(offset + 2),
        ]);

        let pcs_active_power = f32::from_le_bytes([
            *data.get_unchecked(offset + 3),
            *data.get_unchecked(offset + 4),
            *data.get_unchecked(offset + 5),
            *data.get_unchecked(offset + 6),
        ]);

        let pcs_reactive_power = f32::from_le_bytes([
            *data.get_unchecked(offset + 7),
            *data.get_unchecked(offset + 8),
            *data.get_unchecked(offset + 9),
            *data.get_unchecked(offset + 10),
        ]);

        let mut cmd_spare = [0u8; 16];
        std::ptr::copy_nonoverlapping(data.as_ptr().add(offset + 11), cmd_spare.as_mut_ptr(), 16);

        pcs_cmds.push(StPlcCmdPub {
            protocol: cmd_protocol,
            pcs_logical_id,
            pcs_active_power,
            pcs_reactive_power,
            spare: cmd_spare,
        });

        offset += 27;
    }

    Ok(StPlcCmdAll {
        protocol,
        nanotimer,
        number_of_pcs,
        spare,
        pcs_cmds,
    })
}

// read goose_publisher_cfg.json to get pcs publisher config
// return vector of struct PublisherConfig
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherConfigRaw {
    #[serde(rename = "srcAddr")]
    pub src_addr: String,
    #[serde(rename = "dstAddr")]
    pub dst_addr: String,
    #[serde(rename = "TPID")]
    pub tpid: String,
    #[serde(rename = "TCI")]
    pub tci: String,
    #[serde(rename = "APPID")]
    pub appid: String,
    #[serde(rename = "gocbRef")]
    pub gocb_ref: String,
    #[serde(rename = "datSet")]
    pub dat_set: String,
    #[serde(rename = "goID")]
    pub goose_id: String,
    #[serde(rename = "simulation")]
    pub simulation: String,
    #[serde(rename = "confRev")]
    pub conf_rev: String,
    #[serde(rename = "ndsCom")]
    pub ndscom: String,
    #[serde(rename = "numberOfPcs")]
    pub number_of_pcs: String,
}

#[derive(Debug, Clone)]
pub struct PublisherConfig {
    pub src_addr: String,
    pub dst_addr: String,
    pub tpid: u16,
    pub tci: u16,
    pub appid: u16,
    pub gocb_ref: String,
    pub dat_set: String,
    pub goose_id: String,
    pub simulation: bool,
    pub conf_rev: u32,
    pub ndscom: bool,
    pub number_of_pcs: u32,
}

impl PublisherConfigRaw {
    pub fn to_runtime(&self) -> Result<PublisherConfig> {
        Ok(PublisherConfig {
            src_addr: self.src_addr.clone(),
            dst_addr: self.dst_addr.clone(),
            tpid: u16::from_str_radix(self.tpid.trim_start_matches("0x"), 16)
                .context("Failed to parse TPID")?,
            tci: u16::from_str_radix(self.tci.trim_start_matches("0x"), 16)
                .context("Failed to parse TCI")?,
            appid: u16::from_str_radix(self.appid.trim_start_matches("0x"), 16)
                .context("Failed to parse APPID")?,
            gocb_ref: self.gocb_ref.clone(),
            dat_set: self.dat_set.clone(),
            goose_id: self.goose_id.clone(),
            simulation: self.simulation == "true",
            conf_rev: self
                .conf_rev
                .parse::<u32>()
                .context("Failed to parse conf_rev")?,
            ndscom: self.ndscom == "true",
            number_of_pcs: self
                .number_of_pcs
                .parse::<u32>()
                .context("Failed to parse number_of_pcs")?,
        })
    }
}

pub fn load_plc_publisher_config(cfg_path: String) -> Result<Vec<PublisherConfig>> {
    info!("Loading publisher config from: {}", cfg_path);

    let file = match File::open(&cfg_path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open publisher config file '{}': {}", cfg_path, e);
            anyhow::bail!("Failed to open publisher config file '{}': {}", cfg_path, e);
        }
    };

    let reader = BufReader::new(file);
    let configs: Vec<PublisherConfigRaw> = match serde_json::from_reader(reader) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!(
                "Failed to parse publisher config JSON from '{}': {}",
                cfg_path, e
            );
            anyhow::bail!("Failed to parse publisher config JSON: {}", e);
        }
    };

    let mut result = Vec::new();
    for (idx, cfg) in configs.into_iter().enumerate() {
        match cfg.to_runtime() {
            Ok(runtime_cfg) => result.push(runtime_cfg),
            Err(e) => {
                error!("Failed to convert config {} to runtime: {}", idx, e);
                anyhow::bail!("Failed to convert config {} to runtime: {}", idx, e);
            }
        }
    }
    Ok(result)
}
// iterate Vector of PublisherConfig to initialize  type PlcPublisherGooseFrame
pub fn init_publisher_goose_frames(
    configs: &[PublisherConfig],
) -> Result<Vec<PlcPublisherGooseFrame>> {
    if configs.is_empty() {
        anyhow::bail!("Publisher configuration is empty");
    }

    let mut frames = Vec::with_capacity(configs.len());

    for (idx, cfg) in configs.iter().enumerate() {
        // Validate and parse source MAC address
        let src_mac = match parse_mac(&cfg.src_addr) {
            Ok(mac) => mac,
            Err(e) => {
                error!(
                    "Config {}: Failed to parse source MAC address '{}': {}",
                    idx, cfg.src_addr, e
                );
                anyhow::bail!(
                    "Config {}: Failed to parse source MAC address '{}': {}",
                    idx,
                    cfg.src_addr,
                    e
                );
            }
        };

        // Validate and parse destination MAC address
        let dst_mac = match parse_mac(&cfg.dst_addr) {
            Ok(mac) => mac,
            Err(e) => {
                error!(
                    "Config {}: Failed to parse destination MAC address '{}': {}",
                    idx, cfg.dst_addr, e
                );
                anyhow::bail!(
                    "Config {}: Failed to parse destination MAC address '{}': {}",
                    idx,
                    cfg.dst_addr,
                    e
                );
            }
        };

        // Validate APPID range (should be non-zero for GOOSE)
        if cfg.appid == 0 {
            anyhow::bail!("Config {}: APPID cannot be 0", idx);
        }

        // Validate number_of_pcs
        if cfg.number_of_pcs == 0 {
            anyhow::bail!(
                "Config {}: number_of_pcs cannot be 0 (goID: {})",
                idx,
                cfg.goose_id
            );
        }

        // Validate that gocbRef, datSet, and goID are not empty
        if cfg.gocb_ref.is_empty() {
            anyhow::bail!("Config {}: gocbRef cannot be empty", idx);
        }
        if cfg.dat_set.is_empty() {
            anyhow::bail!("Config {}: datSet cannot be empty", idx);
        }
        if cfg.goose_id.is_empty() {
            anyhow::bail!("Config {}: goID cannot be empty", idx);
        }

        // Create Ethernet header
        let mut eth_header = EthernetHeader::default();
        eth_header.srcAddr = src_mac;
        eth_header.dstAddr = dst_mac;
        eth_header.TPID = cfg.tpid.to_be_bytes();
        eth_header.TCI = cfg.tci.to_be_bytes();
        eth_header.ehterType = [0x88, 0xB8]; // GOOSE Ethertype
        eth_header.APPID = cfg.appid.to_be_bytes();
        //eth_header.length will be assiged later.

        // Create GOOSE PDU
        let mut goose_pdu = IECGoosePdu::default();
        goose_pdu.gocbRef = cfg.gocb_ref.clone();
        goose_pdu.timeAllowedtoLive = 5000; // Example value
        goose_pdu.datSet = cfg.dat_set.clone();
        goose_pdu.goID = cfg.goose_id.clone();
        goose_pdu.t = [0; 8]; // Placeholder for timestamp
        goose_pdu.stNum = 0;
        goose_pdu.sqNum = 0;
        goose_pdu.simulation = cfg.simulation;
        goose_pdu.confRev = cfg.conf_rev;
        goose_pdu.ndsCom = cfg.ndscom;
        goose_pdu.numDatSetEntries = cfg.number_of_pcs * 4;

        // Initialize allData with proper capacity
        let expected_data_entries = (cfg.number_of_pcs as usize) * 4; // 2 booleans + 2 floats per PCS
        goose_pdu.allData = Vec::with_capacity(expected_data_entries);

        // Add boolean flags for each PCS (P command active, Q command active)
        for _ in 0..cfg.number_of_pcs {
            goose_pdu.allData.push(IECData::boolean(false)); // p command active
            goose_pdu.allData.push(IECData::boolean(false)); // q command active
        }

        // Add float values for each PCS (P command, Q command)
        for _ in 0..cfg.number_of_pcs {
            goose_pdu.allData.push(IECData::float32(0.0)); // p command
            goose_pdu.allData.push(IECData::float32(0.0)); // q command
        }

        frames.push((eth_header, goose_pdu));
    }
    info!(
        "Initialized {} GOOSE publisher frames:{:?}",
        frames.len(),
        frames
    );

    Ok(frames)
}

/// Parse MAC address string into [u8; 6]
/// Supports formats like "01:0C:CD:01:00:01", "01-0C-CD-01-00-01", "010CCD010001"
fn parse_mac(s: &str) -> Result<[u8; 6]> {
    // try split by common separators first
    let parts: Vec<&str> = s.split(|c| c == ':' || c == '-' || c == '.').collect();
    if parts.len() == 6 {
        let mut mac = [0u8; 6];
        for (i, p) in parts.iter().enumerate() {
            if p.len() != 2 {
                error!("MAC part '{}' has wrong length in '{}'", p, s);
                anyhow::bail!("MAC part '{}' has wrong length", p);
            }
            mac[i] = match u8::from_str_radix(p, 16) {
                Ok(byte) => byte,
                Err(e) => {
                    error!("Invalid hex '{}' in MAC address '{}': {}", p, s, e);
                    anyhow::bail!("Invalid hex in '{}': {}", p, e);
                }
            };
        }
        return Ok(mac);
    }

    // otherwise strip everything except hex digits and try parse as 12 hex chars
    let s_hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if s_hex.len() == 12 {
        let mut mac = [0u8; 6];
        for i in 0..6 {
            let byte = &s_hex[2 * i..2 * i + 2];
            mac[i] = match u8::from_str_radix(byte, 16) {
                Ok(b) => b,
                Err(e) => {
                    error!("Invalid hex '{}' in MAC address '{}': {}", byte, s, e);
                    anyhow::bail!("Invalid hex in '{}': {}", byte, e);
                }
            };
        }
        return Ok(mac);
    }

    anyhow::bail!("invalid MAC format: {}", s)
}

//assign  StPlcCmdAll data to PlcPublisherGooseFrame in case of new udp command received
impl StPlcCmdAll {
    /// Assign PLC commands to GOOSE frames and reset sqNum to 0
    /// This should be called when new commands are received from PLC
    /// Internal method to assign data to frames (used by assign_and_send_goose_frames)
    pub fn assign_to_goose_frame(&self, frames: &mut Vec<PlcPublisherGooseFrame>) {
        let t = getTimeMs();
        let mut cmd_position = 0;
        for frame in frames.iter_mut() {
            // Update frame metadata
            frame.1.t = t;
            // NOTE: stNum will be incremented by retransmit thread when new_data_arrived=true
            // Do NOT increment here to avoid double increment (would go from 1→3 instead of 1→2)
            frame.1.sqNum = 0;
            frame.1.allData.clear();

            let entries_count = (frame.1.numDatSetEntries / 4) as usize; // Each PCS has 4 entries (2 booleans + 2 floats)
            // Check if we have enough commands
            if cmd_position + entries_count > self.pcs_cmds.len() {
                log::error!(
                    "Not enough PCS commands: need {} more, have {} total",
                    cmd_position + entries_count,
                    self.pcs_cmds.len()
                );
                break;
            }

            // First pass: Add boolean flags for each PCS command
            for i in 0..entries_count {
                let cmd_index = i + cmd_position;
                // info!(
                //     "Processing PCS command {}: protocol={}",
                //     cmd_index, self.pcs_cmds[cmd_index].protocol
                // );
                let cmd = &self.pcs_cmds[cmd_index].protocol;
                match cmd {
                    10 => {
                        frame.1.allData.push(IECData::boolean(true)); // P command active
                        frame.1.allData.push(IECData::boolean(false)); // Q command inactive
                    }
                    20 => {
                        frame.1.allData.push(IECData::boolean(false)); // P command inactive
                        frame.1.allData.push(IECData::boolean(true)); // Q command active
                    }
                    30 => {
                        frame.1.allData.push(IECData::boolean(true)); // P command active
                        frame.1.allData.push(IECData::boolean(true)); // Q command active
                    }
                    _ => {
                        frame.1.allData.push(IECData::boolean(false)); // P command inactive
                        frame.1.allData.push(IECData::boolean(false)); // Q command inactive
                    }
                }
            }

            // Second pass: Add power values for each PCS command
            for i in 0..entries_count {
                let cmd = &self.pcs_cmds[i + cmd_position];
                frame.1.allData.push(IECData::float32(cmd.pcs_active_power));
                frame
                    .1
                    .allData
                    .push(IECData::float32(cmd.pcs_reactive_power));
            }
            cmd_position += entries_count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_config() -> PublisherConfig {
        PublisherConfig {
            src_addr: "01:0C:CD:01:00:01".to_string(),
            dst_addr: "01:0C:CD:FF:FF:FF".to_string(),
            tpid: 0x8100,
            tci: 0x8002,
            appid: 0x0008,
            gocb_ref: "TestDevice/LLN0$GO$Gcb1".to_string(),
            dat_set: "TestDevice/LLN0$dsGOOSE1".to_string(),
            goose_id: "TestDevice/LLN0.Gcb1".to_string(),
            simulation: false,
            conf_rev: 1,
            ndscom: false,
            number_of_pcs: 2,
        }
    }

    #[test]
    fn test_init_publisher_goose_frames_success() {
        let configs = vec![create_valid_config()];
        let result = init_publisher_goose_frames(&configs);

        assert!(result.is_ok(), "Should succeed with valid config");
        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);

        // Verify Ethernet header
        assert_eq!(frames[0].0.srcAddr, [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
        assert_eq!(frames[0].0.dstAddr, [0x01, 0x0C, 0xCD, 0xFF, 0xFF, 0xFF]);

        // Verify GOOSE PDU
        assert_eq!(frames[0].1.gocbRef, "TestDevice/LLN0$GO$Gcb1");
        assert_eq!(frames[0].1.numDatSetEntries, 2);
        assert_eq!(frames[0].1.allData.len(), 8); // 2 * (2 bools + 2 floats) = 8
    }

    #[test]
    fn test_init_publisher_goose_frames_empty_config() {
        let configs: Vec<PublisherConfig> = vec![];
        let result = init_publisher_goose_frames(&configs);

        assert!(result.is_err(), "Should fail with empty config");
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_init_publisher_goose_frames_invalid_src_mac() {
        let mut config = create_valid_config();
        config.src_addr = "invalid:mac:addr".to_string();

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with invalid source MAC");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("source MAC"),
            "Error should mention source MAC"
        );
    }

    #[test]
    fn test_init_publisher_goose_frames_invalid_dst_mac() {
        let mut config = create_valid_config();
        config.dst_addr = "ZZ:ZZ:ZZ:ZZ:ZZ:ZZ".to_string();

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with invalid destination MAC");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("destination MAC"),
            "Error should mention destination MAC"
        );
    }

    #[test]
    fn test_init_publisher_goose_frames_zero_appid() {
        let mut config = create_valid_config();
        config.appid = 0;

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with APPID 0");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("APPID"), "Error should mention APPID");
    }

    #[test]
    fn test_init_publisher_goose_frames_zero_number_of_pcs() {
        let mut config = create_valid_config();
        config.number_of_pcs = 0;

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with 0 number_of_pcs");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("numDatSetEntries"),
            "Error should mention numDatSetEntries"
        );
    }

    #[test]
    fn test_init_publisher_goose_frames_empty_gocb_ref() {
        let mut config = create_valid_config();
        config.gocb_ref = "".to_string();

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with empty gocbRef");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("gocbRef"), "Error should mention gocbRef");
    }

    #[test]
    fn test_init_publisher_goose_frames_empty_dat_set() {
        let mut config = create_valid_config();
        config.dat_set = "".to_string();

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with empty datSet");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("datSet"), "Error should mention datSet");
    }

    #[test]
    fn test_init_publisher_goose_frames_empty_goose_id() {
        let mut config = create_valid_config();
        config.goose_id = "".to_string();

        let result = init_publisher_goose_frames(&[config]);
        assert!(result.is_err(), "Should fail with empty goID");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("goID"), "Error should mention goID");
    }

    #[test]
    fn test_init_publisher_goose_frames_multiple_configs() {
        let config1 = create_valid_config();
        let mut config2 = create_valid_config();
        config2.src_addr = "01:0C:CD:01:00:02".to_string();
        config2.goose_id = "TestDevice2/LLN0.Gcb2".to_string();
        config2.number_of_pcs = 3;

        let result = init_publisher_goose_frames(&[config1, config2]);
        assert!(result.is_ok(), "Should succeed with multiple valid configs");

        let frames = result.unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].1.numDatSetEntries, 2);
        assert_eq!(frames[1].1.numDatSetEntries, 3);
        assert_eq!(frames[1].1.allData.len(), 12); // 3 * 4 = 12
    }

    #[test]
    fn test_init_publisher_goose_frames_second_config_invalid() {
        let config1 = create_valid_config();
        let mut config2 = create_valid_config();
        config2.appid = 0; // Invalid

        let result = init_publisher_goose_frames(&[config1, config2]);
        assert!(result.is_err(), "Should fail if any config is invalid");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Config 1"),
            "Error should mention config index"
        );
    }

    #[test]
    fn test_parse_mac_colon_format() {
        let result = parse_mac("01:0C:CD:01:00:01");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
    }

    #[test]
    fn test_parse_mac_dash_format() {
        let result = parse_mac("01-0C-CD-01-00-01");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
    }

    #[test]
    fn test_parse_mac_no_separator() {
        let result = parse_mac("010CCD010001");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
    }

    #[test]
    fn test_parse_mac_invalid_hex() {
        let result = parse_mac("ZZ:0C:CD:01:00:01");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_mac_wrong_length() {
        let result = parse_mac("01:0C:CD:01:00");
        assert!(result.is_err());
    }
}
