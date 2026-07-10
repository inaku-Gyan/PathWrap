# PathWarp (still under development)

PathWarp is a Windows desktop application for quickly switching target folders when a system Open/Save file dialog appears.

The app listens to file dialog state and shows a lightweight overlay with paths from currently opened Explorer windows, reducing manual folder navigation.

## Features

- Detects system Open/Save file dialogs and docks a lightweight overlay flush beneath them
- Reads active Explorer window paths and displays them as selectable items
- Type-to-filter, up/down selection, Enter or double-click to jump the dialog to that folder
- Non-intrusive: the overlay never steals focus from the dialog
- Built with Rust + egui/eframe + windows-rs

## Architecture

PathWarp is layered as `os/` (Win32 integration) ↔ `ui/` (egui presentation) ↔ `app.rs`
(state + coordination), decoupled by channels. Key design decisions:

- **Non-activating overlay** ([src/os/window_ext.rs](src/os/window_ext.rs)): the eframe
  window carries `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`, so clicking it never
  moves foreground away from the dialog. Show/hide uses `ShowWindow`, and docking uses
  `SetWindowPos` in **physical pixels** matched to the dialog's DWM frame bounds — no
  logical-point/DPI conversion, hence no seam. This collapses the old focus state machine to
  a single "is the dialog foreground?" gate in [src/app.rs](src/app.rs).
- **Global keyboard hook** ([src/os/input_hook.rs](src/os/input_hook.rs)): because a
  non-activating window can't hold keyboard focus, a `WH_KEYBOARD_LL` hook — gated to
  "overlay visible AND dialog foreground" — intercepts typing/navigation keys and routes them
  to app state; all other keys pass through to the dialog. egui is a pure renderer over that
  state (no `TextEdit`).
- **UI Automation injection** ([src/os/dialog.rs](src/os/dialog.rs)): locates the filename
  edit and the default button via UIA, then `ValuePattern::SetValue` + `InvokePattern::Invoke`.
  Synchronous, no sleeps, no synthetic keystrokes, no focus theft; works on modern
  `IFileDialog`.
- **Dialog detection** ([src/os/monitor.rs](src/os/monitor.rs)): `SetWinEventHook` wakeups +
  adaptive polling, matching class `#32770` plus structural child-class evidence to avoid
  false positives on generic message boxes.

> Known limitation: the keyboard hook translates keys via `ToUnicodeEx`, so IME composition
> (e.g. Chinese input) is not captured for filtering; ASCII/partial filtering is unaffected.

## Development

### Environment

- Rust stable (recommended via `rustup`)
- Cargo (installed with Rust)
- Windows 10/11 (the project depends on Win32 APIs; full build and runtime verification should be done on Windows)
- Optional: [`just`](https://github.com/casey/just) (for running commands from the repository `Justfile`)

### Workflow

1. Install required tools (Rust, Cargo, optional just)
2. Clone the repository and enter the project directory
3. Run formatting, linting, and build-related commands as needed
4. Run and verify behavior in a Windows environment

### Script Commands (Justfile)

The project root provides a `Justfile` with the following base commands:

- `just check`: runs non-fixing checks
- `just fix`: runs auto-fix flow for formatting and lint/compiler suggestions
- `just run`: runs the application with logging enabled
- `just build`: builds the project
- `just test`: runs all tests
- `just clean`: cleans build artifacts
- `just help`: shows command help

> Note: In non-Windows environments, `build` may fail due to Win32 symbol linking constraints; perform final build verification on Windows.

### Tests

- Test framework: Rust built-in unit test framework (`cargo test`)
- Current tests cover basic UI filtering/selection logic units
- Run tests with:
  - `cargo test`
  - or `just test` (if `just` is installed)

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
