default:
    @just help

fmt mode='':
    if [ "{{mode}}" = "--check" ]; then \
        cargo fmt --all -- --check; \
    else \
        cargo fmt --all; \
    fi

lint mode='':
    if [ "{{mode}}" = "--check" ]; then \
        cargo clippy --all-targets --all-features -- -D warnings; \
    else \
        cargo clippy --all-targets --all-features; \
    fi

check:
    cargo check

build:
    cargo build --all-targets --verbose

clean:
    cargo clean

help:
    @echo "PathWarp development commands"
    @echo "  just fmt           # run rustfmt"
    @echo "  just fmt --check   # check formatting"
    @echo "  just lint          # run clippy"
    @echo "  just lint --check  # run clippy with -D warnings"
    @echo "  just check         # run cargo check"
    @echo "  just build         # build all targets"
    @echo "  just clean         # clean build artifacts"
    @echo "  just help          # show this help"
