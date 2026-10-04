# PathWarp (still under development)

PathWarp is a Windows desktop application for quickly switching target folders when a system Open/Save file dialog appears.

The app listens to file dialog state and shows a lightweight overlay with paths from currently opened Explorer windows, reducing manual folder navigation.

The current code implements the core flow: Explorer path collection, foreground file-dialog detection, an activating overlay editor, and UI Automation based folder injection. Windows interactive verification is still required for the real dialog and focus hand-off paths.

## Download and run

Download the latest Windows release from [GitHub Releases](https://github.com/inaku-Gyan/PathWrap/releases). PathWarp is distributed as a portable Windows 10/11 x64 ZIP; it does not require an installer or administrator permission.

1. Download `PathWarp-vX.Y.Z-windows-x64.zip` from the release you want.
2. Extract it into a directory of your choice. The archive contains a same-named root directory, `PathWarp-vX.Y.Z-windows-x64`.
3. Open that directory and run `PathWarp.exe`.

### Upgrade

PathWarp stores its settings in `pathwrap-store\config.json` beside `PathWarp.exe`. Keep that directory when replacing the executable if you want the saved theme preference and other settings to remain. If you start PathWarp with `--storage-dir <PATH>`, that directory is used instead; relative paths are resolved beside the executable. If the selected directory cannot be used, PathWarp warns and continues in memory without saving settings.

### Download and run problems

If a download is incomplete, download the ZIP again from [GitHub Releases](https://github.com/inaku-Gyan/PathWrap/releases) and extract it fully before starting `PathWarp.exe`. For other problems, open an [issue](https://github.com/inaku-Gyan/PathWrap/issues) with the release version and a short description of what happened.

## Features

- Detects system Open/Save file dialogs and docks a lightweight overlay flush beneath them
- Reads active Explorer window paths and displays them as selectable items
- Click the search control to type-to-filter, use up/down selection, and press Enter or double-click
  to jump the dialog to that folder; its rounded frame has a light hover accent and the editor
  caret communicates keyboard focus
- Use the sun/moon control beside the search frame to switch between light and dark palettes;
  right-click it to follow the Windows theme or choose a fixed palette
- Focus-aware: clicking the overlay makes it the foreground editor while the current file-dialog session remains visible and tracked
- Built with Rust + egui/eframe + windows-rs

## Architecture

PathWarp is layered as `core/` (platform-independent decision logic) ↔ `os/` (Win32
integration) ↔ `ui/` (egui presentation), wired together by a thin `app.rs` shell and
decoupled by channels. Key design decisions:

- **Pure controller state machine** ([src/core/controller.rs](src/core/controller.rs)): a
  `Controller::step(env, event) -> Vec<Effect>` state machine owns **all** show/hide, docking,
  injection, debounce and suppression decisions. Time and the foreground window are injected
  via `Env`, so every timing rule is deterministically unit-testable. `app.rs` only collects
  events (dialog channel and egui input/mouse responses), calls `step`,
  and executes the returned `Effect`s — it contains no decision logic.
- **Activating overlay** ([src/os/window_ext.rs](src/os/window_ext.rs)): the eframe window
  carries `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`, **re-asserted every frame** in [src/app.rs](src/app.rs).
  Any stale `WS_EX_NOACTIVATE` bit is explicitly cleared, and no `WM_MOUSEACTIVATE` override
  blocks activation. Clicking the overlay therefore gives the embedded `TextEdit` a real
  Windows foreground/focus path. The controller treats the overlay as part of the tracked
  dialog session while it is foreground, so the row stays visible. "Hiding" moves the window
  off-screen while keeping it `WS_VISIBLE` (never `SW_HIDE`) — a hidden window stops receiving
  paints and would starve eframe's event loop. Docking uses `SetWindowPos` in **physical pixels**
  from the dialog's DWM frame bounds; the fixed overlay height is scaled from logical pixels
  using the dialog DPI.
- **glow renderer** ([src/main.rs](src/main.rs), `Cargo.toml`): eframe is pinned to the glow
  (OpenGL) backend instead of the default wgpu. wgpu's Windows HWND surface only advertises an
  opaque `CompositeAlphaMode`, so a transparent window renders its transparent pixels as black;
  glow composites transparency via the window's own alpha + DWM, giving the overlay real
  rounded corners and drop shadow. It also avoids noisy Vulkan-loader errors from unrelated
  third-party layers.
- **Text editing** ([src/ui/window.rs](src/ui/window.rs)): the search row uses an egui
  `TextEdit` bound directly to the controller query. The widget owns its blinking caret,
  selection, clipboard, paste, backspace, and platform text/IME events; navigation, Enter, and
  Escape are mapped to controller actions. There is no custom caret glyph and no global keyboard
  hook competing with the focused editor.
- **UI Automation injection** ([src/os/dialog.rs](src/os/dialog.rs)): locates the filename edit
  and the default button via UIA, then `ValuePattern::SetValue` + `InvokePattern::Invoke`.
  If no suitable button is found, it falls back to sending Enter to the filename edit.
  The operation is synchronous, has no sleeps, and does not intentionally take focus on
  modern `IFileDialog` dialogs.
- **Dialog detection** ([src/os/monitor.rs](src/os/monitor.rs)): `SetWinEventHook` wakeups +
  adaptive polling, matching class `#32770` plus structural child-class evidence to avoid
  false positives on generic message boxes.

When no dialog is being tracked, the monitor polls at 30 ms. During tracking it polls at 8 ms,
and WinEvent notifications wake it sooner when the foreground, focus, or window visibility
changes. A dialog is hidden only after three consecutive lost checks, while the controller
applies a 120 ms disappearance grace period and a 150 ms foreground-loss grace period. The
foreground-loss rule treats the PathWarp overlay as a continuation of the tracked dialog session.

## Development

### Planning and issue tracking

Project planning, decisions, dependencies, and the product backlog live in
[GitHub Issues](https://github.com/inaku-Gyan/PathWrap/issues). The
[Wayfinder migration map](https://github.com/inaku-Gyan/PathWrap/issues/22)
indexes the workflow migration and its decisions; `AGENTS.md` and
[`docs/agents/issue-tracker.md`](docs/agents/issue-tracker.md) describe the
repository entry points and issue operations.

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

### Maintainer release flow

1. Update the Cargo package version and let the ordinary CI checks pass on `main`.
2. Create and push the protected release tag for that version; the first formal release uses `v0.1.0`.
3. Observe the release workflow and the GitHub Release it creates.

When a formal release is ready, the release workflow builds and uploads the release assets itself. Maintainers do not manually upload assets or require a separate interactive acceptance record as a release gate. Release notes use GitHub's automatically generated change list; runtime instructions stay in this README.

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
   interactive desktop and are sensitive to other foreground-grabbing tools) and run locally via
   `just e2e`. The GitHub CI workflow covers formatting, Clippy, build, and
   the non-ignored test suite; it does not run E2E.

Run layers 1–2 with `cargo test` (or `just test`); run layer 3 manually with
`just e2e`. The GitHub CI workflow does not run E2E: it runs formatting,
Clippy, build, and the non-ignored test suite on pushes to `main` and pull
requests that touch the configured paths.

[`egui_kittest`]: https://docs.rs/egui_kittest/

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
