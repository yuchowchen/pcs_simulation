// rs to plc data structure

use anyhow::Result;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::pcs::SubscriberPCSData;

#[allow(dead_code)]
#[derive(Debug, Clone,Serialize,Deserialize)]
pub struct StPCSImage {
    pub protocol: u8,
    pub number_of_pcs: u16,
    pub lifecounter: u64,
    pub spare: [u8; 16],
    pub pcs_data_networka: Vec<StPCSinfo>,
    pub pcs_data_networkb: Vec<StPCSinfo>, //if only one network, leave it empty.
}

impl Default for StPCSImage {
    fn default() -> Self {
        Self {
            protocol: 10,
            number_of_pcs: 0,
            lifecounter: 0,
            spare: [0; 16],
            pcs_data_networka: Vec::new(),
            pcs_data_networkb: Vec::new(),
        }
    }
}

//stPCSinfo structure
#[derive(Debug, Clone,Serialize,Deserialize)]
pub struct StPCSinfo {
    pub logical_id: u16,
    pub is_valid: u8, // 1: valid, 0: invalid
    pub feed_line_id: u8,
    pub is_controllable: u8, // 1: controllable, 0: uncontrollable
    pub pcs_realtime_active_power: f32,
    pub pcs_realtime_reactive_power: f32,
    pub pcs_maximum_charging_power: f32,
    pub pcs_maximum_discharging_power: f32,
    pub pcs_maximum_inductive_power: f32,
    pub pcs_maximum_capacitive_power: f32,
    pub pcs_soc: f32,
    pub spare: [u8; 16],
}

impl Default for StPCSinfo {
    fn default() -> Self {
        Self {
            logical_id: 0,
            is_valid: 0,
            feed_line_id: 0,
            is_controllable: 0,
            pcs_realtime_active_power: 0.0,
            pcs_realtime_reactive_power: 0.0,
            pcs_maximum_charging_power: 0.0,
            pcs_maximum_discharging_power: 0.0,
            pcs_maximum_inductive_power: 0.0,
            pcs_maximum_capacitive_power: 0.0,
            pcs_soc: 0.0,
            spare: [0; 16],
        }
    }
}

/// Configuration for byte positions of PCS data in GOOSE PDU allData field
/// This maps the position of each data field for a specific PCS type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StPCSDataBytePosInAllDataCfg {
    /// PCS type identifier (e.g., "PCS-A", "PCS-B")
    pub pcstype: String,
    
    /// Number of PCS devices of this type
    pub quantityofthistype: usize,
    
    /// Byte position of realtime active power in allData
    pub pcs_realtime_active_power_pos: usize,
    
    /// Byte position of realtime reactive power in allData
    pub pcs_realtime_reactive_power_pos: usize,
    
    /// Byte position of maximum charging power in allData
    pub pcs_maximum_charging_power_pos: usize,
    
    /// Byte position of maximum discharging power in allData
    pub pcs_maximum_discharging_power_pos: usize,
    
    /// Byte position of maximum inductive power in allData
    pub pcs_maximum_inductive_power_pos: usize,
    
    /// Byte position of maximum capacitive power in allData
    pub pcs_maximum_capacitive_power_pos: usize,
    
    /// Byte position of State of Charge (SOC) in allData
    pub pcs_soc_pos: usize,
    
    /// Byte position of PCS status in allData
    pub pcs_status_pos: usize,

    /// pcs controllable status values
    pub pcs_controllable_status_value: HashMap<String, u8>,
}






/// Load PCS data byte position configurations from a JSON file
/// 
/// The JSON file should contain an array of PCS type configurations, where each
/// configuration specifies the byte positions of various data fields in the GOOSE
/// PDU allData field.
///
/// # Arguments
/// * `path` - Path to the JSON configuration file
///
/// # Returns
/// * `Ok(HashMap<String,(StPCSDataBytePosInAllDataCfg, Vec<u8>)>)` - HashMap with key as PCS type name and value as a tuple of config and its controllable status values for each type.
///   - PCS configuration for that type
///   - Vector of controllable status values for that specific type
/// * `Err` - Error if file reading or parsing fails
pub fn load_pcs_alldata_config<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, (StPCSDataBytePosInAllDataCfg, Vec<u8>)>> {
    info!("Loading PCS allData configuration from: {:?}", path.as_ref());
    
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open PCS allData config file '{:?}': {}", path.as_ref(), e);
            anyhow::bail!("Failed to open PCS allData config file '{:?}': {}", path.as_ref(), e);
        }
    };
    let reader = BufReader::new(file);

    let configs: Vec<StPCSDataBytePosInAllDataCfg> = match serde_json::from_reader(reader) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to parse PCS allData config JSON from '{:?}': {}", path.as_ref(), e);
            anyhow::bail!("Failed to parse PCS allData config JSON: {}", e);
        }
    };
    
    // Validate configurations
    for (idx, config) in configs.iter().enumerate() {
        if config.pcstype.trim().is_empty() {
            error!("Configuration at index {}: pcstype is empty", idx);
            anyhow::bail!("Invalid configuration at index {}: pcstype cannot be empty", idx);
        }
        
        if config.quantityofthistype == 0 {
            error!("Configuration at index {} ({}): quantityofthistype is zero", idx, config.pcstype);
            anyhow::bail!("Invalid configuration for {}: quantityofthistype must be > 0", config.pcstype);
        }
        
        info!(
            "Loaded config for PCS type '{}': {} devices, {} controllable status values",
            config.pcstype,
            config.quantityofthistype,
            config.pcs_controllable_status_value.len()
        );
        
        // Log controllable status values for this type
        for (key, value) in &config.pcs_controllable_status_value {
            info!("  PCS type '{}': controllable status '{}' = {}", 
                config.pcstype, key, value);
        }
    }

    // Create result HashMap with key as PCS type name and value as a tuple of config and its controllable status values for each type.
    let result: HashMap<String, (StPCSDataBytePosInAllDataCfg, Vec<u8>)> = configs.into_iter()
        .map(|config| {
            let pcs_type = config.pcstype.clone();
            let status_values: Vec<u8> = config.pcs_controllable_status_value.values().copied().collect();
            (pcs_type, (config, status_values))
        })
        .collect();
    
    info!("Successfully loaded {} PCS type configuration(s)", result.len());
    Ok(result)
}

/// Calculate total quantity of all PCS devices across all types
/// 
/// Iterates through the configuration vector and sums up the `quantityofthistype`
/// field for each PCS type configuration.
///
/// # Arguments
/// * `configs` - Reference to vector of PCS configurations
///
/// # Returns
/// * Total number of PCS devices across all types
///
/// # Example
/// ```no_run
/// use pcs_simulator:plc::types::{load_pcs_alldata_config, get_total_pcs_quantity};
/// 
/// # fn main() -> anyhow::Result<()> {
/// let configs_with_status = load_pcs_alldata_config("config.json")?;
/// let configs: Vec<_> = configs_with_status.values().map(|(cfg, _)| cfg.clone()).collect();
/// let total = get_total_pcs_quantity(&configs);
/// println!("Total PCS devices: {}", total);
/// # Ok(())
/// # }
/// ```
pub fn get_total_pcs_quantity(configs: &[StPCSDataBytePosInAllDataCfg]) -> usize {
    let total = configs.iter()
        .map(|config| config.quantityofthistype)
        .sum();
    
    info!("Total PCS quantity across {} type(s): {}", configs.len(), total);
    total
}

/// Count the quantity of PCS devices grouped by type
/// 
/// Iterates through the configuration vector and creates a HashMap mapping
/// each PCS type name to its quantity.
///
/// # Arguments
/// * `configs` - Reference to vector of PCS configurations
///
/// # Returns
/// * HashMap where keys are PCS type names and values are quantities
///
/// # Example
/// ```no_run
/// use pcs_simulator:plc::types::{load_pcs_alldata_config, count_pcs_by_type};
/// 
/// # fn main() -> anyhow::Result<()> {
/// let configs_with_status = load_pcs_alldata_config("config.json")?;
/// let configs: Vec<_> = configs_with_status.values().map(|(cfg, _)| cfg.clone()).collect();
/// let counts = count_pcs_by_type(&configs);
/// for (pcs_type, quantity) in counts.iter() {
///     println!("{}: {} devices", pcs_type, quantity);
/// }
/// # Ok(())
/// # }
/// ```
pub fn count_pcs_by_type(configs: &[StPCSDataBytePosInAllDataCfg]) -> HashMap<String, usize> {
    let mut type_counts = HashMap::new();
    
    for config in configs {
        type_counts.insert(config.pcstype.clone(), config.quantityofthistype);
        info!("PCS type '{}': {} device(s)", config.pcstype, config.quantityofthistype);
    }
    
    info!("Counted {} unique PCS type(s)", type_counts.len());
    type_counts
}

impl StPCSinfo {
    /// Sentinel value indicating invalid/missing data
    pub const INVALID_VALUE: f32 = 999999.0;

    /// Populate StPCSinfo from SubscriberPCSData using configuration mappings
    /// 
    /// # Arguments
    /// * `logical_id` - The logical ID of the PCS
    /// * `pcstype` - The PCS type string (e.g., "PCS-A", "PCS-B")
    /// * `cfg` - Configuration mapping for data field positions
    /// * `subscriber` - The source SubscriberPCSData to extract values from
    /// 
    /// # Returns
    /// * `Ok(())` if data was successfully extracted
    /// * `Err(String)` with error description if extraction failed
    /// 
    /// # Error Cases
    /// * Configuration not found for the specified PCS type
    /// * Data field missing from allData array (sets field to INVALID_VALUE)
    /// * Data type mismatch (sets field to INVALID_VALUE)
    pub fn get_info(
        &mut self, 
        lan_id:u8,
        logical_id: u16, 
        pcstype: String, 
        cfg: &HashMap<String, (StPCSDataBytePosInAllDataCfg, Vec<u8>)>,
        subscriber: &SubscriberPCSData
    ) -> Result<(), String> {
        // Set basic fields
        self.logical_id = logical_id;
        self.is_valid = subscriber.is_data_valid() as u8;
        self.feed_line_id = subscriber.nameplate_feed_line_id().unwrap_or(0) as u8;
        self.is_controllable = 0; // Default to not controllable, will be set based on status

        // Find the matching configuration and status values for the given PCS type
        let (config, status_value) = cfg.get(&pcstype)
            .ok_or_else(|| {
                let available_types: Vec<&String> = cfg.keys().collect();
                let err_msg = format!("No configuration found for PCS type: '{}'. Available types: {:?}", 
                    pcstype, available_types
                );
                error!("{}", err_msg);
                err_msg
            })?;

        let alldata = subscriber.get_alldata();
        // PERFORMANCE: Don't log full allData array - causes I/O blocking
        // log::debug!("Extracting PCS data from lan{} for logical_id {} of type '{}' with {} allData entries", 
        //     lan_id, logical_id, pcstype, alldata.len());
        let mut warnings = Vec::new();
        
        // Extract realtime active power
        match alldata.get(config.pcs_realtime_active_power_pos) {
            Some(value) => {
                self.pcs_realtime_active_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Active power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_realtime_active_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Active power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_realtime_active_power_pos, alldata.len()));
                self.pcs_realtime_active_power = Self::INVALID_VALUE;
            }
        }
        
        // Extract realtime reactive power
        match alldata.get(config.pcs_realtime_reactive_power_pos) {
            Some(value) => {
                self.pcs_realtime_reactive_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Reactive power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_realtime_reactive_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Reactive power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_realtime_reactive_power_pos, alldata.len()));
                self.pcs_realtime_reactive_power = Self::INVALID_VALUE;
            }
        }
        
        // Extract maximum charging power
        match alldata.get(config.pcs_maximum_charging_power_pos) {
            Some(value) => {
                self.pcs_maximum_charging_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Max charging power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_maximum_charging_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Max charging power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_maximum_charging_power_pos, alldata.len()));
                self.pcs_maximum_charging_power = Self::INVALID_VALUE;
            }
        }
        
        // Extract maximum discharging power
        match alldata.get(config.pcs_maximum_discharging_power_pos) {
            Some(value) => {
                self.pcs_maximum_discharging_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Max discharging power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_maximum_discharging_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Max discharging power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_maximum_discharging_power_pos, alldata.len()));
                self.pcs_maximum_discharging_power = Self::INVALID_VALUE;
            }
        }
        
        // Extract maximum inductive power
        match alldata.get(config.pcs_maximum_inductive_power_pos) {
            Some(value) => {
                self.pcs_maximum_inductive_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Max inductive power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_maximum_inductive_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Max inductive power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_maximum_inductive_power_pos, alldata.len()));
                self.pcs_maximum_inductive_power = Self::INVALID_VALUE;
            }
        }
        
        // Extract maximum capacitive power
        match alldata.get(config.pcs_maximum_capacitive_power_pos) {
            Some(value) => {
                self.pcs_maximum_capacitive_power = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} Max capacitive power: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_maximum_capacitive_power_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} Max capacitive power: position {} out of bounds (allData length: {})", 
                    lan_id,logical_id, config.pcs_maximum_capacitive_power_pos, alldata.len()));
                self.pcs_maximum_capacitive_power = Self::INVALID_VALUE;
            }
        }

        // Extract State of Charge (SOC)
        match alldata.get(config.pcs_soc_pos) {
            Some(value) => {
                self.pcs_soc = value.as_f32().unwrap_or_else(|| {
                    warnings.push(format!("lan{} PCS{} SOC: wrong type at position {}, expected float32, got {}", 
                        lan_id,logical_id, config.pcs_soc_pos, value.variant_name()));
                    Self::INVALID_VALUE
                });
            }
            None => {
                warnings.push(format!("lan{} PCS{} SOC: position {} out of bounds (allData length: {})",    
                    lan_id,logical_id, config.pcs_soc_pos, alldata.len()));
                self.pcs_soc = Self::INVALID_VALUE;
            }
        }   

        // Extract pcs status (comes as float, convert to integer)
        match alldata.get(config.pcs_status_pos) {
            Some(value) => {
                // Try to extract status as float32 first, then convert to u8
                match value.as_f32() {
                    Some(status_float) => {
                        // Convert float to integer (round to nearest)
                        let status = status_float.round() as u8;
                        // info!("lan{} PCS{} status (from float): {}", lan_id, logical_id, status);
                        // Check if this status indicates controllable
                        if status_value.contains(&status) {
                            self.is_controllable = 1;
                            // info!("lan{} PCS{} status {} (from float {}) indicates controllable", 
                            //     lan_id, logical_id, status, status_float);
                        } else {
                            self.is_controllable = 0;
                            // log::warn!("lan{} PCS{} status {} (from float {}) is not in controllable status values {:?}", 
                            //     lan_id, logical_id, status, status_float, status_value);
                        }
                    }
                    None => {
                        // If not float32, try u8 directly as fallback
                        match value.as_u8() {
                            Some(status) => {
                                if status_value.contains(&status) {
                                    self.is_controllable = 1;
                                    info!("lan{} PCS{} status {} (u8) indicates controllable", lan_id, logical_id, status);
                                } else {
                                    self.is_controllable = 0;
                                    // log::warn!("lan{} PCS{} status {} (u8) is not in controllable status values {:?}", 
                                    //     lan_id, logical_id, status, status_value);
                                }
                            }
                            None => {
                                // Failed to extract status  as either float or u8
                                warnings.push(format!("lan{} PCS{} status: wrong type at position {}, expected float32 or u8, got {}", 
                                    lan_id, logical_id, config.pcs_status_pos, value.variant_name()));
                                self.is_controllable = 0;
                                // log::warn!("lan{} PCS{} cannot determine controllable status due to type mismatch", lan_id, logical_id);
                            }
                        }
                    }
                }
            }
            None => {
                warnings.push(format!("PCS status: position {} out of bounds (allData length: {})", 
                    config.pcs_status_pos, alldata.len()));
                self.is_controllable = 0;
                // log::warn!("lan{} PCS{} cannot determine controllable status due to missing data", lan_id, logical_id);
            }
        }
        
        // Log warnings if any field extraction had issues
        // if !warnings.is_empty() {
        //     // log::warn!("lan{} PCS{} (type: {}): {} field extraction warnings:", 
        //     //     lan_id, logical_id, pcstype, warnings.len());
        //     // for warning in &warnings {
        //     //     log::warn!("  - {}", warning);
        //     // }
        // }
        
        // info!("Extracted PCS info from lan{} for logical_id {} (type: {}): active_power={}, reactive_power={}", 
        //       lan_id, logical_id, pcstype, self.pcs_realtime_active_power, self.pcs_realtime_reactive_power);
        
        Ok(())
    }
}

