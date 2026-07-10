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

PathWarp is layered as `core/` (platform-independent decision logic) ↔ `os/` (Win32
integration) ↔ `ui/` (egui presentation), wired together by a thin `app.rs` shell and
decoupled by channels. Key design decisions:

- **Pure controller state machine** ([src/core/controller.rs](src/core/controller.rs)): a
  `Controller::step(env, event) -> Vec<Effect>` state machine owns **all** show/hide, docking,
  injection, hook-gating, debounce and suppression decisions. Time and the foreground window
  are injected via `Env`, so every timing rule is deterministically unit-testable. `app.rs`
  only collects events (dialog channel, keyboard hook, egui mouse responses), calls `step`,
  and executes the returned `Effect`s — it contains no decision logic.
- **Non-activating overlay** ([src/os/window_ext.rs](src/os/window_ext.rs)): the eframe window
  carries `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`, **re-asserted every frame** in
  [src/app.rs](src/app.rs) — winit recomputes and rewrites `GWL_EXSTYLE` after our initial
  apply (wiping `WS_EX_NOACTIVATE`), so a one-time set does not hold; the per-frame idempotent
  re-assert is the load-bearing guarantee. The window procedure is additionally subclassed to
  answer `WM_MOUSEACTIVATE` with `MA_NOACTIVATE` as a second line of defence. Together these
  ensure clicking the overlay never activates it or moves foreground off the dialog. "Hiding"
  moves the window off-screen while keeping it `WS_VISIBLE` (never `SW_HIDE`) — a hidden window
  stops receiving paints and would starve eframe's event loop. Docking uses `SetWindowPos` in
  **physical pixels** matched to the dialog's DWM frame bounds — no DPI conversion, no seam.
- **glow renderer** ([src/main.rs](src/main.rs), `Cargo.toml`): eframe is pinned to the glow
  (OpenGL) backend instead of the default wgpu. wgpu's Windows HWND surface only advertises an
  opaque `CompositeAlphaMode`, so a transparent window renders its transparent pixels as black;
  glow composites transparency via the window's own alpha + DWM, giving the overlay real
  rounded corners and drop shadow. It also avoids noisy Vulkan-loader errors from unrelated
  third-party layers.
- **Global keyboard hook** ([src/os/input_hook.rs](src/os/input_hook.rs)): because a
  non-activating window can't hold keyboard focus, a `WH_KEYBOARD_LL` hook — gated to
  "overlay visible AND dialog foreground" — intercepts typing/navigation keys and feeds them to
  the controller; all other keys pass through to the dialog. egui is a pure renderer over
  controller state (no `TextEdit`).
- **UI Automation injection** ([src/os/dialog.rs](src/os/dialog.rs)): locates the filename edit
  and the default button via UIA, then `ValuePattern::SetValue` + `InvokePattern::Invoke`.
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
- `just test`: runs unit tests (`cargo test`)
- `just e2e`: runs the Windows end-to-end tests (real dialogs + real mouse input; needs an interactive desktop)
- `just clean`: cleans build artifacts
- `just help`: shows command help

> Note: In non-Windows environments, `build` may fail due to Win32 symbol linking constraints; perform final build verification on Windows.

### Tests

The suite is a three-layer pyramid:

1. **Controller unit tests** ([src/core/controller.rs](src/core/controller.rs)): the state
   machine's full behavior — docking, foreground debounce, `None`-grace hide, ESC suppression,
   injection ordering (hook off before inject) with the overlay staying docked afterwards,
   following the active dialog when several are open, dock de-duplication — driven with injected
   time. Pure and millisecond-fast.
2. **Glue-layer UI tests** ([src/ui/window.rs](src/ui/window.rs)): [`egui_kittest`] drives the
   real renderer via AccessKit — filtered rendering, clicking an item emitting the right event,
   the search row's placeholder/echo.
3. **Windows E2E** ([tests/e2e.rs](tests/e2e.rs)): launches the real PathWarp binary against a
   real `IFileOpenDialog` (via [src/bin/dialog_host.rs](src/bin/dialog_host.rs)) and asserts
   docking/foreground/reopen behavior with Win32 probes. These are `#[ignore]`d (need an
   interactive desktop and are sensitive to other foreground-grabbing tools) and run via
   `just e2e`.

Run layers 1–2 with `cargo test` (or `just test`); layer 3 with `just e2e`. CI runs layers
1–2 on every push; E2E runs on a schedule / manual dispatch (see `.github/workflows/e2e.yml`).

[`egui_kittest`]: https://docs.rs/egui_kittest/

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
