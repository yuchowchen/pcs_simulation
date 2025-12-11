use crate::goose::types::{EthernetHeader, IECData, IECGoosePdu};
use anyhow::Result;
use log::{info,warn};
use std::collections::HashMap;
use crate::pcs::nameplate::NameplateConfig;
use crate::pcs::{PcsTypeMapping, init_goose_frame_for_pcs, publisher};
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PublisherPcsData {
    /// Nameplate/configuration values for this PCS (includes device id)
    
    pub pcs_mapping: HashMap<u16, Vec<(EthernetHeader, IECGoosePdu)>>, // logical ID and goose frame to be sent from this pcs.
}

impl PublisherPcsData {
    pub fn new(mut self,config:&Vec<NameplateConfig>,pcs_type:&PcsTypeMapping)  {
        for cfg in config {
           let gooseframe =init_goose_frame_for_pcs(cfg, pcs_type);
              match gooseframe {
                Ok(frame) => {
                     info!("Initialized GOOSE frame for PCS with logical ID: {:?}", cfg.logical_id);
                     // Here we assume that cfg.device_id is u16, adjust if necessary
                     // Also assuming that frame is of type (EthernetHeader, IECGoosePdu)
                     // You might need to adjust this based on actual types
                     // For demonstration, we use a placeholder logical ID
                     let logical_id = cfg.logical_id.unwrap() as u16; 
                     self.pcs_mapping.entry(logical_id).or_insert_with(Vec::new).push(frame);
                },
                Err(e) => {
                     warn!("Failed to initialize GOOSE frame for PCS with device ID {:?}: {}", cfg.logical_id, e);
                }
              }
        }   
        
    }

}