//! Application state

use std::sync::Arc;

use prometheus::Registry;
use racfs_vfs::{MountableFS, PluginMetrics};

use crate::api::auth_api::AuthApiState;
use crate::observability::metrics::VfsMetrics;
use crate::stat_cache::StatCache;

/// Application state shared across requests.
#[derive(Clone)]
pub struct AppState {
    /// The virtual filesystem
    pub vfs: Arc<MountableFS>,
    /// Prometheus metrics registry
    pub metrics_registry: Arc<Registry>,
    /// VFS operation metrics
    pub vfs_metrics: Arc<VfsMetrics>,
    /// Stat cache (reduces VFS calls, exposes hit/miss metrics)
    pub stat_cache: Arc<StatCache>,
    /// Auth API state
    pub auth: AuthApiState,
}

impl AppState {
    pub fn new(
        vfs: Arc<MountableFS>,
        auth_config: crate::auth::JwtConfig,
        stat_cache_ttl_secs: u64,
        plugin_metrics: Vec<Arc<dyn PluginMetrics>>,
    ) -> Result<Self, prometheus::Error> {
        let metrics_registry = Arc::new(Registry::new());
        let vfs_metrics = Arc::new(VfsMetrics::new(&metrics_registry)?);
        vfs_metrics.register(&metrics_registry)?;

        for provider in &plugin_metrics {
            if let Err(e) = provider.register(&metrics_registry) {
                tracing::warn!(error = %e, "Failed to register plugin metrics");
            }
        }

        let stat_cache = Arc::new(StatCache::new_with_ttl(
            vfs_metrics.cache_hits_total.clone(),
            vfs_metrics.cache_misses_total.clone(),
            vfs_metrics.cache_evictions_total.clone(),
            std::time::Duration::from_secs(stat_cache_ttl_secs),
        ));

        let auth = AuthApiState::new(auth_config);

        Ok(Self {
            vfs,
            metrics_registry,
            vfs_metrics,
            stat_cache,
            auth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::JwtConfig;
    use racfs_vfs::MountableFS;

    #[test]
    fn test_app_state_new_succeeds() {
        let vfs = Arc::new(MountableFS::new());
        let auth_config = JwtConfig::default();
        let state = AppState::new(vfs, auth_config, 60, vec![]).expect("AppState::new");
        assert!(Arc::strong_count(&state.vfs) >= 1);
        assert!(Arc::strong_count(&state.metrics_registry) >= 1);
    }
}
