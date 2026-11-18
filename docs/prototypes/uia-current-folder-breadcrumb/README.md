# One current-folder breadcrumb Invoke — throwaway automatic probe

**The automatic current-folder breadcrumb Invoke did not expose a visible
address editor or navigate to B.** One exact Invoke returned S_OK in
**45.366 ms**. A fresh owned-host `OnFolderChange` callback then reported A,
and all 27 independent native polls during the three-second observation
reported A. Focus moved to the address toolbar. Native ComboBoxEx32,
ComboBox and Edit stayed hidden in every captured frame.

This is [Prototype one Invoke on the current-folder
breadcrumb](https://github.com/inaku-Gyan/PathWrap/issues/78), selected after
the human requested **“你重新尝试自动测试一下”** (try the automatic test again)
in the [next-action decision](https://github.com/inaku-Gyan/PathWrap/issues/77#issuecomment-6985203288).
The human chose continued automatic testing; the agent selected the exact
mechanical probe within that instruction. The proposed human-established
editor diagnostic was not performed.

## Measured result

| Observation | Single measured Open case |
| --- | --- |
| Exact target | Current A child of the address breadcrumb toolbar |
| Fresh pre-action metadata | SplitButton 50031, Legacy role 62, default `打开` (Open), Invoke available |
| Provider action count | One Invoke; zero setters, focus calls or follow-up actions |
| Invoke HRESULT / duration | S_OK / 45.366 ms |
| Native folder callback | A, seven ms after Invoke returned |
| Independent native polls during observation | 27, all A with initialized filename/type and original live dialog |
| Visible native address editor | No; ComboBoxEx32, ComboBox and Edit hidden in all four sampled frames |
| Visible owned popup in the sampled after frames | None |
| Fresh main UIA tree | 108 elements before and in each after capture |
| Address toolbar child metadata | A still exposed the recorded SplitButton/Open metadata in the after captures |
| Recognized Go-name candidates in captured address trees | Zero under the explicit heuristic in summary.json |
| Actual observation duration | 3000 ms |

The post-action native callback is evidence beyond the HRESULT or focus
change. It reports the **same** A folder; it does not demonstrate an
arbitrary requested-folder commit, visible editing or arrival in B. No
destination was assigned and no Go action was attempted. Preserving the
pending file operation during this A-only case does not validate
preservation after a successful jump to a different folder.

No visible popup was captured; this does not exclude a transient state
between snapshots. Invoke, Value, ExpandCollapse and Legacy were the four
patterns inspected by the inherited inventory. A missing recognized Go
name in these captures is not proof that every provider pattern/state lacks
a commit mechanism.

## Exact case and boundaries

Run on Windows with this throwaway checkout:

```powershell
cargo run --bin uia_current_folder_breadcrumb_prototype
```

- [Exact executed source](../../../src/bin/uia_current_folder_breadcrumb_prototype.rs).
- [Chronological scrubbed trace](open.jsonl).
- [Extracted observations, callbacks and timing](summary.json).
- [Environment, process/service checks and executed source/binary hashes](environment.json).

The binary derives from the accepted single-ComboBox-Invoke harness while
preserving that source unchanged. An owned STA modern Common Item Open
fixture starts at A with filename `pending-prototype-name.csv` and CSV
filter index 2. A separate external MTA UIA worker binds to the original
dialog and fixture process. The owned native IFileDialog supplies setup and
independent read-only truth; it is not a capability acquired by an ordinary
external product client.

The target is discovered beneath Address Band Root and Breadcrumb Parent:
the native ToolbarWindow32 with observed AutomationId `1001` and name
reflecting the exact fixture A path. Its unique direct A child is matched
after removing the observed leading left-to-right marker. Native ancestry,
UIA parent identity, process, enabled/offscreen state and exposed Invoke
are checked. The accepted earlier trace recorded both
SplitButton/Legacy-62/Open and MenuItem/Legacy-12/Press pairs for this child;
the new probe permits only those observed pairs and re-reads the current
pair immediately before the call. The actual executed pair was
SplitButton/Legacy-62/Open. These are this observed zh-CN fixture's metadata,
not a universal target contract or a guarantee of action semantics.

The one action is Invoke on that child. There is no manual setup, simulated
input, clipboard navigation, SetFocus, SetValue, extra action, alternate
backend or Save comparison. Closing the owned fixture is teardown, not a
navigation action. Missing or ambiguous target identity stops without
substitution. One measured GUI case ran; it was not rerun for an unhelpful
outcome.

## Timing and independent observations

Full observations start at offsets 0, 1000 and 2000 ms and completed at
offsets **432, 1341 and 2650 ms**. All eight tree captures were complete, no
observation cutoff fired, and monotonic observation elapsed was **3000 ms**.
Individual synchronous UIA calls cannot be canceled by a deadline check;
the separate worker retains the 15-second supervisor watchdog. Its actual
lifetime was **3487 ms**, without a timeout.

All 27 fresh native polls in the observation interval reported folder A,
the initialized filename, type index 2 and the original live/visible
dialog. Four additional fresh polls after worker exit confirmed that state
before cleanup. All owned prototype processes exited.

## Environment and limits

Windows **26300.9974**, AMD64, zh-CN, Rust **1.100.0**. Processes inherited the
user token; integrity levels were not independently measured.

Immediate pre/post wrapper checks found the Listary desktop application
and interactive helpers absent. `Listary.Service.exe` PID 6176 remained in
Session 0 with no main window; `ListaryServiceV2` stayed running/automatic.
No service or unrelated application was stopped. This retains the accepted
desktop-app-exited boundary and does not establish complete third-party
isolation or causality. The wrapper records these checks; the standalone
binary itself does not enforce them.

Before Invoke, the foreground/focused surface belonged to Windows Terminal
(`CASCADIA_HOSTING_WINDOW_CLASS` / `TermControl`), rather than the fixture.
Afterward the owned dialog was foreground and its address toolbar focused.
Unrelated titles and trees were not captured. This foreground condition is
an explicit sample limit; no controlled-foreground repeat was added.

Runtime scrubbing normalizes the fixture root and redacts other absolute
or profile-relative paths. Source hashes in environment.json identify the
exact source used for the measured binary; the executed source was not
changed after the GUI case.

## Validation and disposition

`just check --ci`, `just build` and `just test` passed: **47 existing tests
passed**, five interactive/diagnostic tests remained ignored. No new
prototype unit tests were added. The announced single owned-fixture GUI
probe ran; `just e2e` was not invoked. The Justfile checkout's EOL-only
formatting difference was temporarily normalized for its required check.

Product code is unchanged. Preserve this artifact on its throwaway branch
with no PR or merge into main. The result awaits human review. The
entry/edit/commit sequence and reliable external arrival-verification
contract remain unresolved; this bounded result does not make the product
navigation contract implementation-ready.
