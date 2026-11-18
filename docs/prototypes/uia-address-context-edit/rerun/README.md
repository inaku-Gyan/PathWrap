# Corrected context-menu entry rerun — throwaway evidence

**The corrected run observed no menu or Edit-address command in its
sampled scopes and did not establish visible editing.** One high-level
ShowContextMenu request returned successfully after **44.643 ms**; zero
Edit-address Invoke calls followed. The native address editor remained
hidden in before/final samples.

The human explicitly authorized [one additional corrected case](https://github.com/inaku-Gyan/PathWrap/issues/85#issuecomment-6989409657).
The exact executed source is [commit f68a7ac19f775ec6bcf950cf510a3041f193e020](https://github.com/inaku-Gyan/PathWrap/blob/f68a7ac19f775ec6bcf950cf510a3041f193e020/src/bin/uia_address_context_edit_prototype.rs).
The [first run](../README.md) is preserved separately, with its original
coverage defect recorded.

## Evidence and actual observations

- [Scrubbed chronological trace](open.jsonl).
- [Derived summary and independent native truth](summary.json).
- [Environment, authorization and source/binary hashes](environment.json).

The debug executable was invoked once without arguments by a hidden-console
UTF-8 launcher. To run the same program at its recorded revision:

```powershell
cargo run --bin uia_address_context_edit_prototype
```

The worker recorded the corrected policy
`all_visible_fixture_top_level_windows_including_preexisting_handles`.
Across **15 native popup polls** and **15 successful Menu-only dialog
lookups**, every selected native popup list was empty and every logged
Menu-array length was zero. No menu item, eligible command, ambiguity or
Invoke binding was reached.

The actual menu-discovery interval was **3021 ms** against a soft
three-second deadline. The worker used **66 explicit UIA calls: 65 reads
and one action**. Every call start/end matched by ID; no query error,
missing end, bound hit or timeout was observed. One address-toolbar element
was captured and two binding seeds registered. Worker work was **3151 ms**,
and supervised lifetime **3182 ms**.

The source's concurrent action/observation design did not wait for the
context request to finish before permitting observation. In this case the
request did finish before the first native poll: that poll was 97 ms after
action start and 53 ms after return. The modal-pending path was therefore
not stressed, and earlier or very brief transient UI is not ruled out.
High-level success is not proof that a menu appeared; raw successful
HRESULT is null rather than a fabricated S_OK.

## Relation to the edit-entry question

The known exclusion of pre-existing nonstandard HWNDs was removed in this
executed version. Before-window membership is provenance only. Every
visible own-process top-level HWND other than the original dialog is
eligible for observation, including reused handles. SysShadow is native
evidence only; original-dialog Menu-only lookup also covers virtual menus
inside that anchor.

No reused visible popup was observed. Thus this case uses the corrected
selection policy but supplies no positive runtime example of binding such
a popup. The result supports only “no menu/entry command observed during
this run in the declared sampled scopes”, not a universal absence or
impossibility claim.

The guarded command path allows exactly one owned, live visible/enabled
MenuItem matching normalized 编辑地址 or Edit address, with fresh uniqueness,
identity/ancestor and Invoke checks. No candidate was found, so neither its
Invoke nor the post-command writable-Value check ran. Native editor
visibility was sampled before and at the end, rather than continuously
through every instant.

## Independent state and environment

Twenty-nine fresh native host polls during the worker interval and three
fresh polls afterward retained A, initialized
`pending-prototype-name.csv`, CSV type index 2 and the same live visible
dialog. No post-baseline poll drift was recorded. Native ComboBoxEx32,
ComboBox and Edit were hidden in both recorded phases.

The owned STA IFileDialog is fixture setup and independent truth only;
the external MTA worker did not acquire it as a navigation backend.
No B setter, commit, successful jump or production-available arrival
verification was attempted.

Windows **26300.9974 / 26H2**, AMD64, zh-CN, Rust **1.100.0**. Listary
desktop/helpers were absent pre/post; automatic ListaryServiceV2 and
Listary.Service.exe PID 6176 remained running in Session 0. No service was
stopped and complete component isolation is not claimed. Integrity levels
were not independently measured.

The in-worker foreground samples changed from another process, PID 4304,
class `CASCADIA_HOSTING_WINDOW_CLASS`, to the original owned dialog, PID
21636, HWND 1445480, class `#32770`, before teardown. The unrelated
application's identity/title/tree were not queried. No explicit
foreground-forcing API ran; causality and a pre-foregrounded-dialog case
were not isolated.

Source and executable SHA256 matched the previously checked correction
before launch and remained unchanged after the run:

- Source: `684265510cb4e6394ad6cdc94daef5746ce5124c1a5513ee9f94f13f47a356c6`.
- Executable: `a2133c090037ab25c07054b34755cd4d9a8a290f55283e93c2150b89fe146e10`.

All owned processes exited and stderr was empty. The first run's trace,
environment and summary are unchanged. New JSON/call-pairing/preservation
checks passed. Privacy scanning found no local-profile or username matches
outside the allowlisted public repository URL.

## Limits and disposition

The unchanged bounds were 16 own binding seeds, 64 menu/address elements,
depth 8, 512 explicit UIA calls, ten-second soft worker budget and
fifteen-second supervisor. Native enumeration/identity guards, internal
RPCs and reference releases are not separately counted as UIA calls.
Synchronous reads may exceed soft deadlines.

No broad inventory, manual editor preparation, simulated input, clipboard,
SetFocus, setter, arbitrary command/default-action fallback, alternate
backend, explicit foreground forcing, service shutdown or further automatic
rerun. Owned fixture initialization and teardown are separate. The checked
source had passed 49 tests (47 existing plus two caption guards), with five
interactive/diagnostic tests ignored; no just e2e was invoked. No product
change, PR or merge.

The corrected observation does not demonstrate visible writable editor
entry or arbitrary-folder navigation. This [prototype](https://github.com/inaku-Gyan/PathWrap/issues/85)
remains open pending human acceptance of the bounded result.
