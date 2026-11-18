# Context-menu address edit entry — throwaway prototype

**The one context-menu request returned successfully, but no Edit-address
command was observed or invoked. Popup observation has a filter defect, so
this is a provisional scoped result, not a complete negative finding.**

The worker finished normally and preserved the original dialog state. It
made one `IUIAutomationElement3::ShowContextMenu` call (41.706 ms), zero
Edit-address Invoke calls and no other provider actions. Before/final native
address-editor samples were hidden. Visible writable editor entry and
arbitrary-folder navigation remain unverified.

This is [Prototype address edit entry with corrected context-menu
observation](https://github.com/inaku-Gyan/PathWrap/issues/85). The human
answered “ok” to [the exact single-case protocol](https://github.com/inaku-Gyan/PathWrap/issues/84#issuecomment-6988637950).
The investigation addresses a specific observation gap in the earlier
toolbar context-menu test; it performs no broad pattern inventory.

## Observations and implementation defect

The scoped observer completed 16 native popup polls and 16 successful
Menu-only descendant lookups inside the original dialog over **3155 ms**.
Native poll selections were empty; no Menu array element was fetched, no
menu branch was captured and no eligible Edit-address command was found.
Successful array lengths were not separately logged in this version;
absence of GetElement calls is a control-flow observation, not a recorded
array-length value.

However, the executed native filter excluded any top-level HWND present in
the before-window set unless its class was `#32768`. That also excluded a
pre-existing hidden nonstandard popup if it became visible after the
request. The baseline contained **18 hidden windows** subject to this
filter, including Auto-Suggest Dropdown and ComboLBox windows. The trace
does not establish whether any such window actually became visible.

This exclusion is a mistake in the observation code. It leaves a reused-
window blind spot despite the native enumeration and Menu-only dialog
lookup. The conclusion is limited to the inspected selections. The raw
program field `complete=true` describes execution/budgets; it must not be
read as complete popup coverage. The derived summary explicitly records
`popup_observation_complete_under_selected_intent=false`.

No claim that all context menus are absent, that the toolbar cannot expose
Edit address, or that UIA navigation is impossible follows from this run.
The observer requires a code correction and a separately authorized rerun
before this gap can be resolved. The selected protocol forbids an automatic
rerun; exactly one GUI case has been executed.

## Exact execution and evidence

- [Executed Rust source captured with this report](../../../src/bin/uia_address_context_edit_prototype.rs).
- [Chronological scrubbed trace](open.jsonl).
- [Derived observations, coverage-gap classification and independent truth](summary.json).
- [Environment and executed source/binary hashes](environment.json).

`just build` produced the debug executable, which a hidden-console UTF-8
launcher invoked once without arguments. At the captured source revision,
the same case is runnable with:

```powershell
cargo run --bin uia_address_context_edit_prototype
```

The owned STA Open host and native observer derive from the accepted
read-only fixture, kept unchanged beside the new source. The host
initializes A, filename `pending-prototype-name.csv` and CSV index 2. Its
retained IFileDialog is setup and independent truth only, not an external
production navigation capability.

The client fresh-binds the original HWND/PID and unique native address
toolbar beneath Breadcrumb Parent, checking UIA/native identity,
AutomationId 1001, enabled/visible state and a fresh element comparison. A
separate MTA action thread requests the context menu while the MTA observer
continues. Native PID-scoped window evidence is saved before potentially
blocking UIA menu queries. Unrelated application UIA names/trees and a
whole-dialog pattern catalog are excluded.

At most one unique, owned, live visible/enabled MenuItem whose normalized
caption is exactly 编辑地址 or Edit address can receive Invoke. A second
capture confirms uniqueness/identity, followed by fresh semantic/state and
menu-ancestor checks. No candidate reached this path in the executed case.
The conditional Invoke guards and post-command editor observation remain
runtime-unexercised by this result.

## Bounds, timing and preservation

The worker used **68 explicit UIA calls: 67 reads and one action**. All 68
starts and ends match by call ID, including the concurrent action path.
No query error or missing call end was observed. Only the address-toolbar
element was captured; two native binding seeds were registered. No bound
was reached. Total worker work was **3282 ms**; supervised worker lifetime
was **3315 ms**, without timeout. The menu-discovery interval was 3155 ms
against a soft three-second deadline; it is not a hard three-second timing
claim.

Limits were 16 window seeds, 64 menu/address elements, depth 8, 512 explicit
UIA calls, a ten-second soft worker budget and fifteen-second external
supervisor. Native enumeration/identity guards and internal RPCs are not
separately counted as UIA calls. Synchronous provider calls can overrun soft
deadlines; active/incomplete calls remain distinguishable.

Thirty fresh native polls during the worker interval and three afterward
retained A, the initialized filename, CSV index 2 and the same live visible
dialog. No post-baseline poll drift was recorded. No B setter or navigation
commit ran. Read-only state preservation around this menu request does not
establish successful-jump preservation or production arrival verification.

Native ComboBoxEx32, ComboBox and Edit were hidden in their before/final
samples. No Edit-address command ran, so the post-command observation phase
was not entered. These samples do not prove that every instant of the
discovery interval had identical visibility.

## Environment and checks

Windows **26300.9974 / 26H2**, AMD64, zh-CN, Rust **1.100.0**. Listary desktop
helpers were absent pre/post, but Listary.Service.exe PID 6176 and automatic
ListaryServiceV2 remained running in Session 0. No service was stopped; no
complete component isolation or causality is claimed.

Before the action, foreground belonged to another process, PID 4304 with
native class `CASCADIA_HOSTING_WINDOW_CLASS`. After observation it belonged
to the original owned dialog, PID 39044, HWND 5310948, class `#32770`.
The unrelated application's identity/title/tree were not queried. No
explicit foreground-forcing API was called, and the cause of the change
was not isolated. Integrity levels were not independently measured.

`just check --ci`, `just build` and `just test` passed: **47 existing tests
plus two exact-caption guard tests**, with five interactive/diagnostic tests
ignored. Justfile's temporary newline normalization was restored to its
original bytes. No `just e2e` was invoked. The source has one context-request
call site and one guarded Invoke call site, with no other navigation
provider action or synthetic input.

Source and binary hashes matched before/after execution. The executed
source remained unchanged during evidence analysis. Raw/summary/environment
privacy scans passed. All owned processes exited and stderr was empty.

No manual editor preparation, clipboard, SetFocus, setter, arbitrary menu
command, default-action fallback, alternate backend, service shutdown,
explicit foreground forcing, automatic rerun, product change, PR or merge.
Owned fixture initialization and teardown are separate from measured
actions. This prototype remains open for human review and correction of
the observation gap; the navigation contract remains blocked.
