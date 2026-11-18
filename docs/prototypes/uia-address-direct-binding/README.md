# Initial-A direct UIA pattern acquisition — throwaway prototype

**All 490 element/pattern pairs were directly queried once. No additional
interface was obtained.** The 14-element initial-A capture returned 23
interfaces and 467 high-level `binding_null_unknown` outcomes. All 23
obtained interfaces had Boolean-true companion availability; all 467
reserved NotSupported availability results paired with binding-null/unknown.
This completed the previously skipped acquisition attempts without proving
visible editing, navigation commit or permanent lack of support.

This is [Prototype direct acquisition of standard address-control UIA
patterns](https://github.com/inaku-Gyan/PathWrap/issues/83). The human answered
“是” to [selecting and automatically executing the single bounded
protocol](https://github.com/inaku-Gyan/PathWrap/issues/82#issuecomment-6987403950).
The [accepted initial-state inventory](https://github.com/inaku-Gyan/PathWrap/issues/81#issuecomment-6987145050)
and [completed API research](https://github.com/inaku-Gyan/PathWrap/blob/68e103e2f8f9d805eb53008334b73ea9c58930fd/docs/research/uia-address-readonly-discovery.md)
provide the comparison and result-classification rules.

## Findings

| Companion availability | Direct acquisition | Cells |
| --- | --- | ---: |
| Boolean true | Interface obtained | 23 |
| Reserved NotSupported | High-level binding-null/unknown | 467 |
| Boolean false | Any outcome | 0 |
| Any outcome | Unqueried acquisition | 0 |

The fixed matrix contained **14 unique elements × 35 standard patterns**.
All 490 `GetCurrentPattern` calls were made once, without an availability
gate or a second binding pass. No interface was obtained when companion
availability was anything other than Boolean true in this sample.

The obtained pattern profile matches the earlier inventory's aggregate
profile. The fresh elements and HWNDs were bound independently; earlier
live identities were not reused.

| Captured address elements | Obtained patterns |
| --- | --- |
| Address Band Root, Breadcrumb Parent and both address toolbars | LegacyIAccessible |
| Loading progress control | RangeValue, LegacyIAccessible |
| All-locations, fixture-root and A breadcrumbs; previous-location and refresh children | Invoke, LegacyIAccessible |
| Hidden ComboBoxEx32 | LegacyIAccessible |
| Hidden ComboBox and Edit | Value, LegacyIAccessible |
| ComboBox's virtual address Button | Invoke, LegacyIAccessible |

The 23 obtained cells comprise 14 Legacy, six Invoke, two Value and one
RangeValue interfaces. All 126 Legacy getter results succeeded, preserving
empty strings. The hidden Edit's UIA and Legacy values were empty, its Value
provider reported IsReadOnly false, and its Legacy default action, shortcut,
description and help were empty. No new editor-entry or commit method was
established by reading this metadata.

The 467 acquisition outcomes were Rust `Err` with error code 0, classified
as `binding_null_unknown`. The high-level binding does not retain the raw
successful HRESULT and pointer separately. **These are not exact observed
`S_OK/NULL` pairs, failed provider HRESULTs or proof of permanent absence.**
The result says no additional interface was acquired in this one initial
state using this method. Other states, custom/future patterns and action
semantics were not tested.

## Exact execution and evidence

The debug executable built by `just build` was invoked once without
arguments. A launcher captured UTF-8 stdout/stderr and immediate environment
snapshots without creating an extra visible console. To run the same case
from this throwaway checkout:

```powershell
cargo run --bin uia_address_direct_binding_prototype
```

- [Exact executed Rust source](../../../src/bin/uia_address_direct_binding_prototype.rs).
- [Chronological scrubbed trace](open.jsonl).
- [Typed matrix, classifications, metadata and native observations](summary.json).
- [Environment and source/binary hashes before and after execution](environment.json).

The accepted read-only source remains unchanged beside the new prototype.
The new source replaces its positive-only gate with one acquisition call per
catalog entry, records that policy, and checks the original dialog's native
HWND/PID before starting each explicit UIA call. Native identity guards are
read-only and are not counted as UIA calls.

The owned STA host initializes a modern Open dialog to A, filename
`pending-prototype-name.csv` and CSV type index 2. Its retained IFileDialog is
setup and independent truth only, not an interface acquired by the external
MTA client or a production navigation backend.

The client binds the original HWND/PID, unique Address Band Root Raw subtree
and validated address-band-scoped ComboBoxEx32, ComboBox and Edit HWND
subtrees. Four source captures completed, with 14 source edges and 14 unique
elements. UIA identity comparison prevents duplicate capture. No unrelated
application names/trees or file-view UIA tree were queried.

The fixed 35-pair catalog is unchanged and was statically checked against
the installed windows 0.62.2 binding. Typed companion availability remains
`GetCurrentPropertyValueEx(id, true)` plus `CheckNotSupported`, with errors,
reserved values and Boolean false kept distinct. Binding is now independent
of that observation. Only the existing Legacy, Value/IsReadOnly and
ExpandCollapse getter whitelist is used after acquisition, plus the existing
element/access-key metadata. Other pattern methods are not enumerated.

## Completeness and budget

The pass completed **1982 explicit UIA calls in 258 ms**. All 1982 starts and
ends match, including all 490 unique direct acquisition labels. No requested
matrix item remained unqueried, all four roots completed, and no bound was
reached. The worker exited after **302 ms**, without timeout. There was one
GUI pass and no automatic rerun.

The predeclared bounds remain 64 unique elements, depth 16, 4096 explicit
UIA query/walk/comparison/binding/getter calls, a ten-second soft budget and
a separate fifteen-second worker supervisor. Native guards, internal RPCs
and reference releases are not separately counted. A synchronous call can
overrun the soft budget; the supervisor retains the active start record if
it must terminate its own worker. The element cap does not promise that
every possible tree would fit the call budget.

The trace contains 481 high-level Rust error results: 467 pattern
binding-null/unknown results and 14 RawViewWalker end sentinels. **No nonzero
error code was observed.** Tree-end sentinels are recorded separately; raw
successful HRESULTs remain null instead of being fabricated. Availability,
binding and getter reads are sequential, not an atomic snapshot.

## Independent truth and environment

Two fresh native host polls during the read pass and three fresh polls after
worker exit retained A, the initialized filename, type index 2 and the same
live visible dialog. No post-baseline poll showed drift. Initialization
folder callbacks are retained separately from the preservation verdict. No
folder jump was attempted, so this is read-only preservation evidence only.

Native ComboBoxEx32, ComboBox and Edit were hidden before their source
captures and after the pass. Their UIA IsOffscreen values were false and
rectangles nonempty; these provider fields are retained separately from
native visibility and do not establish a visible editor.

Windows **26300.9974 / 26H2**, AMD64, zh-CN, Rust **1.100.0**. Pre/post
snapshots found no Listary desktop application or interactive helpers; the
automatic ListaryServiceV2 service and Listary.Service.exe PID 6176 remained
running in Session 0, with no main window. No service was stopped, and this
does not claim complete component isolation or causality.

Another process, PID 5800 with native class `XhtmlFrameView`, was foreground
before and after the pass. Its application identity, title and tree were not
queried. Foreground was not forced, so this is not a controlled-dialog-
foreground case. Processes inherited the invoking user token; integrity
levels were not independently measured. All owned prototype processes
exited, and stderr was empty.

## Validation and disposition

`just check --ci`, `just build` and `just test` passed: **47 existing tests
passed**, five interactive/diagnostic tests ignored. The fixed 35 catalog
pairs were checked against the pinned binding. A source scan found no UIA
Invoke/SetFocus/SetValue/Select/DoDefaultAction/Expand/Collapse call site.
The temporary Justfile newline normalization was restored byte for byte.
No new prototype unit tests were added, and `just e2e` was not invoked.

The executed source and binary hashes matched before/after the run. The
source remained unchanged during evidence analysis. Raw/summary/environment
privacy scans found no profile-path or username matches; fixture paths are
normalized to PROTOTYPE_ROOT and non-fixture absolute paths are redacted.

The UIA worker executed zero provider actions, with no manual editor setup,
simulated input, clipboard navigation, alternate backend, service shutdown
or foreground forcing. Creating/initializing and closing only the owned
fixture are setup/teardown. No product changes, PR or merge into main.

This evidence awaits human review. It does not select a further experiment
or make the minimal UIA navigation contract implementation-ready. Automatic
visible-editor entry, arbitrary-folder commit, successful-jump preservation
and production-available arrival verification remain unresolved.
