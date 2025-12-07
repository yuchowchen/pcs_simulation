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

    /// Configured GOOSE APPID (u16) from pcs manufacturer
    /// goose will subscriber it by this id.
    pub goose_appid: Option<u16>,

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
/// Expected headers: no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type
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

                if let Some(appid) = record.goose_appid {
                    if appid == 0 {
                        error!("CSV row {}: goose_appid value 0 is invalid (must be > 0). Skipping row.", row_num);
                        bad = true;
                    } else if seen_goose.contains(&appid) {
                        error!("CSV row {}: duplicate goose_appid {} found (must be unique). Skipping row.", row_num, appid);
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



#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_nameplates_from_csv_basic() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,9,102,,2,PCS-B").unwrap();
        writeln!(tmpfile, "3,  devC  ,10,103,Line3,3,  PCS-C  ").unwrap(); // Test trimming

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 3);
        
        // First record - all fields populated
        assert_eq!(configs[0].row_number, Some(1));
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
        assert_eq!(configs[0].goose_appid, Some(8));
        assert_eq!(configs[0].feed_line_id, Some(101));
        assert_eq!(configs[0].feed_line_alias.as_deref(), Some("Line1"));
        assert_eq!(configs[0].logical_id, Some(1));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-A"));
        
        // Second record - missing feed_line_alias
        assert_eq!(configs[1].row_number, Some(2));
        assert_eq!(configs[1].device_id.as_deref(), Some("devB"));
        assert_eq!(configs[1].goose_appid, Some(9));
        assert_eq!(configs[1].feed_line_id, Some(102));
        assert!(configs[1].feed_line_alias.is_none());
        assert_eq!(configs[1].logical_id, Some(2));
        assert_eq!(configs[1].pcs_type.as_deref(), Some("PCS-B"));
        
        // Third record - test string trimming
        assert_eq!(configs[2].row_number, Some(3));
        assert_eq!(configs[2].device_id.as_deref(), Some("devC"));
        assert_eq!(configs[2].goose_appid, Some(10));
        assert_eq!(configs[2].logical_id, Some(3));
        assert_eq!(configs[2].pcs_type.as_deref(), Some("PCS-C")); // Trimmed from "  PCS-C  "
    }

    #[test]
    fn test_duplicate_goose_appid_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,8,102,Line2,2,PCS-B").unwrap(); // Duplicate goose_appid=8

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
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,9,102,Line2,1,PCS-B").unwrap(); // Duplicate logical_id=1

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
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,0,101,Line1,1,PCS-A").unwrap(); // goose_appid=0 invalid
        writeln!(tmpfile, "2,devB,8,0,Line2,2,PCS-B").unwrap(); // feed_line_id=0 invalid
        writeln!(tmpfile, "3,devC,9,103,Line3,0,PCS-C").unwrap(); // logical_id=0 invalid
        writeln!(tmpfile, "4,devD,10,104,Line4,4,PCS-D").unwrap(); // Valid

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only the last valid record should be loaded
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devD"));
        assert_eq!(configs[0].goose_appid, Some(10));
        assert_eq!(configs[0].logical_id, Some(4));
    }

    #[test]
    fn test_missing_pcs_type_rejected() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,").unwrap(); // Empty pcs_type
        writeln!(tmpfile, "2,devB,9,102,Line2,2,  ").unwrap(); // Whitespace-only pcs_type
        writeln!(tmpfile, "3,devC,10,103,Line3,3,PCS-C").unwrap(); // Valid with pcs_type

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        // Only the last valid record with pcs_type should be loaded
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devC"));
        assert_eq!(configs[0].goose_appid, Some(10));
        assert_eq!(configs[0].logical_id, Some(3));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-C"));
    }


    #[test]
    fn test_empty_string_normalization() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,  ,8,101,  ,1,PCS-A").unwrap(); // Empty/whitespace-only strings

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 1);
        // Empty/whitespace-only device_id should be None
        assert!(configs[0].device_id.is_none());
        // Empty/whitespace-only feed_line_alias should be None
        assert!(configs[0].feed_line_alias.is_none());
        assert_eq!(configs[0].goose_appid, Some(8));
        assert_eq!(configs[0].logical_id, Some(1));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-A"));
    }

    #[test]
    fn test_optional_fields() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        // Record with only some fields populated
        writeln!(tmpfile, "1,devA,8,,,2,PCS-A").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].device_id.as_deref(), Some("devA"));
        assert_eq!(configs[0].goose_appid, Some(8));
        assert!(configs[0].feed_line_id.is_none());
        assert!(configs[0].feed_line_alias.is_none());
        assert_eq!(configs[0].logical_id, Some(2));
    }

    #[test]
    fn test_malformed_row_continues() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmpfile,
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, "2,devB,invalid,102,Line2,2,PCS-B").unwrap(); // Invalid goose_appid
        writeln!(tmpfile, "3,devC,10,103,Line3,3,PCS-C").unwrap();

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
            "no,device_id,goose_appid,feed_line_id,feed_line_alias,logical_id,pcs_type"
        )
        .unwrap();
        writeln!(tmpfile, "1,devA,8,101,Line1,1,PCS-A").unwrap();
        writeln!(tmpfile, ",devB,9,102,Line2,2,PCS-B").unwrap(); // Empty row number
        writeln!(tmpfile, "3,devC,10,103,Line3,3,PCS-C").unwrap();

        let configs = load_nameplates_from_csv(tmpfile.path()).unwrap();
        assert_eq!(configs.len(), 3);
        
        // Row numbers can be present or absent
        assert_eq!(configs[0].row_number, Some(1));
        assert_eq!(configs[1].row_number, None); // Empty row number
        assert_eq!(configs[2].row_number, Some(3));
        
        // All other fields should load correctly
        assert_eq!(configs[1].device_id.as_deref(), Some("devB"));
        assert_eq!(configs[1].goose_appid, Some(9));
    }

    #[test]
    fn test_load_actual_pcs_csv() {
        // Test loading the actual pcs.csv file if it exists
        if std::path::Path::new("pcs.csv").exists() {
            let configs = load_nameplates_from_csv("pcs.csv").unwrap();
            
            // Should have 40 entries
            assert_eq!(configs.len(), 40);
            
            // Check first entry
            assert_eq!(configs[0].row_number, Some(1));
            assert_eq!(configs[0].device_id.as_deref(), Some("20250101"));
            assert_eq!(configs[0].goose_appid, Some(8801));
            assert_eq!(configs[0].feed_line_id, Some(1));
            assert_eq!(configs[0].logical_id, Some(1));
        assert_eq!(configs[0].pcs_type.as_deref(), Some("PCS-A"));
            
            // Check last entry
            assert_eq!(configs[39].row_number, Some(40));
            assert_eq!(configs[39].device_id.as_deref(), Some("20254352"));
            assert_eq!(configs[39].goose_appid, Some(8840));
            assert_eq!(configs[39].feed_line_id, Some(4));
            assert_eq!(configs[39].logical_id, Some(40));
        assert_eq!(configs[39].pcs_type.as_deref(), Some("PCS-A"));
            
            println!("✅ Successfully loaded and validated pcs.csv with {} entries", configs.len());
        } else {
            println!("⚠️  Skipping pcs.csv test - file not found");
        }
    }
}
