# Run commands

# Run the REST API server
server:
    cargo run --package racfs-server

# Run CLI tool with arguments
cli *ARGS:
    cargo run --package racfs-cli -- {{ARGS}}

# Mount FUSE filesystem
fuse MOUNTPOINT:
    cargo run --package racfs-fuse -- {{MOUNTPOINT}}
