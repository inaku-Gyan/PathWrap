# Address-bar UIA navigation — throwaway prototype

Question: can an external UIA client enter a selected folder in the existing
Open/Save dialog's address bar and actually commit navigation without simulated
input, while leaving the pending file operation intact?

This artifact supports
[Prototype address-bar folder navigation through UI Automation](https://github.com/inaku-Gyan/PathWrap/issues/71)
in [Address-bar navigation without simulated input](https://github.com/inaku-Gyan/PathWrap/issues/70).
It measures the user's chosen B strategy. It changes no production code.
The prototype skill's HTML state demo cannot establish a Windows provider's
capability, so the artifact is a runnable Rust API probe with raw observations.

## Finding

**UIA text assignment worked on the hidden address Edit, but no folder jump
was demonstrated.** The Edit read back B while the independent native observer
continued to report A. No recognized callable Go action appeared in the captured
address subtree, so no navigation commit action was invoked.

| Case | SetValue HRESULT | Provider's own value readback | Actual folder afterward | Go candidates / calls |
| --- | --- | --- | --- | --- |
| Open, Edit provider | S_OK | B | A | 0 / 0 |
| Save, Edit provider | S_OK | B | A | 0 / 0 |
| Open, ComboBox provider | S_OK | empty | A | 0 / 0 |
| Save, ComboBox provider | S_OK | empty | A | 0 / 0 |

All four dialogs remained open. The pending filename
`pending-prototype-name.csv` and CSV filter index 2 stayed unchanged through
these unsuccessful attempts. **That is not successful-navigation preservation
evidence:** no sample reached B. For the ComboBox cases, an empty readback is
what that provider returned; this does not separately establish what happened
to every child field.

The concrete gap is a demonstrated provider action that activates/commits
address navigation. These results do not prove that every UIA path, provider,
dialog lifecycle state or Windows build lacks such an action. B is not an
implementation-ready navigation strategy on this evidence.

## Run

On this throwaway branch, on Windows:

```powershell
cargo run --bin uia_address_bar_prototype -- open
cargo run --bin uia_address_bar_prototype -- save
```

Those commands select the Edit provider. The explicit compound-control
comparison is:

```powershell
cargo run --bin uia_address_bar_prototype -- open combo
cargo run --bin uia_address_bar_prototype -- save combo
```

[Source](../../../src/bin/uia_address_bar_prototype.rs).
Each command owns a separate STA fixture process and scratch directory named
`PathWarp-UIA-PROTOTYPE-*`. A and B contain distinct marker subfolders. The
external client uses MTA COM. Fixture setup initializes A, a nonempty filename
and the non-default filter; its native interface is used only for setup and
independent observation, never as an externally discovered navigation backend.
The command closes its own fixture afterward; scratch folders remain inspectable.

## What was observed

1. **A complete baseline.** A timer on the host's dialog thread reads its owned
   `IFileDialog::GetFolder/GetFileName/GetFileTypeIndex`. The client waits for
   visible A, the expected nonempty filename and filter 2 before acting. The
   earlier empty construction callback is not used as the baseline. Timestamped
   polls after each action stage exclude observations from before that stage.
2. **Initial UIA tree.** Open had 107 raw elements and Save 108. The visible
   address band contained breadcrumbs and toolbars, with no writable address
   Value field in that initial tree. All captured element PIDs matched the
   corresponding separate fixture host PID.
3. **Semantic focus.** `SetFocus` on the address-band element returned S_OK and
   the observed focus was its breadcrumb toolbar. The main tree still did not
   expose the editor. A later `SetFocus` on the exact Edit or ComboBox element
   also returned S_OK; that HRESULT is not treated as proof of visible edit mode.
4. **Hidden HWND discovery.** Read-only `EnumChildWindows` inside the identified
   address band found seven HWNDs, including hidden `ComboBoxEx32`, `ComboBox`
   and `Edit` windows. `ElementFromHandle` exposed writable Value patterns on
   the latter two, even though they were omitted from the main initial tree.
   The same elements reported `IsOffscreen=false` despite native
   `IsWindowVisible=false`; both properties are retained in the evidence.
   Matching was scoped to this address band's actual descendants, never an
   arbitrary filename/search Edit. Observed class names and IDs are fixture
   evidence, not universal selectors.
5. **Value assignment and negative control.** Each run selected exactly one
   provider, called SetValue once, read its value back, and observed the host
   for a second without invoking a navigation action. The Edit echoed B, but
   the native folder and the visible file-view marker stayed at A. The
   ComboBox returned S_OK with its own value still empty. The two providers are
   explicit comparison cases, not a runtime fallback chain.
6. **Commit discovery.** Fresh main-dialog trees after the setter and at the
   end still showed the A breadcrumbs, "all locations", "previous location"
   and refresh-A actions. No recognized Go action appeared in that address
   subtree. The probe only accepts an enabled, on-screen Invoke action with
   observed Go wording (`转到`, `Go to`, or `Go`); this is a bounded fixture
   recognition rule. It does not invoke random breadcrumbs, refresh, filename
   controls or final Open/Save confirmation. The compiled conditional Go-Invoke
   branch was not exercised.
7. **Independent final state.** Another two seconds of fresh native polling
   continued to report A, with the same filename/type and live dialog. The
   traces' `after_commit_stage` label means the observation point after commit
   discovery; **`commit_calls=0`** in every case. It does not mean a commit ran.
   Arrival, navigation failure handling and success-focus handoff remain untested.

There is also an earlier, Open-only semantic context-menu experiment:
[`context-desktop-open.jsonl`](context-desktop-open.jsonl), captured with
[its exact source at the intermediate commit](https://github.com/inaku-Gyan/PathWrap/blob/4c0465eb8baca26a9d686bcbe372a32e59a2bd4f/src/bin/uia_address_bar_prototype.rs).
`IUIAutomationElement3::ShowContextMenu` on the address toolbar returned S_OK
(55 ms), but the three-second search found no additional UIA desktop root
belonging to the fixture, and the after-action dialog tree exposed no Edit
address command. That is an observation limit for this target and lookup, not
proof that all context-menu mechanisms fail. The final source uses the focus
and hidden-provider path; it does not retain context-menu access as a fallback.

## Evidence and verification

- Edit-provider [Open](open.jsonl) and [Save](save.jsonl) traces.
- ComboBox-provider [Open](combo-open.jsonl) and [Save](combo-save.jsonl) traces.
- [Environment](environment.json): Windows 11 Home Chinese, `10.0.26300`,
  build 26300, x64, `zh-CN`; Rust 1.100.0. The fixture inherits the client token;
  no elevation was requested and integrity levels were not separately queried.
- The two Edit traces were captured before adding the optional `combo` argument;
  the final source preserves the same default Edit sequence and adds the
  explicit compound-provider comparison.
- Coverage is initial-state controlled modern Common Item Open/Save dialogs on
  this build. Other apps, architectures, custom/legacy/brokered/elevated dialogs
  and later lifecycle states are unmeasured. No nonexistent-folder navigation
  case was reached because there was no demonstrated commit action.
- `just check --ci`, `just build`, and `just test` passed: 47 existing tests
  passed; five interactive E2E/diagnostic tests ignored. No prototype tests were
  added. The announced GUI checks are capability measurements; `just e2e` was
  not invoked. No prototype fixture processes remained afterward.
- No keyboard/mouse simulation, clipboard navigation, filename injection,
  final confirmation action, injected helper or alternate navigation backend
  was used. PathWarp's production navigation and Ctrl+G were not implemented
  or evaluated in this experiment.
- Names/values outside the address scope are generally omitted. Scratch paths
  and user-profile prefixes are normalized. The raw traces preserve tree
  relationships, classes, IDs, state, pattern results and action HRESULTs.

Absent `GetCurrentPatternAs` lookups can appear as Rust errors carrying
`HRESULT(0x00000000)`: the binding received S_OK with a null interface. The
`available=false` field is the relevant absence indicator; those records are
not failing Windows HRESULTs. Successful action HRESULTs are separately logged
and were never equated with actual folder arrival.

## Primary contracts

Microsoft's [Value pattern](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvalue)
provides value assignment, not a generic address-navigation commit.
[Invoke](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementinginvoke)
executes the element's associated action; a successful API result alone does
not supply independent folder identity.
[`SetFocus`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-setfocus)
sets element focus, without promising an address edit-mode transition.
[`ShowContextMenu`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement3-showcontextmenu)
is the semantic context-menu API, and may act on an ancestor.
[`SetTimer`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer)
supplies the owned host's read-only observation callback on its message loop.
The runtime traces above, rather than those general contracts, establish this
prototype's bounded results.
