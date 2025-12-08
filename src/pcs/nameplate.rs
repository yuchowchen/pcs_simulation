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
    pub goose_appid: Option<String>,

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
}

/// Load nameplate configurations from a CSV file with headers.
/// Expected headers: no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,
///                   goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,
///                   goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type
/// The 'no' column is a row number for human reference and is optional.
pub fn load_nameplates_from_csv<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<NameplateConfig>> {
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open nameplate CSV file '{:?}': {}", path.as_ref(), e);
            anyhow::bail!("Failed to open nameplate CSV file '{:?}': {}", path.as_ref(), e);
        }
    };
    let mut rdr = csv::Reader::from_reader(file);
    let mut configs = Vec::new();
    // track seen numeric identifiers to enforce uniqueness
    let mut seen_goose: HashSet<String> = HashSet::new();
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

                // Normalize goose_appid
                if let Some(s) = record.goose_appid.take() {
                    let t = s.trim();
                    if t.is_empty() {
                        record.goose_appid = None;
                    } else {
                        record.goose_appid = Some(t.to_string());
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

                if let Some(ref appid_str) = record.goose_appid {
                    if appid_str.is_empty() {
                        error!("CSV row {}: goose_appid is empty. Skipping row.", row_num);
                        bad = true;
                    } else if seen_goose.contains(appid_str) {
                        error!("CSV row {}: duplicate goose_appid {} found (must be unique). Skipping row.", row_num, appid_str);
                        bad = true;
                    }
                }

                if let Some(logical_id) = record.logical_id {
                    if logical_id == 0 {
                        error!("CSV row {}: logical_id value 0 is invalid (must be >= 1). Skipping row.", row_num);
                        bad = true;
                    } else if seen_logical.contains(&logical_id) {
                        error!("CSV row {}: duplicate logical_id {} found (must be unique). Skipping row.", row_num, logical_id);
                        bad = true;
                    }
                }

                // Validate pcs_type is configured (after normalization)
                if record.pcs_type.is_none() {
                    error!("CSV row {}: pcs_type is not configured (must be present and non-empty). Skipping row.", row_num);
                    bad = true;
                }

                if bad {
                    // Skip invalid row but continue parsing remainder of file
                    continue;
                }

                // Validate feed_line_id numeric value if present (must be > 0)
                if let Some(fid) = record.feed_line_id {
                    if fid == 0 {
                        error!("CSV row {}: feed_line_id value 0 is invalid (must be > 0). Skipping row.", row_num);
                        continue;
                    }
                }

                // Record identifiers as seen so future rows can be checked
                if let Some(ref appid_str) = record.goose_appid {
                    seen_goose.insert(appid_str.clone());
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



#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_nameplates_from_csv_basic() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,01:0C:CD:01:00:01,01:0C:CD:FF:FF:FF,0x8100,0x8002,FLEXPIGO/LLN0$GO$gocb1,FLEXPIGO/LLN0$dataset1,FLEXPIGO/LLN0$GO$gocb1,false,1,false,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,0x8802,01:0C:CD:01:00:02,01:0C:CD:FF:FF:FF,0x8100,0x8002,,,,,,,102,,2,PCS-B").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 2);
        
        // First record - all fields populated
        assert_eq!(configs[0].row_number, Some(1));
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
        assert_eq!(configs[0].goose_appid.as_deref(), Some("0x8801"));
        assert_eq!(configs[0].goose_src_addr.as_deref(), Some("01:0C:CD:01:00:01"));
        assert_eq!(configs[0].goose_dst_addr.as_deref(), Some("01:0C:CD:FF:FF:FF"));
        assert_eq!(configs[0].feed_line_id, Some(101));
        assert_eq!(configs[0].feed_line_alias.as_deref(), Some("Line1"));
        assert_eq!(configs[0].logical_id, Some(1));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-A"));
        
        // Second record - minimal GOOSE fields
        assert_eq!(configs[1].row_number, Some(2));
        assert_eq!(configs[1].device_id.as_deref(), Some("devB"));
        assert_eq!(configs[1].goose_appid.as_deref(), Some("0x8802"));
        assert!(configs[1].feed_line_alias.is_none());
        assert_eq!(configs[1].logical_id, Some(2));
        assert_eq!(configs[1].pcs_type.as_deref(), Some("PCS-B"));
    }

    #[test]
    fn test_duplicate_goose_appid_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,0x8801,,,,,,,,,,,102,Line2,2,PCS-B").unwrap(); // Duplicate goose_appid

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only first record should be loaded, duplicate should be skipped
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
    }

    #[test]
    fn test_duplicate_logical_id_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,0x8802,,,,,,,,,,,102,Line2,1,PCS-B").unwrap(); // Duplicate logical_id=1

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only first record should be loaded, duplicate should be skipped
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
    }

    #[test]
    fn test_zero_values_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,,,,,,,,,,,,0,Line2,2,PCS-B").unwrap(); // feed_line_id=0 invalid
        writeln!(tmpfile, "2,devC,0x8803,,,,,,,,,,,103,Line3,0,PCS-C").unwrap(); // logical_id=0 invalid
        writeln!(tmpfile, "3,devD,0x8804,,,,,,,,,,,104,Line4,4,PCS-D").unwrap(); // Valid

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only the last valid record should be loaded
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devD"));
        assert_eq!(configs[0].goose_appid.as_deref(), Some("0x8804"));
        assert_eq!(configs[0].logical_id, Some(4));
    }

    #[test]
    fn test_missing_pcs_type_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,101,Line1,1,").unwrap(); // Empty pcs_type
        writeln!(tmpfile, "2,devB,0x8802,,,,,,,,,,,102,Line2,2,  ").unwrap(); // Whitespace-only pcs_type
        writeln!(tmpfile, "3,devC,0x8803,,,,,,,,,,,103,Line3,3,PCS-C").unwrap(); // Valid with pcs_type

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only the last valid record with pcs_type should be loaded
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devC"));
        assert_eq!(configs[0].goose_appid.as_deref(), Some("0x8803"));
        assert_eq!(configs[0].logical_id, Some(3));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-C"));
    }

    #[test]
    fn test_empty_string_normalization() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,  ,0x8801,,,,,,,,,,,101,  ,1,PCS-A").unwrap(); // Empty/whitespace-only strings

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 1);
        // Empty/whitespace-only device_id should be None
        assert!(configs[0].device_id.is_none());
        // Empty/whitespace-only feed_line_alias should be None
        assert!(configs[0].feed_line_alias.is_none());
        assert_eq!(configs[0].goose_appid.as_deref(), Some("0x8801"));
        assert_eq!(configs[0].logical_id, Some(1));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-A"));
    }

    #[test]
    fn test_optional_fields() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        // Record with only some fields populated
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,,,2,PCS-A").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
        assert_eq!(configs[0].goose_appid.as_deref(), Some("0x8801"));
        assert!(configs[0].feed_line_id.is_none());
        assert!(configs[0].feed_line_alias.is_none());
        assert_eq!(configs[0].logical_id, Some(2));
    }

    #[test]
    fn test_malformed_row_continues() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,invalid,extra,data,here,,,,,,,102,Line2,2,PCS-B").unwrap(); // Malformed
        writeln!(tmpfile, "3,devC,0x8803,,,,,,,,,,,103,Line3,3,PCS-C").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Should skip malformed row but continue with valid ones
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
        assert_eq!(configs[1].device_id.as_deref(), Some("devC"));
    }

    #[test]
    fn test_row_number_column_optional_values() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,goose_srcAddr,goose_dstAddr,goose_TPID,goose_TCI,goose_gocbRef,goose_dataSet,goose_goID,goose_simulation,goose_confRev,goose_ndsCom,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0x8801,,,,,,,,,,,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, ",devB,0x8802,,,,,,,,,,,102,Line2,2,PCS-B").unwrap(); // Empty row number
        writeln!(tmpfile, "3,devC,0x8803,,,,,,,,,,,103,Line3,3,PCS-C").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 3);
        
        // Row numbers can be present or absent
        assert_eq!(configs[0].row_number, Some(1));
        assert_eq!(configs[1].row_number, None); // Empty row number
        assert_eq!(configs[2].row_number, Some(3));
        
        // All other fields should load correctly
        assert_eq!(configs[1].device_id.as_deref(), Some("devB"));
        assert_eq!(configs[1].goose_appid.as_deref(), Some("0x8802"));
    }
}
