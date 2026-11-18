# Initial-A address-control read-only discovery — throwaway prototype

**The selected read-only pass completed: 14 unique elements × 35 standard
patterns, 1515 explicit UIA calls in 227 ms.** Availability returned **23
Boolean-positive cells and 467 NotSupported cells, with zero Boolean-false
cells**. All 23 positive cells yielded interfaces. The only positive pattern
outside the previous four-pattern inventory was RangeValue on the loading
progress control. No editor-entry or arbitrary-folder commit capability was
demonstrated; this pass performed no provider actions.

This is [Prototype read-only discovery of standard address-control
capabilities](https://github.com/inaku-Gyan/PathWrap/issues/81). The human
answered “是” to [selecting and automatically executing the bounded
read-only pass](https://github.com/inaku-Gyan/PathWrap/issues/79#issuecomment-6986098487).
The [API research and fixed catalog](https://github.com/inaku-Gyan/PathWrap/blob/68e103e2f8f9d805eb53008334b73ea9c58930fd/docs/research/uia-address-readonly-discovery.md)
define the method, getter whitelist and interpretation limits.

## Findings

The table lists only patterns whose availability was Boolean true **and**
whose interface was obtained in this capture. Other cells returned
NotSupported; their interfaces were not requested.

| Address elements | Positive patterns |
| --- | --- |
| Address Band Root, Breadcrumb Parent and both address toolbars | LegacyIAccessible |
| Loading progress control | RangeValue, LegacyIAccessible |
| All-locations, fixture-root and A breadcrumbs; previous-location and refresh children | Invoke, LegacyIAccessible |
| Hidden ComboBoxEx32 | LegacyIAccessible |
| Hidden address ComboBox and Edit | Value, LegacyIAccessible |
| ComboBox's virtual address Button | Invoke, LegacyIAccessible |

The hidden Edit's UIA Value and Legacy value were both empty; its Value
provider reported IsReadOnly false. Its Legacy default-action, shortcut,
description and help strings were empty. These are state observations, not
evidence of a visible editor or a navigation commit action.

The ComboBox child reported AccessKey and Legacy KeyboardShortcut
`Alt+向下键` (Alt+Down), with the existing Open default-action description.
No key was sent. The observed metadata describe this already measured
dropdown target; no new Go or Edit-address action was identified from the
captured names/descriptions. Empty metadata do not prove that all provider
actions or states are incapable.

All **126 Legacy getter results** (nine for each element) succeeded and
retained empty strings separately from errors. UIA AccessKey returned 12
strings (11 empty and the one Alt+Down string) and two NotSupported values;
AcceleratorKey returned NotSupported on all 14 elements.

Native ComboBoxEx32, ComboBox and Edit were hidden in their before/after
samples, while UIA reported IsOffscreen false and nonempty bounding
rectangles for those providers. Legacy state was retained as raw bits and
hex; the sources of visibility/state information remain distinct. A
reported UIA rectangle or focusable bit is not treated as visible editing.

**NotSupported is not Boolean false.** GetCurrentPropertyValueEx with
ignoreDefaultValue=true plus CheckNotSupported produced a reserved
NotSupported value for 467 availability cells. Under the selected method
only positive cells were bound. These 467 cells therefore do not establish
that the corresponding interfaces are absent or permanently unsupported.
The pass completed all requested queries, while those capabilities remain
unresolved rather than being converted to negatives.

## Exact run and evidence

On Windows in this throwaway checkout:

```powershell
cargo run --bin uia_address_readonly_discovery_prototype
```

- [Exact executed source](../../../src/bin/uia_address_readonly_discovery_prototype.rs).
- [Chronological scrubbed trace](open.jsonl).
- [Matrix counts, positive cells, metadata and native observations](summary.json).
- [Environment and executed source/binary hashes](environment.json).

The harness derives its owned STA fixture and external MTA worker
supervisor from the accepted breadcrumb prototype, preserving that source
unchanged. The modern Open fixture initializes A, filename
`pending-prototype-name.csv` and CSV type index 2. The owned native
IFileDialog is used for setup and independent read-only truth; it is not an
interface acquired by the external client or a production backend.

Read-only native child discovery finds the unique Address Band Root within
the original dialog HWND/PID. Raw UIA walking is limited to that address
band and the ComboBoxEx32, ComboBox and Edit subtrees obtained from its
known native child HWNDs. Each seed validates its native handle/process;
node identities are compared to avoid duplicate capture. Four source roots
completed, yielding 14 unique elements and 14 source edges in this case.
No other application's UIA tree or names were captured. The file-view tree
was not part of this scoped pass.

The catalog contains the exact 35 defined pattern/availability-property
pairs from windows 0.62.2, including SelectionPattern2. Each pair was
statically checked against the installed official binding before the GUI
case. This is a fixed standard catalog, not arbitrary ID scanning or
custom/future-pattern discovery.

Per-cell availability is GetCurrentPropertyValueEx(id, true) followed by
CheckNotSupported and exact VARIANT type handling. Only Boolean-positive
cells call GetCurrentPattern; getter-bearing interfaces then receive
QueryInterface. The getter whitelist is nine Legacy properties, UIA
AccessKey/AcceleratorKey, existing Value/IsReadOnly and ExpandCollapse state
where positively reported, plus element identity/focus/visibility fields.
There is no traversal of arbitrary pattern methods.

## Budget, completeness and result classification

The fixed limits are 64 unique elements, depth 16, 4096 explicit UIA calls
and a soft ten-second read budget, with a separate fifteen-second worker
supervisor. Tree walking, identity comparison, availability checks,
interface binding and getters are counted. Internal RPCs and interface
reference releases are not separately counted.

The measured pass used **1515 calls** and **227 ms**; all 1515 call starts
have matching ends. All 490 availability cells were read, all four source
captures finished with no pending known nodes, no bound was reached and
the completion record was true. The worker exited after **252 ms**, without
a timeout. No additional case or automatic rerun occurred.

The 14 Rust Err results all had code 0 and were RawViewWalker child/sibling
end markers, separately recorded as tree-end binding-null observations.
There were no nonzero error codes or null/unknown bindings among the 23
positive pattern cells. Getters and properties keep their own status;
unrequested, incomplete, NotSupported, unexpected-type and error outcomes
are not collapsed into false.

High-level windows-rs bindings do not preserve raw successful HRESULTs.
The raw trace records Rust result/error codes and explicitly leaves raw
HRESULT null. It does not fabricate S_OK or interpret Err(0) as a failed
provider HRESULT. This is the research's selected high-level observation
method; no raw-vtable instrumentation was added.

Reads are sequential, not an atomic snapshot. A synchronous query could
overrun the soft time budget; the separate supervisor would bound the
worker and retain the active read-start record. Completion describes this
catalog/getter pass on the provider-returned initial-state tree, not every
possible UI state or provider capability.

## Independent truth and environment

Two fresh native polls fell inside the short read pass, both reporting A,
the initialized pending filename, type index 2 and the original live,
visible dialog. Three further fresh polls after worker exit confirmed that
state before teardown. No folder jump was attempted. Preservation during
a read-only initial-state case does not validate successful-jump
preservation or external actual-arrival verification.

Windows **26300.9974**, AMD64, zh-CN, Rust **1.100.0**. The processes inherited
the user token; integrity levels were not independently measured. Before
and after the pass, the foreground belonged to another process with native
class `Chrome_WidgetWin_1`; its application identity/title/tree were not
queried. Foreground was not forced, and this is not a controlled-dialog-
foreground case.

Immediate pre/post wrapper checks found no Listary desktop application or
interactive helpers. The running automatic ListaryServiceV2 service and
Listary.Service.exe PID 6176 remained in Session 0. No service or unrelated
application was stopped. This retains the desktop-app-exited boundary,
without complete component isolation or a causality claim. The standalone
binary does not enforce Listary absence; the wrapper recorded the facts in
environment.json. All owned prototype processes exited.

## Validation and disposition

just check --ci, just build and just test passed: **47 existing tests
passed**, five interactive/diagnostic tests remained ignored. All 35
catalog pairs were statically cross-checked; the source has no UIA
Invoke/SetFocus/SetValue/Select/DoDefaultAction/Expand/Collapse call site.
No new prototype unit tests were added, and just e2e was not invoked.
The temporary Justfile EOL-only normalization used for its required check
is restored before publication.

Creating/initializing and closing the owned fixture are setup/teardown.
The measured UIA worker performed zero provider actions, with no manual
editor setup, input simulation, clipboard navigation or alternate backend.
The executed source was not changed after the GUI case. Runtime scrubbing
normalizes fixture paths and redacts non-fixture absolute/profile paths.

Publish this exact evidence on the throwaway branch with no product
changes, PR or merge into main. The result awaits human review. The full
automatic entry/edit/commit chain remains unresolved; no further action or
extra interface probing of NotSupported cells is selected by this result.
