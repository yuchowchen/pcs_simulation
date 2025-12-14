// recieve goose commnad from PMS and mapping command to each pcs. acitve power enable /disable, reactive power enable/disable, acitve power setpoint, reacitve power set point etc.

use crate::goose::types::IECGoosePdu;
use crate::pcs::nameplate::NameplateConfig;
use anyhow::Result;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct PmsConfig {
    pub pms_command_appid_list: Vec<u16>, // list of APPIDs to subscribe to PMS GOOSE commands  convert to u16 when used.
    pub pms_command_pcs_mapping: HashMap<u16, Vec<u16>>, // mapping from PMS GOOSE APPID to list of PCS IDs <command_appid, vec![pcs_logical_id1, pcs_logical_  id2,...]>
                                                         // the default mapping is: boolean_enable_active_power_control_pcs1, boolean_enable_reactive_power_control_pcs1,etc... pcs_n, float_active_power_setpoint_pcs1, float_reactive_power_setpoint_pcs1,etc... pcs_n.
}

// create PmsConfig instance from NamplateConfig vecotr
impl PmsConfig {
    pub fn load_pms_configs(nameplate_configs: &Vec<NameplateConfig>) -> Result<Self> {
        let mut pms_command_pcs_mapping: HashMap<u16, Vec<u16>> = HashMap::new();

        // Iterate through all nameplate configs and group PCS logical_ids by pms_appid
        for config in nameplate_configs.iter() {
            if let Some(pms_appid) = config.pms_appid {
                // Get the PCS logical_id (use from config, not index)
                if let Some(pcs_logical_id) = config.logical_id {
                    // Add this PCS logical_id to the vector for this pms_appid
                    pms_command_pcs_mapping
                        .entry(pms_appid)
                        .or_insert_with(Vec::new)
                        .push(pcs_logical_id);

                    info!(
                        "Mapped PCS logical_id {} to PMS APPID 0x{:04X}",
                        pcs_logical_id, pms_appid
                    );
                } else {
                    error!(
                        "Nameplate config for device_id {:?} has pms_appid but missing logical_id",
                        config.device_id
                    );
                }
            } else {
                info!(
                    "Nameplate config for device_id {:?} has no pms_appid (not controlled by PMS)",
                    config.device_id
                );
            }
        }

        // Sort each vector of PCS logical_ids within the mapping
        for pcs_ids in pms_command_pcs_mapping.values_mut() {
            pcs_ids.sort();
        }

        // Extract unique pms_appid values from the mapping keys
        let mut pms_command_appid_list: Vec<u16> =
            pms_command_pcs_mapping.keys().copied().collect();
        pms_command_appid_list.sort(); // Sort for consistent ordering

        info!(
            "Created PmsConfig with {} unique PMS APPIDs: {:?}",
            pms_command_appid_list.len(),
            pms_command_appid_list
                .iter()
                .map(|&id| format!("0x{:04X}", id))
                .collect::<Vec<_>>()
        );

        // Log the mapping for each PMS APPID
        for (appid, pcs_ids) in &pms_command_pcs_mapping {
            info!(
                "PMS APPID 0x{:04X} controls {} PCS units (sorted): {:?}",
                appid,
                pcs_ids.len(),
                pcs_ids
            );
        }

        Ok(PmsConfig {
            pms_command_appid_list,
            pms_command_pcs_mapping,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PmsGooseCmdSubscriber {
    // pub LAN_id: u16,
    pub goose_appid: u16,
    pub goosepdu: IECGoosePdu, // received GOOSE PDU from PMS
    pub last_update_time: Option<std::time::SystemTime>, // timestamp of last update
    pub invalidity_time: Option<std::time::SystemTime>, // timestamp of invalidity time used for timeout detection
    pub invalid: bool, // flag to indicate if the command is invalid
}
