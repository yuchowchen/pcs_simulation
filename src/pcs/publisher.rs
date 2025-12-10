// GOOSE Publisher for PCS Data
// Each PCS has its own GOOSE frame based on nameplate configuration
// Each PCS type has different allData field mappings from PCS_publisher_alldata_mapping.json

use crate::goose::types::{EthernetHeader, IECData, IECGoosePdu};
use crate::pcs::{NameplateConfig, PublisherPcsData};
use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

/// Type alias for GOOSE frame (Ethernet header + GOOSE PDU)
pub type GooseFrame = (EthernetHeader, IECGoosePdu);

/// Mapping configuration for PCS type-specific allData fields
/// Fields are stored as a Vec to preserve the exact order from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcsTypeMapping {
    pub pcstype: String,
    /// Ordered list of (field_name, data_type) - order matches JSON and GOOSE frame positions
    #[serde(skip)]
    pub fields: Vec<(String, String)>,
}

/// Load PCS type mappings from JSON file
/// Preserves field order from JSON which matches GOOSE allData positions
pub fn load_pcs_type_mappings(path: &str) -> Result<HashMap<String, PcsTypeMapping>> {
    info!("Loading PCS type mappings from: {}", path);
    
    let file = File::open(path)
        .with_context(|| format!("Failed to open PCS type mapping file: {}", path))?;
    
    let reader = BufReader::new(file);
    let json_array: Vec<Value> = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse PCS type mapping JSON: {}", path))?;
    
    let mut result = HashMap::new();
    
    for json_obj in json_array {
        let obj = json_obj.as_object()
            .ok_or_else(|| anyhow::anyhow!("Expected JSON object in mapping array"))?;
        
        // Extract pcstype first
        let pcs_type = obj.get("pcstype")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'pcstype' field"))?
            .to_string();
        
        // Build ordered field list from JSON object, preserving insertion order
        // serde_json::Map preserves the order from the JSON file
        let mut fields = Vec::new();
        for (field_name, value) in obj.iter() {
            if field_name == "pcstype" {
                continue; // Skip the pcstype field itself
            }
            let data_type = value.as_str()
                .ok_or_else(|| anyhow::anyhow!("Field '{}' has non-string value", field_name))?
                .to_string();
            fields.push((field_name.clone(), data_type));
        }
        
        let field_count = fields.len();
        let mapping = PcsTypeMapping {
            pcstype: pcs_type.clone(),
            fields,
        };
        
        result.insert(pcs_type.clone(), mapping);
        info!("Loaded mapping for PCS type: {} with {} fields (order preserved from JSON)", pcs_type, field_count);
    }
    
    Ok(result)
}

/// Initialize GOOSE frame for a single PCS from its nameplate configuration
pub fn init_goose_frame_for_pcs(
    nameplate: &NameplateConfig,
    type_mapping: &PcsTypeMapping,
) -> Result<GooseFrame> {
    // Parse source MAC address
    let src_mac = nameplate.goose_src_addr.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_src_addr for PCS logical_id {:?}", nameplate.logical_id))?;
    let src_mac = parse_mac(src_mac)?;
    
    // Parse destination MAC address
    let dst_mac = nameplate.goose_dst_addr.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_dst_addr for PCS logical_id {:?}", nameplate.logical_id))?;
    let dst_mac = parse_mac(dst_mac)?;
    
    // Parse TPID
    let tpid = nameplate.goose_tpid.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_tpid for PCS logical_id {:?}", nameplate.logical_id))?;
    let tpid = parse_hex_u16(tpid)?;
    
    // Parse TCI
    let tci = nameplate.goose_tci.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_tci for PCS logical_id {:?}", nameplate.logical_id))?;
    let tci = parse_hex_u16(tci)?;
    
    // Parse APPID
    let appid = nameplate.goose_appid.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_appid for PCS logical_id {:?}", nameplate.logical_id))?;
    // let appid = parse_hex_u16(appid)?;
    
    // Get GOOSE PDU fields
    let gocb_ref = nameplate.goose_gocb_ref.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_gocb_ref for PCS logical_id {:?}", nameplate.logical_id))?;
    let data_set = nameplate.goose_data_set.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_data_set for PCS logical_id {:?}", nameplate.logical_id))?;
    let go_id = nameplate.goose_go_id.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing goose_go_id for PCS logical_id {:?}", nameplate.logical_id))?;
    
    let simulation = nameplate.goose_simulation.as_ref()
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);
    
    let conf_rev = nameplate.goose_conf_rev.as_ref()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    
    let nds_com = nameplate.goose_nds_com.as_ref()
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);
    
    // Create Ethernet header
    let mut eth_header = EthernetHeader::default();
    eth_header.srcAddr = src_mac;
    eth_header.dstAddr = dst_mac;
    eth_header.TPID = tpid.to_be_bytes();
    eth_header.TCI = tci.to_be_bytes();
    eth_header.ehterType = [0x88, 0xB8]; // GOOSE Ethertype
    eth_header.APPID = appid.to_be_bytes();
    
    // Create GOOSE PDU
    let mut goose_pdu = IECGoosePdu::default();
    goose_pdu.gocbRef = gocb_ref.clone();
    goose_pdu.timeAllowedtoLive = 5000; // Default 5 seconds
    goose_pdu.datSet = data_set.clone();
    goose_pdu.goID = go_id.clone();
    goose_pdu.t = [0; 8]; // Will be updated when publishing
    goose_pdu.stNum = 0;
    goose_pdu.sqNum = 0;
    goose_pdu.simulation = simulation;
    goose_pdu.confRev = conf_rev;
    goose_pdu.ndsCom = nds_com;
    
    // Initialize allData based on type mapping
    // Fields are already in correct order from JSON (Vec preserves order)
    goose_pdu.numDatSetEntries = type_mapping.fields.len() as u32;
    
    // Initialize allData with default values in the exact order from JSON
    // This order matches the GOOSE frame structure where position matters
    for (field_name, data_type) in &type_mapping.fields {
        match data_type.as_str() {
            "boolean" => goose_pdu.allData.push(IECData::boolean(false)),
            "float" => goose_pdu.allData.push(IECData::float32(0.0)),
            "int" => goose_pdu.allData.push(IECData::int32(0)),
            _ => warn!("Unknown data type '{}' for field '{}'", data_type, field_name),
        }
    }
    
    info!("Initialized GOOSE frame for PCS logical_id {:?}, type {}, {} fields in JSON order",
        nameplate.logical_id, type_mapping.pcstype, goose_pdu.allData.len());
    
    Ok((eth_header, goose_pdu))
}

/// Update GOOSE frame allData with current PCS data
pub fn update_goose_frame_data(
    frame: &mut GooseFrame,
    pcs_data: &PublisherPcsData,
    type_mapping: &PcsTypeMapping,
) -> Result<()> {
    // Update allData values based on field mappings
    // Fields are in correct order from JSON (Vec preserves order)
    for (data_index, (field_name, _data_type)) in type_mapping.fields.iter().enumerate() {
        if data_index >= frame.1.allData.len() {
            break;
        }
        
        // Map field names to actual PCS data based on position in allData
        // Field order from JSON matches GOOSE frame structure
        match field_name.as_str() {
            name if name.contains("realtime_active_power") => {
                let (active_power, _, _, _) = pcs_data.get_feedback_values();
                frame.1.allData[data_index] = IECData::float32(active_power);
            }
            name if name.contains("realtime_reactive_power") => {
                let (_, reactive_power, _, _) = pcs_data.get_feedback_values();
                frame.1.allData[data_index] = IECData::float32(reactive_power);
            }
            name if name.contains("status") => {
                // Default status - extend based on your PCSStatus enum
                frame.1.allData[data_index] = IECData::int32(2); // Standby
            }
            name if name.contains("soc") => {
                // State of charge - placeholder
                frame.1.allData[data_index] = IECData::float32(50.0);
            }
            name if name.contains("maximum_charging_power") => {
                frame.1.allData[data_index] = IECData::float32(1000.0); // Placeholder
            }
            name if name.contains("maximum_discharging_power") => {
                frame.1.allData[data_index] = IECData::float32(1000.0); // Placeholder
            }
            name if name.contains("maximum_capacitive_power") => {
                frame.1.allData[data_index] = IECData::float32(500.0); // Placeholder
            }
            name if name.contains("maximum_inductive_power") => {
                frame.1.allData[data_index] = IECData::float32(500.0); // Placeholder
            }
            _ => {
                // Keep default values for spare fields
            }
        }
    }
    
    Ok(())
}

/// Parse MAC address string into [u8; 6]
/// Supports formats: "01:0C:CD:01:00:01", "01-0C-CD-01-00-01", "010CCD010001"
fn parse_mac(s: &str) -> Result<[u8; 6]> {
    // Remove quotes if present
    let s = s.trim().trim_matches('"');
    
    // Try split by common separators first
    let parts: Vec<&str> = s.split(|c| c == ':' || c == '-' || c == '.').collect();
    if parts.len() == 6 {
        let mut mac = [0u8; 6];
        for (i, p) in parts.iter().enumerate() {
            if p.len() != 2 {
                anyhow::bail!("MAC part '{}' has wrong length in '{}'", p, s);
            }
            mac[i] = u8::from_str_radix(p, 16)
                .with_context(|| format!("Invalid hex '{}' in MAC address '{}'", p, s))?;
        }
        return Ok(mac);
    }
    
    // Otherwise strip everything except hex digits and try parse as 12 hex chars
    let s_hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if s_hex.len() == 12 {
        let mut mac = [0u8; 6];
        for i in 0..6 {
            let byte = &s_hex[2 * i..2 * i + 2];
            mac[i] = u8::from_str_radix(byte, 16)
                .with_context(|| format!("Invalid hex '{}' in MAC address '{}'", byte, s))?;
        }
        return Ok(mac);
    }
    
    anyhow::bail!("Invalid MAC format: {}", s)
}

/// Parse hex string (with or without 0x prefix) to u16
fn parse_hex_u16(s: &str) -> Result<u16> {
    let s = s.trim().trim_matches('"');
    let s = s.strip_prefix("0x").unwrap_or(s);
    u16::from_str_radix(s, 16)
        .with_context(|| format!("Failed to parse hex u16: {}", s))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_mac_colon_format() {
        let result = parse_mac("01:0C:CD:01:00:01");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
    }
    
    #[test]
    fn test_parse_mac_with_quotes() {
        let result = parse_mac("\"01:0C:CD:01:00:01\"");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x01]);
    }
    
    #[test]
    fn test_parse_hex_u16_with_prefix() {
        let result = parse_hex_u16("0x8100");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x8100);
    }
    
    #[test]
    fn test_parse_hex_u16_without_prefix() {
        let result = parse_hex_u16("8100");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x8100);
    }
}
