# PathWarp

PathWarp is a Windows desktop application for quickly switching target folders when a system Open/Save file dialog appears.

The app listens to file dialog state and shows a lightweight overlay with paths from currently opened Explorer windows, reducing manual folder navigation.

## Features

- Detects system Open/Save file dialogs and shows an overlay panel when appropriate
- Reads active Explorer window paths and displays them as selectable items
- Supports search filtering, keyboard up/down selection, and Enter confirmation
- Built with Rust + egui/eframe + windows-rs

## Development Environment

- Rust stable (recommended via `rustup`)
- Cargo (installed with Rust)
- Windows 10/11 (the project depends on Win32 APIs; full build and runtime verification should be done on Windows)
- Optional: [`just`](https://github.com/casey/just) (for running commands from the repository `Justfile`)

## Development Workflow

1. Install required tools (Rust, Cargo, optional just)
2. Clone the repository and enter the project directory
3. Run formatting, linting, and build-related commands as needed
4. Run and verify behavior in a Windows environment

## Script Commands (Justfile)

The project root provides a `Justfile` with the following base commands:

- `just check`: runs `cargo fmt --all`, `cargo clippy --all-targets --all-features`, and `cargo check`
- `just check --ci`: runs CI-style checks (`cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo check`)
- `just fix`: runs auto-fix flow for formatting and lint/compiler suggestions (`cargo fmt --all`, `cargo clippy --fix ...`, and `cargo fix ...`)
- `just run`: runs `cargo run`
- `just build`: runs `cargo build --all-targets --verbose`
- `just clean`: runs `cargo clean`
- `just help`: shows command help

> Note: In non-Windows environments, `build` may fail due to Win32 symbol linking constraints; perform final build verification on Windows.

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
