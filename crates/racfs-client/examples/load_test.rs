//! Load test example: run concurrent requests against a RACFS server.
//!
//! Start a server first: `cargo run -p racfs-server`
//! Then: `cargo run -p racfs-client --example load_test`
//!
//! Environment variables:
//!   RACFS_SERVER              - Server URL (default: http://127.0.0.1:8080)
//!   RACFS_LOAD_CONCURRENCY    - Number of concurrent workers (default: 8)
//!   RACFS_LOAD_DURATION_SECS  - Run duration in seconds (default: 10)
//!   RACFS_LOAD_WORKLOAD       - Workload type: stat, readdir, read, mixed (default: mixed)
//!
//! For high concurrency you may need to raise server rate limits (e.g. [ratelimit] in config).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use racfs_client::Client;

fn parse_env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[derive(Clone, Copy, Debug)]
enum Workload {
    Stat,
    Readdir,
    Read,
    Mixed,
}

fn parse_workload() -> Workload {
    match std::env::var("RACFS_LOAD_WORKLOAD")
        .as_deref()
        .unwrap_or("mixed")
    {
        "stat" => Workload::Stat,
        "readdir" => Workload::Readdir,
        "read" => Workload::Read,
        _ => Workload::Mixed,
    }
}

async fn run_worker(
    client: Client,
    workload: Workload,
    path_stat: String,
    path_read: String,
    until: Instant,
    ok_count: Arc<AtomicU64>,
    err_count: Arc<AtomicU64>,
) {
    while Instant::now() < until {
        let mut op_ok = true;
        match workload {
            Workload::Stat => {
                if client.stat(&path_stat).await.is_err() {
                    op_ok = false;
                }
            }
            Workload::Readdir => {
                if client.read_dir(&path_stat).await.is_err() {
                    op_ok = false;
                }
            }
            Workload::Read => {
                if client.read_file(&path_read).await.is_err() {
                    op_ok = false;
                }
            }
            Workload::Mixed => {
                if client.stat(&path_stat).await.is_err()
                    || client.read_dir(&path_stat).await.is_err()
                    || client.read_file(&path_read).await.is_err()
                {
                    op_ok = false;
                }
            }
        }
        if op_ok {
            ok_count.fetch_add(1, Ordering::Relaxed);
        } else {
            err_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        std::env::var("RACFS_SERVER").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let concurrency = parse_env_u32("RACFS_LOAD_CONCURRENCY", 8);
    let duration_secs = parse_env_u32("RACFS_LOAD_DURATION_SECS", 10);
    let workload = parse_workload();

    let client = Client::new(&base);
    println!(
        "Load test: server={} concurrency={} duration={}s workload={:?}",
        base, concurrency, duration_secs, workload
    );

    let health = client.health().await?;
    println!("Server health: {} {}", health.status, health.version);

    let path_stat = "/memfs";
    let path_read = "/memfs/loadtest/fixture.txt";

    if matches!(workload, Workload::Read | Workload::Mixed) {
        client.mkdir("/memfs/loadtest", Some(0o755)).await.ok();
        client.create_file(path_read).await.ok();
        client
            .write_file(path_read, "load test fixture", None)
            .await
            .ok();
    }

    let until = Instant::now() + Duration::from_secs(duration_secs as u64);
    let ok_count = Arc::new(AtomicU64::new(0));
    let err_count = Arc::new(AtomicU64::new(0));

    let path_stat = path_stat.to_string();
    let path_read = path_read.to_string();

    let mut handles = Vec::with_capacity(concurrency as usize);
    for _ in 0..concurrency {
        let client = Client::new(&base);
        let handle = tokio::spawn(run_worker(
            client,
            workload,
            path_stat.clone(),
            path_read.clone(),
            until,
            Arc::clone(&ok_count),
            Arc::clone(&err_count),
        ));
        handles.push(handle);
    }

    for h in handles {
        h.await?;
    }

    let ok = ok_count.load(Ordering::Relaxed);
    let err = err_count.load(Ordering::Relaxed);
    let total = ok + err;
    let ops_per_sec = total as f64 / duration_secs as f64;

    println!("Result: {} ok, {} err, {:.1} ops/sec", ok, err, ops_per_sec);

    Ok(())
}
