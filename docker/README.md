# RACFS Docker images

## racfs-server

Build and run the RACFS REST API server in a container. The image uses default mounts (/memfs, /dev) if no config is provided.

### Build

From the **repository root**:

```bash
docker build -f docker/Dockerfile -t racfs-server:latest .
```

### Run

```bash
# Default: server on 0.0.0.0:8080 with built-in memfs + devfs
docker run --rm -p 8080:8080 racfs-server:latest
```

Optional environment variables (see [configuration](https://github.com/adamcavendish/racfs/blob/master/examples/configs/README.md)):

- `RACFS_HOST` — bind address (default in image: `0.0.0.0`)
- `RACFS_PORT` — port (default: `8080`)
- `RACFS_CONFIG` — path to a TOML config file inside the container (mount a file and set this)

### Custom config

Mount a config file and point the server at it:

```bash
docker run --rm -p 8080:8080 \
  -v /path/on/host/config.toml:/etc/racfs/config.toml:ro \
  -e RACFS_CONFIG=/etc/racfs/config.toml \
  racfs-server:latest
```

### Health and metrics

- Health: `GET http://localhost:8080/health`
- Metrics: `GET http://localhost:8080/metrics`
- Swagger UI: `http://localhost:8080/swagger`

### Image details

- **Base:** `debian:bookworm-slim`
- **Binary:** `/usr/local/bin/racfs-server`
- **Exposed port:** 8080
- **Entrypoint:** `racfs-server` with default `--host 0.0.0.0 --port 8080`
