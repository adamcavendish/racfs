//! Prometheus metrics for RACFS

use prometheus::{HistogramOpts, HistogramVec, IntCounter, IntGauge, Registry};

/// Default histogram buckets for request duration (seconds). Covers 1ms to 5s.
const REQUEST_DURATION_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
];

/// Custom VFS operation metrics
pub struct VfsMetrics {
    /// Total number of file read operations
    pub file_reads_total: IntCounter,
    /// Total number of file write operations
    pub file_writes_total: IntCounter,
    /// Total number of file delete operations
    pub file_deletes_total: IntCounter,
    /// Total number of directory list operations
    pub directory_lists_total: IntCounter,
    /// Total number of directory create operations
    pub directory_creates_total: IntCounter,
    /// Total number of stat operations
    pub stat_operations_total: IntCounter,
    /// Total number of rename operations
    pub rename_operations_total: IntCounter,
    /// Total number of chmod operations
    pub chmod_operations_total: IntCounter,
    /// Current number of active connections
    pub active_connections: IntGauge,
    /// Total number of HTTP requests
    pub http_requests_total: IntCounter,
    /// Total number of HTTP errors
    pub http_errors_total: IntCounter,
    /// Cache hits (e.g. stat cache)
    pub cache_hits_total: IntCounter,
    /// Cache misses (e.g. stat cache)
    pub cache_misses_total: IntCounter,
    /// Cache evictions (e.g. capacity or TTL expiry; 0 until a cache reports them)
    pub cache_evictions_total: IntCounter,
    /// Request duration in seconds by operation (for latency percentiles)
    pub request_duration_seconds: HistogramVec,
}

impl VfsMetrics {
    /// Create new VFS metrics and register them
    pub fn new(_registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(VfsMetrics {
            file_reads_total: IntCounter::new(
                "racfs_file_reads_total",
                "Total number of file read operations",
            )?,
            file_writes_total: IntCounter::new(
                "racfs_file_writes_total",
                "Total number of file write operations",
            )?,
            file_deletes_total: IntCounter::new(
                "racfs_file_deletes_total",
                "Total number of file delete operations",
            )?,
            directory_lists_total: IntCounter::new(
                "racfs_directory_lists_total",
                "Total number of directory list operations",
            )?,
            directory_creates_total: IntCounter::new(
                "racfs_directory_creates_total",
                "Total number of directory create operations",
            )?,
            stat_operations_total: IntCounter::new(
                "racfs_stat_operations_total",
                "Total number of stat operations",
            )?,
            rename_operations_total: IntCounter::new(
                "racfs_rename_operations_total",
                "Total number of rename operations",
            )?,
            chmod_operations_total: IntCounter::new(
                "racfs_chmod_operations_total",
                "Total number of chmod operations",
            )?,
            active_connections: IntGauge::new(
                "racfs_active_connections",
                "Current number of active connections",
            )?,
            http_requests_total: IntCounter::new(
                "racfs_http_requests_total",
                "Total number of HTTP requests",
            )?,
            http_errors_total: IntCounter::new(
                "racfs_http_errors_total",
                "Total number of HTTP errors",
            )?,
            cache_hits_total: IntCounter::new(
                "racfs_cache_hits_total",
                "Total number of cache hits",
            )?,
            cache_misses_total: IntCounter::new(
                "racfs_cache_misses_total",
                "Total number of cache misses",
            )?,
            cache_evictions_total: IntCounter::new(
                "racfs_cache_evictions_total",
                "Total number of cache evictions (capacity or TTL)",
            )?,
            request_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "racfs_request_duration_seconds",
                    "Request duration in seconds by operation (for p50, p95, p99)",
                )
                .buckets(REQUEST_DURATION_BUCKETS.to_vec()),
                &["operation"],
            )?,
        })
    }

    /// Register metrics with a registry
    pub fn register(&self, registry: &Registry) -> Result<(), prometheus::Error> {
        registry.register(Box::new(self.file_reads_total.clone()))?;
        registry.register(Box::new(self.file_writes_total.clone()))?;
        registry.register(Box::new(self.file_deletes_total.clone()))?;
        registry.register(Box::new(self.directory_lists_total.clone()))?;
        registry.register(Box::new(self.directory_creates_total.clone()))?;
        registry.register(Box::new(self.stat_operations_total.clone()))?;
        registry.register(Box::new(self.rename_operations_total.clone()))?;
        registry.register(Box::new(self.chmod_operations_total.clone()))?;
        registry.register(Box::new(self.active_connections.clone()))?;
        registry.register(Box::new(self.http_requests_total.clone()))?;
        registry.register(Box::new(self.http_errors_total.clone()))?;
        registry.register(Box::new(self.cache_hits_total.clone()))?;
        registry.register(Box::new(self.cache_misses_total.clone()))?;
        registry.register(Box::new(self.cache_evictions_total.clone()))?;
        registry.register(Box::new(self.request_duration_seconds.clone()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Registry;

    #[test]
    fn test_vfs_metrics_new_and_register() {
        let registry = Registry::new();
        let metrics = VfsMetrics::new(&registry).expect("VfsMetrics::new");
        metrics.register(&registry).expect("register");
        // Ensure we can use the metrics
        metrics.file_reads_total.inc();
        metrics.stat_operations_total.inc();
        assert_eq!(metrics.file_reads_total.get(), 1);
        assert_eq!(metrics.stat_operations_total.get(), 1);
    }
}
