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

All 13 cases have passing coverage on the measured desktop, across a full run and
one focused follow-up. Each invocation was separately approved:

| Invocation | Result | Duration |
| --- | --- | --- |
| Initial `just e2e` | 6 passed, 7 failed | 41.94 s |
| Revised `just e2e` | 12 passed, 1 failed | 28.71 s |
| Focused search-preservation case | 1 passed, 12 filtered | 2.02 s |

The initial run exposed a pending-interaction transition after unconfirmed
activation. It also identified probe/fixture problems: cross-process Edit text now
uses bounded `WM_GETTEXT`, coordinates are physical and synthetic Shell callbacks
receive a foreground grant from an owned fixture window.

The last failing case had not confirmed that its input had reached the search
editor before opening settings. The focused case now reads back the initial text,
opens settings, verifies Ctrl+G exclusion, returns to the original dialog and reads
back the same text. Only this test changed between the last two invocations;
the product binary was identical (SHA-256
`BDF23B8F083040C032A8361B4DE9C29DADA3F9CFB688ADDE904F4C80CE207DE5`).
This records combined case coverage, not a single 13/13 green invocation.

The full revised run measured six overlay edge arrangements on each of two
attached monitors at 192 DPI and 144 DPI. No new product runtime code changed
after that run.

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
