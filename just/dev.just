# Development utilities

# Watch and rebuild on changes
watch:
    cargo watch -x "build --workspace"

# Build and open documentation
docs:
    cargo doc --workspace --no-deps --open

# Check for outdated dependencies
outdated:
    cargo outdated

# Run all CI checks (for local CI verification)
ci:
    @just lint/all
    @just test/all
