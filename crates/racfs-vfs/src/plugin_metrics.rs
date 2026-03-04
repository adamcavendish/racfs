//! Plugin-specific Prometheus metrics.
//!
//! Plugins that implement [`PluginMetrics`] can register their own counters, gauges,
//! and histograms with the server's metrics registry. Metrics are prefixed with
//! `racfs_plugin_<name>_` to avoid collisions.

use prometheus::Registry;

/// Trait for plugins that expose their own Prometheus metrics.
///
/// Implement this trait for a filesystem plugin to register plugin-specific
/// metrics (e.g. entry count, operation counts per type) with the server's
/// registry. The server will call `register` once during startup for each
/// mounted plugin that implements this trait.
pub trait PluginMetrics: Send + Sync {
    /// Register this plugin's metrics with the given registry.
    ///
    /// Use unique metric names prefixed with `racfs_plugin_<plugin_name>_` to
    /// avoid collisions with global VFS metrics and other plugins.
    fn register(&self, registry: &Registry) -> Result<(), prometheus::Error>;
}
