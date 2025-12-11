//! PCS data structures and nameplate configuration.

// Submodules
pub mod nameplate;
pub mod types;
pub mod publisher;

// Re-export main types
pub use nameplate::NameplateConfig;
pub use types::{PublisherPcsData};
pub use publisher::{load_pcs_type_mappings, init_goose_frame_for_pcs, GooseFrame, PcsTypeMapping};

// Prelude for convenient imports
pub mod prelude {
pub use super::NameplateConfig;
pub use super::{PublisherPcsData};
pub use super::{GooseFrame, PcsTypeMapping};
}
