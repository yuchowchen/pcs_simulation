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


