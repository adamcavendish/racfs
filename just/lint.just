# Code quality commands

# Format code
fmt:
    cargo fmt --all

# Check formatting without applying
fmt-check:
    cargo fmt --all -- --check

# Run clippy with warnings as errors
clippy:
    cargo clippy --workspace -- -D warnings

# Run all linting checks
all: fmt-check clippy
