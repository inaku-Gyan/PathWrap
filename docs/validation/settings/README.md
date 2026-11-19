# Settings and current-user startup validation

Contract: [确认首版设置的保存失败、自启状态与窗口契约](https://github.com/inaku-Gyan/PathWrap/issues/98#issuecomment-7006695913).
Implementation base: `01fa41ba1ec23492be544fdaf7e129cf1b3a94c2`.

## Automated verification

The implementation separates configuration save results, registration observations and
native window state. Tests cover failed file replacement preserving the old file,
explicit retry at the original location, malformed-load diagnostics, command quoting,
full-command length/path rejection, stale revisions, foreign changes, readback failure,
unknown content, other configurations, and settings/search interruption.

`startup_argv_probe` is a console fixture. Its integration test runs the actual Rust
Windows argv parser with Chinese, spaces, UNC paths and final backslashes. The registry
adapter test writes only a uniquely named non-Run HKCU test key and verifies an unrelated
value survives deletion of the owned value. It does not change real login startup.

On 2026-11-19, `just check --ci`, `just build` and `just test` passed.
The noninteractive run passed 101 tests: 85 library/model/adapter tests, 14 egui UI
tests and two process integration tests. One owned blocking-child fixture and the
13 interactive desktop tests remain ignored in this command.

The focused regression `unfocused_settings_open_does_not_end_the_pending_interaction`
was observed failing before the pending-open fix and passing afterwards. It replays
the actual native command queue without creating any window or injecting input.

## Interactive desktop verification

Five new ignored desktop tests are prepared alongside the eight existing regressions.
They use owned windows and temporary configuration directories, covering initial open
without a file dialog, singleton reuse, minimize/restore, close/reopen, theme persistence,
visible load/save failures and retry, preserved search text, Ctrl+G exclusion, denied
activation, tray menu cancellation/selection and notification icon recovery.

The first individually approved desktop run completed with 6 passing and 7 failing
tests (41.94 seconds). This is preserved as a failed run, not desktop acceptance.
The revised version corrects the pending-interaction transition after unconfirmed
activation, reads cross-process Edit text with bounded `WM_GETTEXT`, uses physical
coordinates consistently and supplies the foreground grant missing from synthetic
Shell callbacks. A repeat of the 13 tests is awaiting fresh per-run permission.

The tray test delivers the documented mouse/keyboard callback to its owned native
notification window, then uses actual input on the native menu. Recovery removes only
the test process's icon and sends `TaskbarCreated`; it does not restart Explorer.
This distinguishes callback integration from a physical icon gesture or a real Shell crash.
The synthetic-callback fixture grants ordinary foreground permission from its own
plain window before delivering the notification. It does not force-focus the popup
or assist activation after the user command.

No actual login/logoff/reboot or Windows Startup Apps permission toggle has been run.
Registration presence never claims Windows permission or successful execution at login.
No real Run value has been changed by these tests.
