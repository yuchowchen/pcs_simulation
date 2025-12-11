use crate::goose::types::{EthernetHeader, IECData, IECGoosePdu};
use anyhow::Result;
use log::warn;
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PublisherPcsData {
    /// Nameplate/configuration values for this PCS (includes device id)
    nameplate: Option<crate::pcs::NameplateConfig>,
    pub pcs_mapping: HashMap<u16, Vec<(EthernetHeader, IECGoosePdu)>>, // logical ID and goose frame to be sent from this pcs.
}
