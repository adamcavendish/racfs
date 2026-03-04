//! RACFS Virtual Filesystem (VFS)
//!
//! This crate provides the virtual filesystem layer with mount point routing
//! and handles management for the RACFS project.

pub mod cached_fs;
pub mod handle_manager;
pub mod mountable_fs;
pub use cached_fs::CachedFs;
pub mod plugin_metrics;

pub use handle_manager::HandleManager;
pub use mountable_fs::MountableFS;
pub use plugin_metrics::PluginMetrics;
