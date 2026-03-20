set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

# Show help message by default
default:
    @just help

# Run non-fixing fmt/clippy/check
[arg("mode", pattern="--ci|--local")]
check mode="--local":
    cargo fmt --all -- --check
    {{ if mode == "--ci" { "cargo clippy --all-targets --all-features -- -D warnings" } else { "cargo clippy --all-targets --all-features" } }}
    cargo check

# Run auto-fix for fmt, clippy, and rustc suggestions
fix:
    cargo fmt --all
    cargo clippy --fix --all-targets --all-features --allow-dirty
    cargo fix --all-targets --all-features --allow-dirty

# Run the application
[arg("logLevel", pattern="error|warn|info|debug|trace")]
run logLevel="info":
    $env:RUST_LOG="{{ logLevel }}"; cargo run

# Build all targets
build:
    cargo build --all-targets --verbose

# Clean build artifacts
clean:
    cargo clean

# Show this help message
help:
    @just --list
