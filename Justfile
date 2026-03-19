default:
    @just help

check mode='':
    if [ "{{mode}}" = "--ci" ]; then \
        cargo fmt --all -- --check; \
        cargo clippy --all-targets --all-features -- -D warnings; \
        cargo check; \
    else \
        cargo fmt --all; \
        cargo clippy --all-targets --all-features; \
        cargo check; \
    fi

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
    @echo "  just check         # run fmt + clippy + cargo check"
    @echo "  just check --ci    # run CI-style checks"
    @echo "  just fix           # run auto-fix for fmt, clippy, and rustc suggestions"
    @echo "  just run           # run the application"
    @echo "  just build         # build all targets"
    @echo "  just clean         # clean build artifacts"
    @echo "  just help          # show this help"
