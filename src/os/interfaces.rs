use anyhow::Result;
use crate::os::config::AppConfig;
use pnet_datalink::NetworkInterface;

/// Look up interfaces listed in AppConfig (goose_interface_lan1, goose_interface_lan2)
/// and return a vector of matched NetworkInterface objects.
pub fn load_goose_interfaces(config: &AppConfig) -> Result<Vec<NetworkInterface>> {
	let names = vec![config.goose_interface_lan1.clone(), config.goose_interface_lan2.clone()];
	let all = pnet_datalink::interfaces();

	let mut found = Vec::new();
	for name in names.into_iter() {
		if name.is_empty() {
			continue;
		}

        // check available interfaces for a match
		if let Some(iface) = all.iter().find(|i| i.name == name) {
			found.push(iface.clone());
		}
	}

	Ok(found)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load_goose_interfaces_empty() {
		let cfg = AppConfig {
			sw_version: String::new(),
			config_file_path: String::new(),
			goose_interface_lan1: String::new(),
			goose_interface_lan2: String::new(),
		};

		let res = load_goose_interfaces(&cfg).unwrap();
		assert!(res.is_empty());
	}

	#[test]
	fn test_load_goose_interfaces_match_first() {
		let all = pnet_datalink::interfaces();
        println!("Detected interfaces:");
        for iface in &all {
            println!(" - {}", iface.name);
        }
		if all.is_empty() {
			// Nothing to test on systems without interfaces
			return;
		}
		let first_name = all[0].name.clone();

		let cfg = AppConfig {
            comm_interval_pcs_to_plc:String::new(),
			sw_version: String::new(),
			config_file_path: String::new(),
			goose_interface_lan1: first_name.clone(),
			goose_interface_lan2: String::new(),
		};

		let res = load_goose_interfaces(&cfg).unwrap();
		assert!(!res.is_empty());
		assert_eq!(res[0].name, first_name);
	}
}
