use log::{error, info};
use std::path::Path;

/// Initialize logging from `logging_config.yaml` when available.
/// Returns the log4rs Handle on success.
pub fn init_logging<P: AsRef<Path>>(config_path: P) -> Result<(), Box<dyn std::error::Error>> {
	let path = config_path.as_ref();
	if path.exists() {
		match log4rs::init_file(path, Default::default()) {
			Ok(_) => {
				info!("Loaded logging configuration from {}", path.display());
				return Ok(());
			}
			Err(e) => {
				let msg = format!("Failed to load logging config {}: {}", path.display(), e);
				error!("{}", msg);
				return Err(msg.into());
			}
		}
	}

	let msg = format!("Logging configuration not found: {}", path.display());
	error!("{}", msg);
	Err(msg.into())
}