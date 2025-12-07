//! PCS (Power Conversion System) related types and helpers.
//!
//! This module exposes submodules for PCS nameplate information and
//! runtime process data used by the GOOSE publisher/subscriber code.

pub mod nameplate;
pub mod process_data;
pub mod types;

// Re-export commonly used types at the module root here if needed, e.g.
pub use nameplate::NameplateConfig;
pub use process_data::{ProcessData, AppIdIndex, MutablePcsData};
pub use types::{SubscriberPCSData};

/// Convenience prelude for PCS types.
pub mod prelude {
	pub use super::NameplateConfig;
	pub use super::{ProcessData, AppIdIndex, MutablePcsData};
	pub use super::{SubscriberPCSData};
}
