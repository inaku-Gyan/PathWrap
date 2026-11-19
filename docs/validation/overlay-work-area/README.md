# Work-area docking validation

The implementation and desktop driver were built from commit
`f2204415f508499d1486e6bd3a6e14eb488c9174`. This record was added afterward without
changing runtime or test inputs. Exact source and executable SHA256 values, Windows
build and run timestamps are in [environment.json](environment.json).

## Checks

- `just check --ci`: passed formatting and strict Clippy.
- `just build`: passed all-target compilation.
- `just test`: **85 passed**, 0 failed; 53 library, 31 app/config/UI and one owned-worker
  integration test. Eight desktop entries and one blocking-child fixture were ignored.
- One separately authorized `just e2e`: **8 passed**, 0 failed, **17.67 seconds**.
  The command wrapper took 18.454 seconds including startup.
- [Windows PR CI](https://github.com/inaku-Gyan/PathWrap/actions/runs/44441422876)
  passed format, strict Clippy, build and tests on the implementation commit.

All 28 captured source/toolchain inputs and all three executables had matching
hashes before and after desktop execution. No new `PathWarp` or `dialog_host`
process remained afterward. Sanitized output is in [results.txt](results.txt);
the raw diagnostic window inventory is not published.

## Recorded desktop results

Windows build 26300.9974 (26H2), two attached monitors:

| Monitor | Dialog DPI / scale | Work area (left, top, right, bottom) |
| --- | --- | --- |
| 0 | 192 / 200% | `(0, 0, 3072, 1824)` |
| 1 | 144 / 150% | `(-3840, -249, 0, 1839)` |

The owned per-monitor-aware Open dialog was moved through six arrangements on
each monitor. Independent Win32 probes checked its DWM visual bounds and the
actual native overlay rectangle, producing **12 passing samples**:

- Normal below placement retained the dialog's visual left edge and DPI-scaled height.
- A dialog flush with the work area's bottom placed the overlay above it.
- An overwide dialog clipped the overlay to the full work-area width.
- A dialog extending past the left edge kept the overlay inside that edge.
- Twelve-pixel gaps above and below produced a contained 12-pixel overlay.
- A dialog covering the work-area height kept the overlay visible inside the
  work area, overlapping the dialog.

The seven existing desktop cases also passed: click activation/docking, Ctrl+G
focus/repeated focus/unrelated-window passthrough, denied-activation recovery,
activation diagnostics, window inventory, click then reopen, and close then reopen.

## Scope

This run exercises the primary and negative-origin secondary monitor in one
mixed-DPI desktop. A separate single-display configuration, other taskbar edges,
additional monitor layouts and display hot-plug were not exercised. Pure geometry
tests cover side/top work-area offsets and extreme coordinates independently.
The very short-gap case establishes native containment; it does not establish
that all search content is usable at a 12-pixel height.

These checks establish placement and focus behavior on the recorded configuration.
They do not repeat the separate UIA folder-jump preservation acceptance.
