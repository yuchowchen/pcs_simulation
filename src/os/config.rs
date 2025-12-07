// read configuration from toml file
use config::Config;
use std::env;
use std::path::PathBuf;

pub struct AppConfig {

    pub sw_version: String,
    pub config_file_path: String,

    pub goose_interface_lan1: String,
    pub goose_interface_lan2: String,
    pub validity_interval_ms: u64,
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        // Try to find Config.toml in multiple locations
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let exe_path = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        
        let settings = match Config::builder()
            .add_source(config::File::with_name("Config"))
            .build() {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("ERROR: Could not find or load Config.toml: {e}");
                    eprintln!("       Current working directory: {}", current_dir.display());
                    if let Some(exe_dir) = exe_path {
                        eprintln!("       Executable directory: {}", exe_dir.display());
                        eprintln!("       Try running from the directory containing Config.toml, or");
                        eprintln!("       copy Config.toml to: {}", current_dir.display());
                    } else {
                        eprintln!("       Please ensure Config.toml exists in the current working directory.");
                    }
                    return Err(e);
                }
            };

        let sw_version = match settings.get_string("sw_version") {
            Ok(val) => {
                println!("✅ sw_version: {}", val);
                val
            }
            Err(e) => {
                eprintln!("ERROR: Missing or invalid 'sw_version' in Config.toml: {}", e);
                panic!("Failed to read 'sw_version' from Config.toml");
            }
        };

        let config_file_path = match settings.get_string("config_file_path") {
            Ok(val) => {
                println!("✅ config_file_path: {}", val);
                val
            }
            Err(e) => {
                eprintln!("ERROR: Missing or invalid 'config_file_path' in Config.toml: {}", e);
                panic!("Failed to read 'config_file_path' from Config.toml");
            }
        };

        let goose_interface_lan1 = match settings.get_string("goose_interface_lan1") {
            Ok(val) => {
                println!("✅ goose_interface_lan1: {}", val);
                val
            }
            Err(e) => {
                eprintln!("ERROR: Missing or invalid 'goose_interface_lan1' in Config.toml: {}", e);
                panic!("Failed to read 'goose_interface_lan1' from Config.toml");
            }
        };

        let goose_interface_lan2 = match settings.get_string("goose_interface_lan2") {
            Ok(val) => {
                println!("✅ goose_interface_lan2: {}", val);
                val
            }
            Err(e) => {
                eprintln!("ERROR: Missing or invalid 'goose_interface_lan2' in Config.toml: {}", e);
                panic!("Failed to read 'goose_interface_lan2' from Config.toml");
            }
        };

        let validity_interval_ms: u64 = match settings.get_string("validity_interval_ms") {
            Ok(val) => {
                println!("✅ validity_interval_ms: {}", val);
                val.parse().unwrap_or(5000)
            }
            Err(_) => {
                println!("⚠️  validity_interval_ms not found, using default: 5000ms");
                5000
            }
        };

        println!("Loaded configuration from Config.toml");

        Ok(AppConfig {
            sw_version,
            config_file_path,
            goose_interface_lan1,
            goose_interface_lan2,
            validity_interval_ms,
        })
    }
}
