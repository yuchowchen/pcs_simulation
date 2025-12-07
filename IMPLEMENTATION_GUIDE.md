# PCS Simulator - Command Processing Implementation Guide

## Overview

Implementation of **README Requirement #5**:
> "Each PCS will decode the received GOOSE commands and update the active power and reactive power feedback according to setpoints accordingly"

## Files Created

### 1. `src/main_enhanced.rs` - Enhanced Main Entry Point

**Key Features:**
- Loads `PCS_subscriber_mapping.json` (maps APPID → numberOfPcs for command frames)
- Passes command mapping to worker threads
- Integrates command extraction into packet processing flow

**New Function:**
```rust
fn load_subscriber_config(path: String) -> Result<HashMap<u16, usize>>
```

### 2. `src/threads/worker_enhanced.rs` - Worker Threads with Command Extraction

**Key Features:**
- After updating PCS data from GOOSE, checks if frame is a command
- Extracts power setpoints for each PCS unit
- Updates feedback values based on commands

**Command Extraction Flow:**
1. Decode GOOSE frame
2. Update PCS data (`update_with_index()`)
3. Check if APPID is in command mapping
4. If yes → extract commands → update feedback

### 3. `src/pcs/types.rs` - Already Modified

**Added to SubscriberPCSData:**
- `active_power_feedback: f32`
- `reactive_power_feedback: f32`
- `active_power_enable: bool`
- `reactive_power_enable: bool`

**New Methods:**
- `extract_and_apply_commands()` - Extracts commands from GOOSE allData
- `get_feedback_values()` - Returns current feedback values

## GOOSE allData Format

For N PCS units:
```
Indices 0 to N-1:       active_power_enable[0..N]     (bool)
Indices N to 2N-1:      reactive_power_enable[0..N]   (bool)
Indices 2N to 3N-1:     active_power_setpoint[0..N]   (f32)
Indices 3N to 4N-1:     reactive_power_setpoint[0..N] (f32)
```

## How to Integrate

### Option 1: Replace Existing Files (Recommended)

```bash
cd /home/chow/rust_prj/pcs_simulator

# Backup originals
cp src/main.rs src/main.rs.backup
cp src/threads/worker.rs src/threads/worker.rs.backup

# Copy enhanced versions
cp src/main_enhanced.rs src/main.rs
cp src/threads/worker_enhanced.rs src/threads/worker.rs
```

### Option 2: Add as New Module

**Edit `src/threads/mod.rs`:**
```rust
pub mod worker_enhanced;
pub use worker_enhanced::spawn_worker_threads_enhanced;
```

**Edit `src/main.rs`:**
Replace:
```rust
let worker_handles = spawn_worker_threads(
    packet_rx,
    appid_index_shared,
    mutable_data_shared,
    num_workers,
);
```

With:
```rust
// Load subscriber config
let subscriber_cfg_path = format!("{}PCS_subscriber_mapping.json", config_path);
let appid_to_pcs_count = load_subscriber_config(subscriber_cfg_path)?;

// Use enhanced workers
let worker_handles = spawn_worker_threads_enhanced(
    packet_rx,
    appid_index_shared,
    mutable_data_shared,
    Arc::new(appid_to_pcs_count),
    num_workers,
);
```

## Compilation

```bash
cargo build --release
```

## Testing

### 1. Check Configuration Load

Run the simulator and verify logs:
```
✅ Loaded command GOOSE APPID mappings
  APPID 0x0101 → 10 PCS units
  APPID 0x0008 → 10 PCS units
  APPID 0x0009 → 12 PCS units
```

### 2. Send Command GOOSE Frame

Use IEC61850 test tool to send GOOSE with APPID 0x0101

### 3. Verify Command Extraction

Check logs:
```
Received command GOOSE: APPID 0x0101, 10 PCS units, LAN1
✅ Extracted commands for PCS index 0 (APPID 0x0101, LAN1)
✅ Extracted commands for PCS index 1 (APPID 0x0101, LAN1)
...
```

## Configuration Files

### PCS_subscriber_mapping.json

Maps command GOOSE frames FROM controller:
```json
[
  {
    "APPID": "0x0101",
    "numberOfPcs": "10"
  }
]
```

### PCS_publisher_alldata_mapping.json

Maps feedback GOOSE frames TO controller (existing)

## Troubleshooting

### Issue: "No PCS indices found for APPID"

**Cause:** APPID mismatch between `pcs.csv` and `PCS_subscriber_mapping.json`

**Solution:** Verify APPID values match in both files

### Issue: "PCS index X not found"

**Cause:** ProcessData initialization issue

**Solution:** Check `pcs.csv` loads correctly

### Issue: "Failed to extract IECData"

**Cause:** GOOSE allData format doesn't match expected structure

**Solution:** Verify numberOfPcs matches actual GOOSE frame data count

## Summary

✅ Receives GOOSE command frames from controller  
✅ Identifies command frames using PCS_subscriber_mapping.json  
✅ Extracts power setpoints from allData  
✅ Updates PCS feedback values  
✅ Feedback published back to controller  

The implementation integrates seamlessly with existing architecture!
