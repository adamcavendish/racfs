//! RACFS Server - REST API for virtual filesystem

mod api;
mod auth;
mod config;
mod error;
mod middleware;
mod observability;
mod ratelimit;
mod stat_cache;
mod state;
mod validation;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use racfs_core::FoyerCache;
use racfs_plugin_devfs::DevFS;
use racfs_plugin_kvfs::KvFS;
use racfs_plugin_localfs::LocalFS;
use racfs_plugin_memfs::MemFS;
use racfs_plugin_proxyfs::ProxyFS;
use racfs_plugin_queuefs::QueueFS;
use racfs_plugin_streamfs::{StreamConfig, StreamFS};
use racfs_plugin_vectorfs::{VectorConfig, VectorFS};
use racfs_vfs::{CachedFs, MountableFS, PluginMetrics};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::auth_api;
use crate::config::ServerConfig;
use crate::observability::tracing::{TraceExporter, init_tracing};
use crate::ratelimit::{RateLimitConfig, RateLimiter};
use crate::state::AppState;
use racfs_server::openapi::ApiDoc;

/// Reload rate limit and stat cache config from file on SIGHUP (Unix only).
#[cfg(unix)]
async fn reload_on_sighup(
    config_path: Option<String>,
    rate_limiter: Arc<RateLimiter>,
    state: AppState,
) {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sig = match signal(SignalKind::hangup()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "SIGHUP not supported, config hot reload disabled");
            return;
        }
    };

    loop {
        if sig.recv().await.is_none() {
            break;
        }

        let path = match &config_path {
            Some(p) => p.clone(),
            None => continue,
        };

        let mut config = match ServerConfig::from_file(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "config reload: failed to load file");
                continue;
            }
        };
        config.apply_env_overrides();

        if let Err(e) = config.validate() {
            tracing::warn!(error = %e, "config reload: validation failed, keeping previous config");
            continue;
        }

        let rl_config = RateLimitConfig {
            max_requests: config.ratelimit.max_requests,
            window: std::time::Duration::from_secs(config.ratelimit.window_secs),
            headers: crate::ratelimit::RateLimitHeaders::default(),
        };
        rate_limiter.update_config(rl_config);
        state
            .stat_cache
            .set_ttl(std::time::Duration::from_secs(config.stat_cache.ttl_secs));

        tracing::info!(
            path = %path,
            ratelimit_max_requests = config.ratelimit.max_requests,
            stat_cache_ttl_secs = config.stat_cache.ttl_secs,
            "config reloaded (ratelimit, stat_cache)"
        );
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Mount configuration (toml file or inline)
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

type ServerError = Box<dyn std::error::Error + Send + Sync>;

async fn run() -> Result<(), ServerError> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize OpenTelemetry tracing (OTLP disabled by default)
    let _tracer = init_tracing(TraceExporter::None);

    // Parse arguments
    let args = Args::parse();

    // Config file path: CLI --config or RACFS_CONFIG env
    let config_path: Option<String> = args.config.or_else(|| std::env::var("RACFS_CONFIG").ok());

    // Load configuration
    let mut config =
        ServerConfig::from_file_or_default(config_path.as_deref()).unwrap_or_else(|e| {
            info!("Failed to load config: {}, using defaults", e);
            ServerConfig::default()
        });

    // Environment overrides (RACFS_HOST, RACFS_PORT)
    config.apply_env_overrides();

    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    }

    // Config values (file + env) determine host/port
    let host = config.host;
    let port = config.port;

    // Create the virtual filesystem
    let vfs = Arc::new(MountableFS::new());
    let mut plugin_metrics: Vec<Arc<dyn PluginMetrics>> = Vec::new();

    // Mount filesystems from config
    if config.mounts.is_empty() {
        // Mount default filesystems if none configured
        let memfs = Arc::new(MemFS::new());
        let devfs = Arc::new(DevFS::new()) as Arc<dyn racfs_core::FileSystem>;

        vfs.mount(
            PathBuf::from("/memfs"),
            memfs.clone() as Arc<dyn racfs_core::FileSystem>,
        )?;
        vfs.mount(PathBuf::from("/dev"), devfs)?;
        plugin_metrics.push(memfs as Arc<dyn PluginMetrics>);

        info!("Mounted filesystems: /memfs, /dev (defaults)");
    } else {
        // Mount configured filesystems
        for (name, mount_cfg) in &config.mounts {
            let (fs, as_metrics): (
                Arc<dyn racfs_core::FileSystem>,
                Option<Arc<dyn PluginMetrics>>,
            ) = match mount_cfg.fs_type.as_str() {
                "memfs" => {
                    let memfs = Arc::new(MemFS::new());
                    let m = memfs.clone() as Arc<dyn PluginMetrics>;
                    (memfs as Arc<dyn racfs_core::FileSystem>, Some(m))
                }
                "devfs" => (Arc::new(DevFS::new()), None),
                "localfs" => {
                    let root = mount_cfg.root.as_ref().ok_or_else(|| {
                        crate::config::ConfigValidationError {
                            message: "localfs requires 'root' path".to_string(),
                        }
                    })?;
                    (Arc::new(LocalFS::new(PathBuf::from(root))), None)
                }
                "proxyfs" => {
                    let url = mount_cfg.url.as_ref().ok_or_else(|| {
                        crate::config::ConfigValidationError {
                            message: "proxyfs requires 'url'".to_string(),
                        }
                    })?;
                    (Arc::new(ProxyFS::new(url.clone())), None)
                }
                "streamfs" => {
                    let stream_config = StreamConfig {
                        buffer_size: mount_cfg.buffer_size.unwrap_or(1000),
                        history_size: mount_cfg.history_size.unwrap_or(100),
                        max_streams: mount_cfg.max_streams.unwrap_or(100),
                        compression: None,
                    };
                    (Arc::new(StreamFS::new(stream_config)), None)
                }
                "vectorfs" => {
                    let storage_uri = mount_cfg
                        .storage_uri
                        .clone()
                        .or(mount_cfg.db_path.clone())
                        .unwrap_or_else(|| "./vector_db".to_string());
                    let vector_config = VectorConfig {
                        storage_uri,
                        embedding_api: mount_cfg.embedding_url.clone(),
                        table_name: mount_cfg
                            .table_name
                            .clone()
                            .unwrap_or_else(|| "vectors".to_string()),
                        dimension: mount_cfg.dimension.unwrap_or(384),
                    };
                    let fs = VectorFS::with_config_async(vector_config)
                        .await
                        .map_err(|e| -> ServerError { Box::new(e) })?;
                    (Arc::new(fs), None)
                }
                "queuefs" => (Arc::new(QueueFS::new()), None),
                "kvfs" => {
                    let kvfs = match &mount_cfg.database_path {
                        Some(p) => {
                            KvFS::with_database(p).map_err(|e| -> ServerError { Box::new(e) })?
                        }
                        None => KvFS::new(),
                    };
                    (Arc::new(kvfs), None)
                }
                _ => {
                    info!("Unknown fs type: {}, skipping", mount_cfg.fs_type);
                    continue;
                }
            };

            // Optional cache tier: wrap with CachedFs + FoyerCache when mount.cache is enabled
            let fs: Arc<dyn racfs_core::FileSystem> = if let Some(ref cache_cfg) = mount_cfg.cache
                && cache_cfg.enabled
                && cache_cfg.max_entries.is_some()
            {
                let max_entries = cache_cfg.max_entries.unwrap();
                let cache = Arc::new(FoyerCache::new(max_entries));
                let cached = CachedFs::new(fs.clone(), cache)
                    .with_key_prefix(&format!("{}:", &mount_cfg.path));
                Arc::new(cached) as Arc<dyn racfs_core::FileSystem>
            } else {
                fs
            };

            let mount_path = PathBuf::from(&mount_cfg.path);
            vfs.mount(mount_path, fs.clone()).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Failed to mount {}: {}", name, e),
                )
            })?;
            if let Some(m) = as_metrics {
                plugin_metrics.push(m);
            }
            info!(
                "Mounted {} at {} ({})",
                name, mount_cfg.path, mount_cfg.fs_type
            );
        }
    }

    // Create application state
    let state = AppState::new(
        vfs,
        config.jwt.clone(),
        config.stat_cache.ttl_secs,
        plugin_metrics,
    )
    .map_err(|e| -> ServerError { Box::new(e) })?;

    // Rate limit config from file/env (reloadable via SIGHUP)
    let rate_limit_config = crate::ratelimit::RateLimitConfig {
        max_requests: config.ratelimit.max_requests,
        window: std::time::Duration::from_secs(config.ratelimit.window_secs),
        headers: crate::ratelimit::RateLimitHeaders::default(),
    };
    let rate_limiter = Arc::new(RateLimiter::new(rate_limit_config));

    // Optional: spawn SIGHUP handler to reload non-critical config (Unix only)
    #[cfg(unix)]
    if config_path.is_some() {
        let reload_path = config_path.clone();
        let reload_limiter = rate_limiter.clone();
        let reload_state = state.clone();
        tokio::spawn(async move {
            reload_on_sighup(reload_path, reload_limiter, reload_state).await;
        });
    }

    // Build the router with OpenAPI
    let (router, _) = OpenApiRouter::new()
        .routes(routes!(api::health::health_check))
        .routes(routes!(api::metrics::metrics))
        .routes(routes!(api::files::read_file))
        .routes(routes!(api::files::create_file))
        .routes(routes!(api::files::write_file))
        .routes(routes!(api::files::delete_file))
        .routes(routes!(api::directories::list_directory))
        .routes(routes!(api::directories::create_directory))
        .routes(routes!(api::metadata::stat))
        .routes(routes!(api::metadata::rename))
        .routes(routes!(api::metadata::chmod))
        .routes(routes!(api::metadata::truncate))
        .routes(routes!(api::metadata::symlink))
        .routes(routes!(api::xattr::get_xattr))
        .routes(routes!(api::xattr::set_xattr))
        .routes(routes!(api::xattr::remove_xattr))
        .routes(routes!(api::xattr::list_xattr))
        .split_for_parts();

    // Build auth routes with AuthApiState
    let auth_router = auth_api::routes().with_state(state.auth.clone());

    let app = router
        .merge(auth_router)
        .merge(SwaggerUi::new("/swagger").url("/openapi.json", ApiDoc::openapi()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::latency_middleware,
        ))
        .layer(axum::middleware::from_fn(ratelimit::rate_limit_middleware))
        .layer(axum::Extension(rate_limiter))
        .with_state(state);

    // Bind to address
    let addr = format!("{}:{}", host, port);
    let addr: SocketAddr = addr.parse()?;
    info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
