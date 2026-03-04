# Build commands

# Build all workspace crates
build:
    cargo build --workspace

# Build in release mode
release:
    cargo build --workspace --release

# Fast check without codegen
check:
    cargo check --workspace

# Clean build artifacts
clean:
    cargo clean
