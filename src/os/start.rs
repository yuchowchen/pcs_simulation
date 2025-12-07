// start of the program: initialize configuration, logging and sockets
use crate::os::config::AppConfig;
use crate::os::interfaces::load_goose_interfaces;
use crate::os::logs::init_logging;
use anyhow::Result;
use log::info;
use pnet_datalink::NetworkInterface;

/// Initialize application subsystems and return an error on fatal failures.
pub fn start() -> Result<(String, Vec<NetworkInterface>, u64)> {
    // load configuration values
    let config = match AppConfig::load() {
        Ok(cfg) => {
            println!("✅ Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load application configuration: {}", e);
            panic!("Cannot continue without valid configuration");
        }
    };

    // initialize the logging system (non-fatal: if logging fails we continue but report)
    let log_config_path = format!("{}logging_config.yaml", config.config_file_path);
    if let Err(e) = init_logging(&log_config_path) {
        eprintln!("WARNING: Could not find or load {log_config_path}: {e}");
        eprintln!("         Logging will not be initialized. Please ensure the file exists and is readable.");
    }

    info!("Application version: {}", config.sw_version);
    info!("Goose interface LAN1: {}", config.goose_interface_lan1);
    info!("Goose interface LAN2: {}", config.goose_interface_lan2);



    // load goose interfaces
    let interfaces = match load_goose_interfaces(&config) {
        Ok(ifaces) => {
            println!("✅ Loaded {} GOOSE interface(s)", ifaces.len());
            for iface in &ifaces {
                println!("   - {}", iface.name);
            }
            ifaces
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load GOOSE interfaces: {}", e);
            panic!("Failed to load network interfaces");
        }
    };

    // return socket and interfaces for main to use
    Ok((config.config_file_path, interfaces, config.validity_interval_ms))
}
