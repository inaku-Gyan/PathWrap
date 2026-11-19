# Settings open foreground readback

The clean-desktop acceptance task exposed an interaction-state defect: an
activation notification can clear the pending-open latch before the final
foreground readback reports that settings did not focus.

## Evidence

- The original full E2E run reported 12 passed / 1 failed. The failed case was
  `settings_report_unfocused_open_without_bypassing_foreground_lock`.
- A focused diagnostic reproduced the failure: the file dialog remained
  foreground and the overlay remained on screen after a further 2030 ms.
- A later diagnostic with temporary logs passed its instantaneous assertion,
  while recording `begin open`, an active notification, and an unconfirmed
  foreground readback. This pass does not invalidate the earlier failure.
- Replaying that notification before an unsuccessful final readback through the
  real `Shared` methods made
  `unfocused_settings_open_does_not_end_the_pending_interaction` fail without
  desktop input. The same test passes after the fix.

The fix sets the pending-open latch from the final foreground readback. A failed
readback keeps the interaction pending until a later activation or close. The
regression also covers an open without an intervening activation notification,
then activation/deactivation and close. It does not require Windows to produce a
particular notification timing.

[WM_ACTIVATE](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-activate)
describes activation notifications;
[GetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow)
provides the separate foreground observation. The event order above is local
evidence, not a guaranteed Windows sequence.

## Current checks

`just check --ci`, `just build`, and `just test` pass on Windows. The latter has
103 passing automated tests; 13 interactive tests are ignored there.

Desktop acceptance of this fix is pending. Temporary diagnostic logging has
been removed. No successful Open/Save UI acceptance, Windows 10, physical
single-display, or complete clean-desktop acceptance is claimed.
