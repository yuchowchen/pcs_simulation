# PCS Publisher Update Summary

## Overview
Updated the PCS publisher system to support per-PCS GOOSE frame generation with type-specific allData field mappings.

## Key Changes

### 1. New Publisher Architecture (`src/pcs/publisher.rs`)

#### **Per-PCS GOOSE Frames**
- Changed from grouped frames to individual frames per PCS
- Each PCS now has its own GOOSE frame based on its nameplate configuration
- No more shared frames or grouping by APPID

#### **Type-Specific Field Mappings**
- Implemented `PcsTypeMapping` struct to load field configurations from JSON
- Function `load_pcs_type_mappings()` reads `PCS_publisher_alldata_mapping.json`
- Supports 3 PCS types:
  - **PCS-A**: 21 fields (3 boolean + 17 float + 1 int)
  - **PCS-B**: 25 fields (3 boolean + 21 float + 1 int)
  - **PCS-C**: 25 fields (2 boolean + 22 float + 1 int)

#### **New Functions**

```rust
// Load PCS type mappings from JSON file
pub fn load_pcs_type_mappings(path: &str) -> Result<HashMap<String, PcsTypeMapping>>

// Initialize GOOSE frame for a single PCS from its nameplate configuration
pub fn init_goose_frame_for_pcs(
    nameplate: &NameplateConfig,
    type_mapping: &PcsTypeMapping,
) -> Result<GooseFrame>

// Update GOOSE frame allData with current PCS data
pub fn update_goose_frame_data(
    frame: &mut GooseFrame,
    pcs_data: &PublisherPcsData,
    type_mapping: &PcsTypeMapping,
) -> Result<()>
```

#### **Helper Functions**
- `parse_mac()`: Supports multiple MAC address formats (colon, dash, continuous hex)
- `parse_hex_u16()`: Parses hex strings with or without "0x" prefix

### 2. Enhanced Data Structures (`src/pcs/types.rs`)

#### **Added Core Types**
- `AppIdIndex`: Maps GOOSE APPID to (logical_id, pcs_type) for fast packet processing
- `MutablePcsData`: Thread-safe storage using DashMap for concurrent access
- `ProcessData`: Container for AppIdIndex and MutablePcsData

#### **New Methods on MutablePcsData**
- `update_with_index()`: Update PCS data from GOOSE PDU
- `check_validity_by_lan()`: Periodic validity checks per LAN
- `check_validity_both_lans()`: Check both LANs simultaneously
- `get_validity_stats_by_lan()`: Get valid/invalid counts
- `get_validity_stats_both_lans()`: Stats for both LANs
- `get_invalid_pcs_by_lan()`: List of invalid PCS units

#### **ProcessData Methods**
- `init_from_nameplates()`: Load PCS configurations from CSV
- `into_components()`: Decompose for Arc-wrapping
- `from_components()`: Reconstruct from components

### 3. Updated Module Structure (`src/pcs/mod.rs`)

New exports:
```rust
pub use types::{PublisherPcsData, AppIdIndex, MutablePcsData, ProcessData};
pub use publisher::{
    load_pcs_type_mappings, 
    init_goose_frame_for_pcs, 
    update_goose_frame_data, 
    GooseFrame, 
    PcsTypeMapping
};
```

## Usage Example

```rust
// Load PCS type mappings
let type_mappings = load_pcs_type_mappings("PCS_publisher_alldata_mapping.json")?;

// Load nameplate configurations
let nameplates = load_nameplates_from_csv("pcs.csv")?;

// For each PCS, initialize its GOOSE frame
for nameplate in nameplates {
    let pcs_type = nameplate.pcs_type.as_ref().unwrap();
    let type_mapping = type_mappings.get(pcs_type).unwrap();
    
    // Initialize frame with type-specific allData fields
    let frame = init_goose_frame_for_pcs(&nameplate, type_mapping)?;
    
    // Later, update frame with real-time PCS data
    update_goose_frame_data(&mut frame, &pcs_data, type_mapping)?;
}
```

## Configuration Files

### PCS_publisher_alldata_mapping.json
Defines field layouts for each PCS type:
```json
[
  {
    "pcstype": "PCS-A",
    "spare1": "boolean",
    "spare2": "boolean",
    "spare3": "boolean",
    "pcs_realtime_active_power_pos": "float",
    ...
    "pcs_status_pos": "int"
  }
]
```

### pcs.csv
Each PCS has individual GOOSE configuration:
- `goose_appid`: Application ID (hex format supported)
- `goose_srcAddr`, `goose_dstAddr`: MAC addresses
- `goose_TPID`, `goose_TCI`: VLAN tags
- `goose_gocbRef`, `goose_dataSet`, `goose_goID`: IEC 61850 identifiers
- `goose_simulation`, `goose_confRev`, `goose_ndsCom`: GOOSE parameters
- `pcs_type`: Type identifier (PCS-A/B/C)

## Benefits

1. **Type Safety**: Each PCS type has correct number and types of allData fields
2. **Flexibility**: Easy to add new PCS types or modify field layouts via JSON
3. **Scalability**: Individual frames scale better than grouped frames
4. **Clarity**: Each PCS configuration is self-contained in CSV
5. **Maintainability**: Field mappings separated from code logic

## Compilation Status

✅ Project compiles successfully with zero warnings
✅ All existing tests pass
✅ Thread-safe concurrent access patterns preserved

## Next Steps

To use the new publisher system:

1. Load PCS type mappings at startup
2. Initialize GOOSE frames for each PCS from nameplate
3. Store frames in a HashMap keyed by logical_id
4. Update frames periodically with real-time PCS data
5. Encode and transmit updated frames on network interfaces

The publisher.rs provides the building blocks - integration into the main publishing loop is the next phase.
