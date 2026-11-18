# One address ComboBox Invoke after exiting Listary — throwaway repeat

**The dropdown-opening/native-editor-hidden finding reproduced after the
Listary desktop application was exited.** One exact child Invoke returned
S_OK in 30.989 ms, exposed a visible ComboLBox with 25 ListItems and changed
the Button's Legacy default action from `打开` (Open) to `关闭` (Close).
The native ComboBoxEx32, ComboBox and Edit remained hidden. Actual folder A,
the initialized filename/type and the live dialog were unchanged.

This is the selected [Repeat the single address ComboBox Invoke with Listary
closed](https://github.com/inaku-Gyan/PathWrap/issues/76), following the user's
[next-probe decision](https://github.com/inaku-Gyan/PathWrap/issues/75#issuecomment-6984266170)
and explicit instruction to run it. The [previous accepted sample](https://github.com/inaku-Gyan/PathWrap/blob/b29d2c92d4324ec37cf172d49fc7f2df12a139de/docs/prototypes/uia-address-combobox-invoke/README.md)
remains preserved on its original throwaway branch.

## Environment boundary

The user exited the Listary desktop application. A broad preflight process
check then stopped before fixture creation or Invoke because the Windows
service remained. Read-only inspection identified only `Listary.Service.exe`,
PID 6176, Session 0, no main window, registered as the running automatic
service `ListaryServiceV2`. The ticket's precondition was clarified to record
this actual boundary before the sole GUI case ran.

Immediate pre/post case snapshots found no Listary desktop application or
interactive helpers; the Session 0 service remained running. No system
service or unrelated application was stopped. This is a **desktop-app-exited
sample**, not complete isolation from every Listary component or all possible
third-party integrations. One pair of samples does not establish causality.
Full process/service facts and timestamps are in [environment.json](environment.json).

## Comparison

| Observation | Previous accepted sample | Desktop-app-exited repeat |
| --- | --- | --- |
| Before foreground/focus | Listary-class window | Owned Open dialog / filename Edit |
| Invoke calls / HRESULT | 1 / S_OK | 1 / S_OK |
| Invoke elapsed | 38.043 ms | 30.989 ms |
| Address ComboBox subtree | 2 to 33 elements | 2 to 33 elements |
| Visible popup | ComboLBox and SysShadow | ComboLBox and SysShadow |
| ListItems | 25 | 25 |
| Exact child default action | Open to Close | Open to Close |
| Native ComboBoxEx32 / ComboBox / Edit | Hidden throughout | Hidden throughout |
| Actual folder | A throughout | A throughout |
| Pending filename / type index | Preserved / 2 | Preserved / 2 |
| Monotonic observation elapsed | 3419 ms | 3000 ms |

The repeat's after samples reported focus on the address ComboBox while its
native window remained hidden. The dialog remained foreground. This is not
proof of visible address editing. Its Value readback stayed empty, and no
recognized Go action appeared in the captured main address trees. The popup
ListItems expose Invoke with a Legacy double-click description; the Button
exposes Close afterward. No further action was invoked.

## Run and evidence

With the Listary desktop application and interactive helpers exited, on Windows:

```powershell
cargo run --bin uia_address_combo_clean_retest_prototype
```

- [Exact executed source](../../../src/bin/uia_address_combo_clean_retest_prototype.rs).
- [Scrubbed chronological trace](open.jsonl).
- [Extracted observations and snapshot timing](summary.json).
- [Environment and process/service pre/post checks](environment.json).

The standalone binary is derived from the previous exact source and retains
the same owned STA modern Open fixture, fully initialized native baseline,
external MTA UIA worker, current address-band/native ComboBox discovery and
exact direct-child/parent comparison. The name `地址`, Legacy role 43,
default action `打开` and Invoke support identify this observed zh-CN
fixture target, not a universal provider contract. Missing or ambiguous
identity stops without substitution.

The CLI creates only its owned fixture and scratch A/B folders. The retained
native IFileDialog is used for setup and independent read-only observations;
it is not an interface discovered by an ordinary external client. The one
provider action is Invoke on the exact child. There are no SetFocus/SetValue,
Close/list-item actions, manual editing, Save case, simulated input,
clipboard navigation or alternate navigation mechanism. WM_CLOSE is owned
fixture teardown, separate from that action count.

The process/service pre/post checks were performed by the launch wrapper and
recorded in environment.json; the standalone binary itself does not enforce
Listary absence. The initial stopped preflight was not an additional GUI
case. The single measured GUI case completed successfully and all owned
prototype processes exited.

## Observation timing and independent truth

Full snapshot rounds start at offsets 0, 1000 and 2000 ms. The late 2900 ms
round from the previous source was removed. A monotonic three-second budget
is checked before new observation work and during tree traversal; interrupted
captures are marked rather than assumed complete. Each round records its
actual start/end, and the entire UIA worker retains the separate 15-second
supervisor bound for a blocked provider call. Individual synchronous calls
can still block; this source does not claim that a deadline check cancels an
already-running provider call.

In this capture the three full rounds completed at offsets **691, 1502 and
2489 ms**. All trees were complete, no cutoff was hit, and the reported
monotonic observation duration was **3000 ms**. No timing-adjustment GUI
rerun occurred. These are measured timings, not a real-time scheduling
guarantee.

There were **26 fresh independent native polls** between Invoke returning and
the observation ending. Every one reported folder A, pending filename
`pending-prototype-name.csv`, CSV filter index 2 and the same live, visible
dialog. A final fresh native observation after the client exited confirmed
that state before teardown. No folder jump was attempted or observed.
Preservation during a dropdown-only case does not validate preservation after
successful navigation or a complete entry/edit/commit chain. Save and other
applications/providers remain untested.

## Publication and validation

Runtime scrubbing replaces the fixture root with `PROTOTYPE_ROOT` and
non-fixture absolute/profile-relative names with `NON_FIXTURE_PATH`.
Unrelated applications' titles and UI trees are not captured. Tree structure,
identities, states, provider patterns, action counts, HRESULTs and timing
remain in the raw trace.

`just check --ci`, `just build` and `just test` passed: 47 existing tests
passed, five interactive/diagnostic tests ignored. No prototype tests were
added. The announced owned Open capability check ran; `just e2e` was not
invoked. Product code is unchanged. The local Justfile checkout's CRLF-only
difference was normalized for its formatting check.

This evidence awaits human review. It does not select a subsequent action or
make the UIA navigation contract implementation-ready. Preserve it on this
throwaway branch with no PR or merge into main.
