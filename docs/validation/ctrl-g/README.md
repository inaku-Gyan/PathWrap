# Session-scoped Ctrl+G: Windows validation

This change implements [session-scoped Ctrl+G focus](https://github.com/inaku-Gyan/PathWrap/issues/67) under the [accepted simple focus policy](https://github.com/inaku-Gyan/PathWrap/issues/64#issuecomment-6927668785). It builds on verified navigation commit `eaae4e7f46a400de98bd16aba537dedaf64b1bec` on the separate `feat/session-ctrl-g` branch.

Ctrl+G while the live tracked file dialog or PathWarp is foreground requests normal activation of PathWarp. The existing foreground TextEdit focus path selects its search editor. Other keys and Ctrl+G in unrelated windows pass through. Escape-dismissed, closed and replaced sessions cannot be summoned by a queued shortcut.

## Implementation

- [Shortcut listener](../../../src/os/shortcut.rs): a dedicated message thread owns a foreground WinEvent observer, a native Ctrl+G hotkey registration, and a narrow key-time scope guard. Session updates and foreground notifications register/unregister the chord. Each registration gets a new ID, so queued WM_HOTKEY messages cannot bind to a replacement session.
- The key-time guard checks G key-downs against the published scope and live foreground pair. It unregisters an obsolete binding before forwarding that key, covering the first chord after a foreground switch. It always forwards input, never activates a window or edits text, and uses a nonblocking scope read. It is not an alternative activation path.
- The native hotkey notification checks the current foreground pair, original dialog PID/process creation identity and native visibility/enabled state, then makes the sole normal activation attempt. A session-bound result reports the observed outcome to the UI. The UI never retries activation from a delayed shortcut result. Repeated presses use the same straightforward editor focus behavior; MOD_NOREPEAT suppresses held-key repetition.
- [Controller](../../../src/core/controller.rs) and [app shell](../../../src/app.rs) publish the active scope and ignore results from stale sessions or unrelated foreground apps. The existing TextEdit focus path selects the editor. No query/selection snapshot or restoration feature is added.
- [Window activation](../../../src/os/window_ext.rs): an already-foreground window succeeds by observation; otherwise request SetForegroundWindow once and check the actual foreground. A denied request shows a focus notice, a later success clears it, and a navigation recovery message keeps priority.
- Observer/registration failure produces a distinct Ctrl+G-unavailable notice. There is no alternative hotkey or activation fallback. Normal shutdown unregisters, unhooks and joins the owned thread; unregistration failure retires the thread. Registration IDs are never reused during that listener's lifetime; exhaustion fails closed until restart.

The trigger uses [RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey) / [WM_HOTKEY](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-hotkey). The guard follows the forwarding rules of [LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc). Activation remains subject to Windows' [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) restrictions. These tests do not establish atomic routing across every simultaneous foreground/key transition, external hook interference or hook removal.

## Noninteractive checks

`just check --ci`, `just build`, and `just test` passed. There are 71 passing noninteractive tests: 39 library tests, 31 app/config/UI tests, and one real product worker entry-point test. Eight ignored entries comprise seven interactive/diagnostic E2E tests and the intentionally blocking worker fixture invoked by its parent timeout regression.

New regressions cover original registration/session binding, replaced process identity, observer startup/shutdown, unrelated foregrounds, pending disappearance, Escape dismissal, no activation retry from a shortcut result, unavailable/denied/later-success notices, and preservation of a navigation recovery message. The removed keyboard-hook implementation's modifier tests are not tests of native hotkey routing.

The checkout Justfile is restored byte-for-byte after its format check. The sandbox could not launch the installed pwsh.exe; required checks ran outside the sandbox with the same declared toolchain. No toolchain substitution was used.

## Interactive acceptance

The first user-approved `just e2e` invocation finished in 20.40 seconds: five existing cases passed; both new Ctrl+G cases failed at the actual focused-editor assertion. The denial-notice assertion passed before the second case failed after unlocking. See [first results](first-run-results.txt) and [its source/binary metadata](first-run-environment.json).

A second, separately authorized run selected only the two Ctrl+G cases. Both failed again in 8.59 seconds. [Focused observations](diagnostic/results.txt) showed foreground HWND different from the overlay, a focused Edit in the host PID and the product's denial notice. Thus this was an actual denied foreground transition, rather than a UIA-only focus mismatch. The [unsuccessful listener source](diagnostic/keyboard-hook.rs.txt) is retained as a noncompiled diagnostic asset; the old result files do not establish support.

Replacing keyboard-hook forwarding to the next UI frame with native hotkey notification delivery and activation in that handler fixed the real foreground/editor focus assertions. A third separately approved seven-case run passed six cases in 17.56 seconds, including denial and later recovery, but stopped the main Ctrl+G case at the unrelated window's expected G-count assertion. See [that result](native-first-results.txt) and [metadata](native-first-environment.json).

Adding the key-time obsolete-registration guard made the original scenario pass without changing its input sequence, delays or success assertions. This supports the asynchronous registration boundary as the passthrough fault; the failed run did not capture the actual counter value, so it does not independently prove how many G events were delivered. Temporary diagnostic probes were removed from compiled source; the final assertion retains useful failure context.

The fourth, separately approved invocation selected the two Ctrl+G cases on the final product and driver: **2 passed, 0 failed, 5 filtered out in 3.47 seconds**. [Final results](results.txt) and [source/three-executable hashes](environment.json) record this run. The five existing E2E cases passed in the earlier seven-case runs; they were not repeated after the final key-time guard. They are not counted as seven passes on the final binary.

All three required noninteractive checks passed again afterward, and the tested source plus product, host and E2E executable hashes remained unchanged. No named test processes remained. Hashes describe the tested working-file bytes; Git line-ending normalization can change source byte hashes after checkout.

The two new cases use the actual product executable and an owned host:

1. Ctrl+G from a real Open dialog must produce actual foreground and UIA-focused Edit state; typed test text must reach the editor. Repeated shortcuts retain editor focus and docking. An owned plain window counts delivered G key-downs to establish unrelated-window passthrough while the dialog is live and after it closes. Reopening the dialog must support the shortcut again.
2. The owned foreground host locks normal foreground changes. Ctrl+G must leave that dialog foreground and display the denial notice. After the host unlocks, the next shortcut must focus the actual editor and remove the stale notice.

The test driver's existing forced foreground helper is fixture setup only. Test keyboard/mouse input remains confined to the driver; product activation uses no AttachThreadInput, simulated input, or retry. These cases do not establish arbitrary host/elevation/desktop compatibility or every focus race.

The prior [navigation acceptance record](../uia-navigation/README.md) belongs to its recorded source/binary hashes. This Ctrl+G validation does not reclassify those earlier runs as tests of the new executable.

## PR review follow-up

The branch now builds on navigation review commit `f15d0d972498720aaee6e19cef7be9072e153b7b`. Independent Standards review had no material findings. Both Spec findings were in the navigation base and are corrected there: same-frame cancellation signalling after UI session invalidation, and recovery messages retaining the stopping stage plus last-read/unknown operation state. The Ctrl+G increment had no additional finding.

On the reviewed stack, `just check --ci`, `just build` and `just test` passed: **73 noninteractive tests passed, 8 ignored** (41 library, 31 app/config/UI and one product worker entry-point case). [Review-build source and executable hashes](pr-review-environment.json) identify this distinct build.

A fifth, separately authorized invocation ran `just e2e` on that reviewed stack: **7 passed, 0 failed in 15.28 seconds**, including the two Ctrl+G cases and the five existing interactive/diagnostic cases. [Review-build desktop results](pr-review-results.txt) record the complete selection. All 20 recorded source files and all three executable hashes still matched afterward, and no named product/fixture/test processes remained. This is a fresh whole-suite pass on the reviewed combination; the four earlier interactive runs above retain their original provenance. The full native Open/Save navigation chains were not repeated during this follow-up.

The navigation base also passed [Windows pull-request CI](https://github.com/inaku-Gyan/PathWrap/actions/runs/44428201074). The stacked Ctrl+G PR targets the navigation branch; the repository's pull-request CI trigger currently targets `main` only. Its local validation above does not claim remote CI for the combined stack; CI is still required after retargeting to `main`.
