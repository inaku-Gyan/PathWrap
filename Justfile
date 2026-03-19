set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

default:
    @just help

check mode='':
    cargo fmt --all -- --check
    {{ if mode == "--ci" { "cargo clippy --all-targets --all-features -- -D warnings" } else { "cargo clippy --all-targets --all-features" } }}
    cargo check

fix:
    cargo fmt --all
    cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged
    cargo fix --all-targets --all-features --allow-dirty --allow-staged

run:
    cargo run

build:
    cargo build --all-targets --verbose

clean:
    cargo clean

help:
    @echo "PathWarp development commands"
    @echo "  just check         # run non-fixing fmt/clippy/check"
    @echo "  just check --ci    # run CI-style checks"
    @echo "  just fix           # run auto-fix for fmt, clippy, and rustc suggestions"
    @echo "  just run           # run the application"
    @echo "  just build         # build all targets"
    @echo "  just clean         # clean build artifacts"
    @echo "  just help          # show this help"
