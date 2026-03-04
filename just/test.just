# Testing commands

# Run all tests
all:
    cargo test --workspace

# Run integration tests
integration:
    cargo test --workspace --test integration_tests

# Run unit tests only
unit:
    cargo test --workspace --lib

# Run test coverage (stdout only). Requires: cargo install cargo-tarpaulin
coverage:
    cargo tarpaulin --workspace --out Stdout

# Run test coverage and write HTML report to coverage/. Requires: cargo install cargo-tarpaulin
coverage-html:
    cargo tarpaulin --workspace --out Stdout --out Html --output-dir coverage
    @echo "HTML report written to coverage/"
