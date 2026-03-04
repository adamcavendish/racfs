# Chaos testing for RACFS

This document describes **chaos testing** for distributed RACFS scenarios: client (FUSE or HTTP) interacting with the server under failure conditions. The goal is to validate resilience (retries, circuit breaker, graceful degradation) and avoid data corruption or undefined behavior.

## Scope

- **Distributed scenarios:** At least two processes—racfs-server and a consumer (racfs-fuse mount or racfs-client). Failures are injected (kill server, partition network, slow responses).
- **Not covered here:** Single-process unit tests; those live in `crates/*/tests/` and integration tests in `crates/racfs-server/tests/` and `crates/racfs-fuse/tests/`.

## Scenarios (manual)

Run these by hand to observe current behavior. Automating them (e.g. with a chaos toolkit or toxiproxy) is a future improvement.

### 1. Server killed during read

1. Start server: `cargo run -p racfs-server`.
2. Mount FUSE: `cargo run -p racfs-fuse -- /tmp/racfs --server http://127.0.0.1:8080` (or use racfs-cli).
3. Trigger reads (e.g. `ls /tmp/racfs/memfs`, `cat /tmp/racfs/memfs/foo`).
4. Kill the server (Ctrl+C or `kill <pid>`).
5. **Observe:** Client should eventually see errors (e.g. I/O error or hang until timeout). With FUSE cache, cached metadata/readdir may still be served; new ops should fail or use stale cache per design.

### 2. Server killed during write

1. Same setup as above.
2. Start a write (e.g. `echo hello > /tmp/racfs/memfs/bar`).
3. Kill the server mid-request.
4. **Observe:** Write may fail with error; after restart, file may be missing or partial. No silent data loss is acceptable; partial writes should be detectable (e.g. short file).

### 3. Network partition (simulated)

- Use firewall rules or a proxy that drops packets (e.g. `iptables`, toxiproxy) to simulate partition between client and server.
- **Observe:** Client should hit timeouts and circuit breaker; after cooldown, retries should resume when connectivity returns.

### 4. Server restart

1. Start server, mount FUSE, create files.
2. Restart the server (same or new process).
3. Continue using the mount (read, write, list).
4. **Observe:** Stale cache may show old state until TTL; after TTL or invalidations, client should see current server state. No panics or permanent hang.

### 5. Slow server

- Use a proxy or artificial latency (e.g. `tc` on Linux) to delay server responses.
- **Observe:** Client should respect timeouts; long operations may fail with timeout errors. Load test (`load_test` example) can be run against a slowed server to stress retries and circuit breaker.

## Current resilience features

- **racfs-client:** Retry with exponential backoff on connection/timeout; circuit breaker (opens after repeated failures, cooldown 30 s). See `crates/racfs-client/src/client.rs`.
- **racfs-fuse:** Serves stale cache when backend is unreachable (read-only degradation). See `crates/racfs-fuse/src/cache.rs` and FUSE layer.
- **Server:** Stat cache and rate limiting; no chaos-specific behavior.

## Automation (future)

- Integrate a chaos tool (e.g. [Chaos Toolkit](https://chaostoolkit.org/), or custom scripts with `kill`, `iptables`, or toxiproxy) to run the scenarios above and assert expected outcomes.
- CI could run a minimal chaos job (e.g. start server, start client, kill server, assert client exits or returns errors without panic).

### Minimal verification script

A minimal automation is provided: **`./scripts/chaos_verify.sh`**

It starts the server in the background, waits for the health endpoint to respond, then kills the server and asserts that a subsequent client request fails (connection refused or similar). This verifies that when the server is gone, the client sees failure rather than hanging or panicking. Run from the repo root:

```bash
./scripts/chaos_verify.sh
```

Use `RACFS_PORT=18999` (default) or set another port if needed. See [ROADMAP.md](../ROADMAP.md) Testing Excellence for the "Chaos testing for distributed scenarios" item.
