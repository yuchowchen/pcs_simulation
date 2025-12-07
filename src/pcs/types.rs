use crate::goose::types::{IECData, IECGoosePdu};
use anyhow::Result;
use log::warn;
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SubscriberPCSData {
    /// Nameplate/configuration values for this PCS (includes device id)
    nameplate: Option<crate::pcs::NameplateConfig>,
    goosepdu: IECGoosePdu, // goosepdu from the latest GOOSE message
    last_update: Option<std::time::SystemTime>, // timestamp of last update
    invaliditytime: Option<std::time::SystemTime>, // timestamp when data became invalid
    statevalid: bool,      // state valid flag from GOOSE
    
    // Feedback values - updated from received GOOSE commands
    /// Active power feedback value in kW (updated from setpoint commands)
    active_power_feedback: f32,
    /// Reactive power feedback value in kVAR (updated from setpoint commands)
    reactive_power_feedback: f32,
    /// Active power enable flag from received command
    active_power_enable: bool,
    /// Reactive power enable flag from received command
    reactive_power_enable: bool,
}

impl SubscriberPCSData {
    /// Load nameplate configurations from a CSV and build an initial
    /// HashMap of `PCSData` entries keyed by runtime `pcs_id`. Each resulting
    /// `PCSData` will have its `pcs_id` set (from the nameplate when present,
    /// otherwise 0) and the nameplate attached.
    pub fn pcs_from_nameplates<P: AsRef<Path>>(path: P) -> Result<HashMap<u16, SubscriberPCSData>> {
        let configs = crate::pcs::nameplate::load_nameplates_from_csv(path)?;
        let mut out: HashMap<u16, SubscriberPCSData> = HashMap::with_capacity(configs.len());

        for cfg in configs.into_iter() {
            // Determine a runtime device id: prefer explicit device_id from
            // the nameplate, otherwise use 0 as a fallback.
            let pcs_id = cfg.logical_id.unwrap_or(0);

            let mut pcs = SubscriberPCSData::new();
            pcs.set_nameplate(cfg);
            out.insert(pcs_id, pcs);
        }

        Ok(out)
    }
}

impl SubscriberPCSData {
    /// Return a copy of the nameplate logical id
    pub fn nameplate_logical_id(&self) -> Option<u16> {
        self.nameplate.as_ref().and_then(|np| np.logical_id)
    }

    /// Return the GOOSE APPID from the attached nameplate if present.
    pub fn nameplate_appid(&self) -> Option<u16> {
        self.nameplate.as_ref().and_then(|np| np.goose_appid)
    }

    /// Return the feed line ID from the attached nameplate if present.
    pub fn nameplate_feed_line_id(&self) -> Option<u16> {
        self.nameplate.as_ref().and_then(|np| np.feed_line_id)
    }

    /// Return the device ID from the attached nameplate if present.
    pub fn nameplate_device_id(&self) -> Option<&String> {
        self.nameplate.as_ref().and_then(|np| np.device_id.as_ref())
    }

    /// Return the feed line alias from the attached nameplate if present.
    pub fn nameplate_feed_line_alias(&self) -> Option<&String> {
        self.nameplate
            .as_ref()
            .and_then(|np| np.feed_line_alias.as_ref())
    }

    pub fn nameplate_pcs_type(&self) -> Option<String> {
        self.nameplate.as_ref().and_then(|np| np.pcs_type.clone())
    }
}

impl SubscriberPCSData {
    /// Create a new SubscriberPCSData with minimal required runtime fields. Nameplate
    /// values can be set separately. The `logical_id` is the runtime
    /// identifier used for message matching.
    pub fn new() -> Self {
        SubscriberPCSData {
            nameplate: None,
            goosepdu: IECGoosePdu::default(),
            last_update: None,
            invaliditytime: None,
            statevalid: false, // means communication state is OK or not. OR the received GOOSE message is valid or not by compare stNum and sqNumber.
            active_power_feedback: 0.0,
            reactive_power_feedback: 0.0,
            active_power_enable: false,
            reactive_power_enable: false,
        }
    }

    /// Return the primary device identifier for this PCS (logical id from nameplate
    /// or 0 when none is set). Useful for matching incoming messages to runtime entries.
    pub fn pcs_id(&self) -> u16 {
        self.nameplate_logical_id().unwrap_or(0)
    }

    /// Attach a `NameplateConfig` to this runtime entry. This stores the
    /// config and, if the runtime device id was previously "unknown", will
    /// adopt a provided nameplate device id.
    pub fn set_nameplate(&mut self, cfg: crate::pcs::NameplateConfig) {
        self.nameplate = Some(cfg);
    }

    /// Update the goosepdu field with new GOOSE packet data and timestamps.
    /// This is called when a new GOOSE packet is received for this PCS.
    pub fn update_from_goose(&mut self, pdu: &IECGoosePdu, lan_id: u16) {
        // Check GOOSE sequence validity before updating
        // REDUCED LOGGING: Excessive logging while holding DashMap lock causes contention
        log::info!(
            "LAN{} Updating PCS ID: {} (APPID: {:04X}) stNum: {}→{}, sqNum: {}→{}",
            lan_id,
            self.pcs_id(),
            self.nameplate_appid().unwrap_or(0),
            self.goosepdu.stNum,
            pdu.stNum,
            self.goosepdu.sqNum,
            pdu.sqNum
        );
        
        let is_sequence_valid = self.validate_goose_sequence(pdu, lan_id);

        // PERFORMANCE: Copy PDU fields instead of clone() to avoid deep copy of Vec<IECData>
        // Only copy critical fields that are actually used
        self.goosepdu.stNum = pdu.stNum;
        self.goosepdu.sqNum = pdu.sqNum;
        self.goosepdu.timeAllowedtoLive = pdu.timeAllowedtoLive;
        self.goosepdu.t = pdu.t;
        self.goosepdu.simulation = pdu.simulation;
        self.goosepdu.confRev = pdu.confRev;
        self.goosepdu.ndsCom = pdu.ndsCom;
        self.goosepdu.numDatSetEntries = pdu.numDatSetEntries;
        // Only clone allData if it's needed - this is the expensive part
        self.goosepdu.allData = pdu.allData.clone();
        
        // Update timestamps
        self.last_update = Some(std::time::SystemTime::now());

        // Calculate invalidate time based on timeAllowedtoLive from GOOSE PDU
        // timeAllowedtoLive is in milliseconds
        if pdu.timeAllowedtoLive > 0 {
            let ttl_duration = std::time::Duration::from_millis(pdu.timeAllowedtoLive as u64 * 2+5000); // Use 2x for safety margin and additional 5s it will be configured in the toml file.
            self.invaliditytime = Some(std::time::SystemTime::now() + ttl_duration);
        }

        // Update state validity based on GOOSE sequence validation and recent receipt
        // In GOOSE, data is considered valid if sequence is correct and we received it recently
        self.statevalid = is_sequence_valid;

        if !is_sequence_valid {
            log::warn!(
                "LAN{} GOOSE sequence validation failed (APPID: {:04X}) PCS ID: {} - stNum: {}, sqNum: {}",
                lan_id,
                self.nameplate_appid().unwrap_or(0),
                self.pcs_id(),
                pdu.stNum,
                pdu.sqNum
            );
        }
    }

    /// Extract control commands from GOOSE allData and update feedback values
    /// 
    /// The GOOSE command format from PCS controller (subscriber) contains:
    /// - For N PCS units in the GOOSE frame:
    ///   * First N*2 elements: Boolean enable flags (active_power_enable, reactive_power_enable) for each PCS
    ///   * Next N*2 elements: Float setpoints (active_power_setpoint, reactive_power_setpoint) for each PCS
    /// 
    /// # Arguments
    /// * `pcs_index` - The index of this PCS within the GOOSE frame (0-based)
    /// * `total_pcs_count` - Total number of PCS units controlled by this GOOSE frame
    /// 
    /// Returns Ok(()) if commands were successfully extracted and applied, Err otherwise
    pub fn extract_and_apply_commands(&mut self, pcs_index: usize, total_pcs_count: usize) -> Result<()> {
        let alldata = &self.goosepdu.allData;
        
        // Validate that we have enough data
        let expected_entries = total_pcs_count * 4; // 2 booleans + 2 floats per PCS
        if alldata.len() < expected_entries {
            return Err(anyhow::anyhow!(
                "Insufficient data in allData: expected {} entries, got {}",
                expected_entries,
                alldata.len()
            ));
        }
        
        // Extract boolean enable flags
        // Format: [P_enable_pcs0, Q_enable_pcs0, P_enable_pcs1, Q_enable_pcs1, ...]
        let bool_start_idx = pcs_index * 2;
        let p_enable_idx = bool_start_idx;
        let q_enable_idx = bool_start_idx + 1;
        
        // Extract float setpoints
        // Format: [P_setpoint_pcs0, Q_setpoint_pcs0, P_setpoint_pcs1, Q_setpoint_pcs1, ...]
        // These come after all the boolean flags
        let float_start_idx = total_pcs_count * 2 + (pcs_index * 2);
        let p_setpoint_idx = float_start_idx;
        let q_setpoint_idx = float_start_idx + 1;
        
        // Extract active power enable
        if let Some(p_enable_data) = alldata.get(p_enable_idx) {
            if let Some(p_enable) = p_enable_data.as_bool() {
                self.active_power_enable = p_enable;
            } else {
                log::warn!("PCS {} active power enable at index {} is not boolean, got {:?}", 
                    self.pcs_id(), p_enable_idx, p_enable_data);
            }
        }
        
        // Extract reactive power enable
        if let Some(q_enable_data) = alldata.get(q_enable_idx) {
            if let Some(q_enable) = q_enable_data.as_bool() {
                self.reactive_power_enable = q_enable;
            } else {
                log::warn!("PCS {} reactive power enable at index {} is not boolean, got {:?}", 
                    self.pcs_id(), q_enable_idx, q_enable_data);
            }
        }
        
        // Extract active power setpoint
        if let Some(p_setpoint_data) = alldata.get(p_setpoint_idx) {
            if let Some(p_setpoint) = p_setpoint_data.as_f32() {
                // Update feedback to match setpoint (simulating PCS response)
                self.active_power_feedback = if self.active_power_enable {
                    p_setpoint
                } else {
                    0.0 // If disabled, feedback goes to zero
                };
            } else {
                log::warn!("PCS {} active power setpoint at index {} is not float32, got {:?}", 
                    self.pcs_id(), p_setpoint_idx, p_setpoint_data);
            }
        }
        
        // Extract reactive power setpoint
        if let Some(q_setpoint_data) = alldata.get(q_setpoint_idx) {
            if let Some(q_setpoint) = q_setpoint_data.as_f32() {
                // Update feedback to match setpoint (simulating PCS response)
                self.reactive_power_feedback = if self.reactive_power_enable {
                    q_setpoint
                } else {
                    0.0 // If disabled, feedback goes to zero
                };
            } else {
                log::warn!("PCS {} reactive power setpoint at index {} is not float32, got {:?}", 
                    self.pcs_id(), q_setpoint_idx, q_setpoint_data);
            }
        }
        
        log::debug!(
            "PCS {} updated feedback: P_enable={}, P_feedback={:.2}kW, Q_enable={}, Q_feedback={:.2}kVAR",
            self.pcs_id(),
            self.active_power_enable,
            self.active_power_feedback,
            self.reactive_power_enable,
            self.reactive_power_feedback
        );
        
        Ok(())
    }
    
    /// Get the current feedback values
    /// Returns (active_power_feedback, reactive_power_feedback, active_power_enable, reactive_power_enable)
    pub fn get_feedback_values(&self) -> (f32, f32, bool, bool) {
        (
            self.active_power_feedback,
            self.reactive_power_feedback,
            self.active_power_enable,
            self.reactive_power_enable,
        )
    }

    /// Validate GOOSE sequence numbers according to IEC 61850-8-1 standard
    /// Returns true if the sequence is valid, false otherwise
    fn validate_goose_sequence(&self, new_pdu: &IECGoosePdu, lan_id: u16) -> bool {
        // For the first packet or if we don't have previous data, consider it valid
        if self.last_update.is_none() {
            log::warn!(
                "LAN{} First GOOSE packet received (APPID: {:04X}) PCS ID: {} - stNum: {}, sqNum: {}",
                lan_id,
                self.nameplate_appid().unwrap_or(0),
                self.pcs_id(),
                new_pdu.stNum,
                new_pdu.sqNum
            );
            return true;
        }

        let old_st_num = self.goosepdu.stNum;
        let old_sq_num = self.goosepdu.sqNum;
        let new_st_num = new_pdu.stNum;
        let new_sq_num = new_pdu.sqNum;

        // Case 1: stNum has increased - this is always valid (state change occurred)
        if new_st_num > old_st_num {
            let st_num_gap = new_st_num - old_st_num;
            if st_num_gap == 1 {
                // Normal consecutive state change
                log::warn!(
                    "LAN{} GOOSE state change detected (APPID: {:04X}) PCS ID: {} - stNum: {} -> {}, sqNum: {} -> {}",
                    lan_id,
                    self.nameplate_appid().unwrap_or(0),
                    self.pcs_id(),
                    old_st_num,
                    new_st_num,
                    old_sq_num,
                    new_sq_num
                );
            } else {
                // Non-consecutive state number - indicates missed state changes
                log::error!(
                    "LAN{} GOOSE state number GAP detected (APPID: {:04X}) PCS ID: {} - stNum: {} -> {} (gap: {}), sqNum: {} -> {} - MISSED {} state change(s)!",
                    lan_id,
                    self.nameplate_appid().unwrap_or(0),
                    self.pcs_id(),
                    old_st_num,
                    new_st_num,
                    st_num_gap,
                    old_sq_num,
                    new_sq_num,
                    st_num_gap - 1
                );
            }
            return true;
        }

        // Case 2: stNum is the same - sqNum should be monotonically increasing
        if new_st_num == old_st_num {
            if new_sq_num >= old_sq_num {
                // Valid: sequence number increased or stayed the same (retransmission)
                if new_sq_num == old_sq_num {
                    log::warn!(
                        "LAN{} GOOSE retransmission (APPID: {:04X}) PCS ID: {} - stNum: {}, sqNum: {}",
                        lan_id,
                        self.nameplate_appid().unwrap_or(0),
                        self.pcs_id(),
                        new_st_num,
                        new_sq_num
                    );
                } else {
                    log::warn!(
                        "LAN{} GOOSE sequence increment (APPID: {:04X}) PCS ID: {} - stNum: {}, sqNum: {} -> {}",
                        lan_id,
                        self.nameplate_appid().unwrap_or(0),
                        self.pcs_id(),
                        new_st_num,
                        old_sq_num,
                        new_sq_num
                    );
                }
                return true;
            } else {
                // Invalid: sequence number went backward with same state number
                log::error!(
                    "LAN{} GOOSE sequence error (APPID: {:04X}) PCS ID: {} - stNum unchanged ({}), but sqNum decreased: {} -> {}",
                    lan_id,
                    self.nameplate_appid().unwrap_or(0),
                    self.pcs_id(),
                    new_st_num,
                    old_sq_num,
                    new_sq_num
                );
                return false;
            }
        }

        // Case 3: stNum has decreased - this might indicate a restart or configuration change
        // This is generally considered suspicious but we'll log it and accept it
        log::warn!(
            "LAN{} GOOSE state number decreased (APPID: {:04X}) PCS ID: {} - stNum: {} -> {}, sqNum: {} -> {} (possible restart)",
            lan_id,
            self.nameplate_appid().unwrap_or(0),
            self.pcs_id(),
            old_st_num,
            new_st_num,
            old_sq_num,
            new_sq_num
        );
        return true;
    }

    /// Check if the data is still valid based on timeAllowedtoLive
    pub fn is_data_valid(&self) -> bool {
        if !self.statevalid {
            return false;
        }

        if let Some(invalid_time) = self.invaliditytime {
            std::time::SystemTime::now() < invalid_time
        } else {
            // If no invaliditytime set, consider valid if we have recent update
            if let Some(last_update) = self.last_update {
                let elapsed = std::time::SystemTime::now()
                    .duration_since(last_update)
                    .unwrap_or(std::time::Duration::from_secs(u64::MAX));
                elapsed < std::time::Duration::from_secs(10) // Default 10 second timeout
            } else {
                false
            }
        }
    }

    /// Get a reference to the latest GOOSE PDU
    pub fn get_goosepdu(&self) -> &IECGoosePdu {
        &self.goosepdu
    }

    /// Get a reference to the allData field from the GOOSE PDU
    pub fn get_alldata(&self) -> &Vec<IECData> {
        &self.goosepdu.allData
    }

    /// Get the timestamp of the last update
    pub fn get_last_update(&self) -> Option<std::time::SystemTime> {
        self.last_update
    }

    /// Force invalidate the data (useful for error conditions)
    pub fn invalidate_data(&mut self) {
        self.statevalid = false;
        self.invaliditytime = Some(std::time::SystemTime::now());
    }

    /// Check and update data validity based on current time
    /// Returns true if data was valid before the check, false if it was already invalid or became invalid
    pub fn check_and_update_validity(&mut self, lan_id: u8) -> bool {
        let was_valid = self.is_data_valid();

        // If data has expired based on invaliditytime, mark it as invalid
        if let Some(invalid_time) = self.invaliditytime {
            let now = std::time::SystemTime::now();
            if now >= invalid_time {
                let overdue_duration = now.duration_since(invalid_time)
                    .unwrap_or_default();
                let overdue_secs = overdue_duration.as_secs_f64();
                warn!(
                    "Data for LAN{} PCS ID: {} (APPID: {:04X}) has become invalid due to timeout - expired {}s ago (invalid_time: {:?}, now: {:?})",
                    lan_id,
                    self.pcs_id(),
                    self.nameplate_appid().unwrap_or(0),
                    overdue_secs,
                    invalid_time,
                    now
                );
                self.statevalid = false;
            }
        } else if let Some(last_update) = self.last_update {
            // Fallback check: if no invaliditytime but we have last_update, check against default timeout
            let now = std::time::SystemTime::now();
            let elapsed = now
                .duration_since(last_update)
                .unwrap_or(std::time::Duration::from_secs(u64::MAX));
            if elapsed >= std::time::Duration::from_secs(10) {
                // Default 10 second timeout
                let elapsed_secs = elapsed.as_secs_f64();
                warn!(
                    "Data for LAN{} PCS ID: {} (APPID: {:04X}) has become invalid due to 10s timeout - {}s since last update (last_update: {:?}, now: {:?})",
                    lan_id,
                    self.pcs_id(),
                    self.nameplate_appid().unwrap_or(0),
                    elapsed_secs,
                    last_update,
                    now
                );
                self.statevalid = false;
            }
        }

        was_valid
    }

    /// Get seconds until data becomes invalid (negative if already invalid)
    pub fn seconds_until_invalid(&self) -> Option<i64> {
        if let Some(invalid_time) = self.invaliditytime {
            match invalid_time.duration_since(std::time::SystemTime::now()) {
                Ok(duration) => Some(duration.as_secs() as i64),
                Err(_) => Some(
                    -(std::time::SystemTime::now()
                        .duration_since(invalid_time)
                        .unwrap_or_default()
                        .as_secs() as i64),
                ),
            }
        } else {
            None
        }
    }

    /// Get current GOOSE sequence information for monitoring
    /// Returns (stNum, sqNum)
    pub fn get_goose_sequence(&self) -> (u32, u32) {
        (self.goosepdu.stNum, self.goosepdu.sqNum)
    }

    /// Check if the GOOSE data has valid sequence numbers
    /// This checks the internal statevalid flag which is set based on sequence validation
    pub fn is_sequence_valid(&self) -> bool {
        self.statevalid
    }
}

// make a enum for pcs status : 1-stop , 2-standby , 3-charging , 4-discharging , 5-fault, 6-zero power
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PCSStatus {
    Stop = 1,
    Standby,
    Charging,
    Discharging,
    Fault,
    ZeroPower,
}
impl PCSStatus {
    pub fn from_u8(value: u8) -> Option<PCSStatus> {
        match value {
            1 => Some(PCSStatus::Stop),
            2 => Some(PCSStatus::Standby),
            3 => Some(PCSStatus::Charging),
            4 => Some(PCSStatus::Discharging),
            5 => Some(PCSStatus::Fault),
            6 => Some(PCSStatus::ZeroPower),
            _ => None,
        }
    }
}
