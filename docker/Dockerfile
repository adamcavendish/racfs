# Multi-stage build: compile racfs-server, then minimal runtime image.
# Build from repository root: docker build -f docker/Dockerfile -t racfs-server:latest .

FROM rust:1-bookworm AS builder

WORKDIR /build

# Copy workspace and lock file first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary (only racfs-server and its deps)
RUN cargo build --release -p racfs-server

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Listen on all interfaces inside the container
ENV RACFS_HOST=0.0.0.0
ENV RACFS_PORT=8080

EXPOSE 8080

COPY --from=builder /build/target/release/racfs-server /usr/local/bin/racfs-server

ENTRYPOINT ["/usr/local/bin/racfs-server"]
