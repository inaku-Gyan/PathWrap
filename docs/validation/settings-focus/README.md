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

## Checks at the first fix

`just check --ci`, `just build`, and `just test` pass on Windows. The latter has
103 passing automated tests; 13 interactive tests are ignored there.

The full desktop suite passed **13/13** in **26.54 s** on Windows 11 build 26300.
It includes the previously failing case and two diagnostic helpers. The run
used product commit `efffa20d1cbd54be9fbde5564cc47ce2a46957bd`, approval
`call_3551fadc06e147419054e2a053a0b8bb`. CI also passed for that commit.

Temporary diagnostic logging has been removed. Open/Save UI entry acceptance
encountered a tool permission block. A manual Open attempt then did not reach
the target: host polling stayed in A, the filename changed to masked path text,
type remained CSV, and no file acceptance was attempted. The stop stage is
awaiting the visible error. Save, Windows 10, physical single-display, and
complete clean-desktop acceptance are not claimed.

## Foreground child discovered by manual testing

The user kept the Open interaction in front for 20 seconds and still obtained
`Interrupted` at Assign. At failure, identity, visibility and enabled checks
were true. The unexpected foreground HWND was a live `Edit` belonging to the
fixture process, under the original dialog. It was not evidence of an early
return to chat. The original failed attempts remain recorded.

Foreground observations now resolve native child controls to their same-process
top-level parent. This uses `GA_ROOT`, not the owner chain: a separate owned
popup is still a separate interaction. Detection, cancellation, shortcut scope,
UIA guards and normal activation share this observation.

A hidden native Edit regression failed with direct handle comparison and passes
with parent resolution. It also checks an independent owned popup and a null
handle. `just check --ci`, `just build`, and all **104** automated tests pass.
Desktop and Open/Save acceptance of this second fix are pending. The 13/13 result
above predates it. Temporary guard diagnostics are removed; the owned fixture
now records foreground PID alongside its existing folder/filename/type samples.

The distinction follows
[GetAncestor](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getancestor).
