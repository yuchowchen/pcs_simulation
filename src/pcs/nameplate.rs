// rated active power and reactive power, manufacturer, type, year of manufacture defined by csv file
// configure goose APPID
// configure feed line ID
use anyhow::Result;
use log::{error, info};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

/// Represents configurable nameplate values for a PCS device.
#[derive(Debug, Deserialize, Clone)]
pub struct NameplateConfig {
    /// Row number from CSV (first column, for human reference only)
    #[serde(rename = "no")]
    pub row_number: Option<u16>,

    /// Human readable device id (matches CSV header `device_id`)
    /// Optional: missing/empty device ids are represented as `None`.
    pub device_id: Option<String>,

    /// Configured GOOSE APPID (hex string or decimal) from pcs manufacturer
    /// goose will subscriber it by this id.
    pub goose_appid: Option<u16>,

    /// GOOSE source MAC address
    #[serde(rename = "goose_srcAddr")]
    pub goose_src_addr: Option<String>,

    /// GOOSE destination MAC address
    #[serde(rename = "goose_dstAddr")]
    pub goose_dst_addr: Option<String>,

    /// GOOSE TPID (Tag Protocol Identifier)
    #[serde(rename = "goose_TPID")]
    pub goose_tpid: Option<String>,

    /// GOOSE TCI (Tag Control Information)
    #[serde(rename = "goose_TCI")]
    pub goose_tci: Option<String>,

    /// GOOSE gocbRef (GOOSE Control Block Reference)
    #[serde(rename = "goose_gocbRef")]
    pub goose_gocb_ref: Option<String>,

    /// GOOSE dataSet
    #[serde(rename = "goose_dataSet")]
    pub goose_data_set: Option<String>,

    /// GOOSE goID (GOOSE Identifier)
    #[serde(rename = "goose_goID")]
    pub goose_go_id: Option<String>,

    /// GOOSE simulation flag
    pub goose_simulation: Option<String>,

    /// GOOSE confRev (Configuration Revision)
    #[serde(rename = "goose_confRev")]
    pub goose_conf_rev: Option<String>,

    /// GOOSE ndsCom (needs commission)
    #[serde(rename = "goose_ndsCom")]
    pub goose_nds_com: Option<String>,

    /// Identifier for the feed line (numeric id from CSV)
    pub feed_line_id: Option<u16>,

    /// Optional feed line alias
    pub feed_line_alias: Option<String>,

    ///  software/app id start from 1 to 65535 must be unique for each PCS
    pub logical_id: Option<u16>,

    /// pcs type from manufacturer to identify different pcs model goose message structure
    pub pcs_type: Option<String>,

    /// pms appid list for this pcs
    pub pms_appid: Option<u16>,
}

/// Load nameplate configurations from a CSV file with headers.
/// Expected headers: no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,
///                   goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,
///                   goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type, pms_appid
/// The 'no' column is a row number for human reference and is optional.
pub fn load_nameplates_from_csv<P: AsRef<Path>>(path: P) -> Result<Vec<NameplateConfig>> {
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            error!(
                "Failed to open nameplate CSV file '{:?}': {}",
                path.as_ref(),
                e
            );
            anyhow::bail!(
                "Failed to open nameplate CSV file '{:?}': {}",
                path.as_ref(),
                e
            );
        }
    };
    let mut rdr = csv::Reader::from_reader(file);
    let mut configs = Vec::new();
    // track seen numeric identifiers to enforce uniqueness
    let mut seen_goose: HashSet<u16> = HashSet::new();
    let mut seen_logical: HashSet<u16> = HashSet::new();

    for (idx, result) in rdr.deserialize::<NameplateConfig>().enumerate() {
        let row_num = idx + 1; // 1-based for human readable logs (first data row)
        match result {
            Ok(mut record) => {
                // First normalize string fields: trim and convert empty -> None

                // Normalize device_id
                if let Some(s) = record.device_id.take() {
                    let t = s.trim();
                    if t.is_empty() {
                        record.device_id = None;
                    } else {
                        record.device_id = Some(t.to_string());
                    }
                }

                // Normalize all GOOSE string fields
                let goose_fields = [
                    &mut record.goose_src_addr,
                    &mut record.goose_dst_addr,
                    &mut record.goose_tpid,
                    &mut record.goose_tci,
                    &mut record.goose_gocb_ref,
                    &mut record.goose_data_set,
                    &mut record.goose_go_id,
                    &mut record.goose_simulation,
                    &mut record.goose_conf_rev,
                    &mut record.goose_nds_com,
                ];

                for field in goose_fields {
                    if let Some(s) = field.take() {
                        let t = s.trim();
                        if t.is_empty() {
                            *field = None;
                        } else {
                            *field = Some(t.to_string());
                        }
                    }
                }

                // Normalize feed_line_alias
                if let Some(alias) = record.feed_line_alias.take() {
                    let t = alias.trim();
                    if t.is_empty() {
                        record.feed_line_alias = None;
                    } else {
                        record.feed_line_alias = Some(t.to_string());
                    }
                }

                // Normalize pcs_type
                if let Some(pcs_type) = record.pcs_type.take() {
                    let t = pcs_type.trim();
                    if t.is_empty() {
                        record.pcs_type = None;
                    } else {
                        record.pcs_type = Some(t.to_string());
                    }
                }

                // Now perform validation after normalization
                let mut bad = false;

                // Normalize goose_appid
                if let Some(appid) = record.goose_appid.take() {
                    if (appid == 0) || (appid > 0xFFFF) {
                        record.goose_appid = None;
                        error!(
                            "CSV row {}: goose_appid value 0 or out of range (must be 1-65535). Skipping row.",
                            row_num
                        );
                    } else if seen_goose.contains(&appid) {
                        error!(
                            "CSV row {}: duplicate goose_appid {} found (must be unique). Skipping row.",
                            row_num, appid);
                        bad = true;
                    } else {
                        // Validation passed - restore the value
                        record.goose_appid = Some(appid);
                    }

                }

                // Normalize pms_appid
                if let Some(appid) = record.pms_appid.take() {
                    if appid == 0 {
                        record.pms_appid = None; // 0 is invalid, treat as None
                        error!("CSV row {}: pms_appid is empty. Skipping row.", row_num);
                        bad = true;
                    } else {
                        record.pms_appid = Some(appid);
                    }
                }

                if let Some(logical_id) = record.logical_id {
                    if logical_id == 0 {
                        error!(
                            "CSV row {}: logical_id value 0 is invalid (must be >= 1). Skipping row.",
                            row_num
                        );
                        bad = true;
                    } else if seen_logical.contains(&logical_id) {
                        error!(
                            "CSV row {}: duplicate logical_id {} found (must be unique). Skipping row.",
                            row_num, logical_id
                        );
                        bad = true;
                    }
                }

                // Validate pcs_type is configured (after normalization)
                if record.pcs_type.is_none() {
                    error!(
                        "CSV row {}: pcs_type is not configured (must be present and non-empty). Skipping row.",
                        row_num
                    );
                    bad = true;
                }

                if bad {
                    // Skip invalid row but continue parsing remainder of file
                    continue;
                }

                // Validate feed_line_id numeric value if present (must be > 0)
                if let Some(fid) = record.feed_line_id {
                    if fid == 0 {
                        error!(
                            "CSV row {}: feed_line_id value 0 is invalid (must be > 0). Skipping row.",
                            row_num
                        );
                        continue;
                    }
                }

                // Record identifiers as seen so future rows can be checked
                if let Some(appid) = record.goose_appid {
                    seen_goose.insert(appid);
                }
                if let Some(logical_id) = record.logical_id {
                    seen_logical.insert(logical_id);
                }


                configs.push(record);
            }
            Err(e) => {
                // Log parse errors and continue
                error!(
                    "Failed to deserialize CSV row {}: {}. Skipping row.",
                    row_num, e
                );
                continue;
            }
        }
    }

    info!("Loaded {} valid nameplate entries from CSV", configs.len());
    Ok(configs)
}

