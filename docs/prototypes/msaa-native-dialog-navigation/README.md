# MSAA native-dialog access — throwaway prototype

Question: can an external process retrieve the existing standard Open/Save
dialog's `IFileDialog` through `AccessibleObjectFromWindow` with
`OBJID_NATIVEOM`, then navigate through its native folder API?

This is a real Windows API capability probe for
[Prototype MSAA access to the existing native file dialog](https://github.com/inaku-Gyan/PathWrap/issues/68),
under [Native folder navigation and Ctrl+G focus](https://github.com/inaku-Gyan/PathWrap/issues/61).
It reuses the controlled fixture from the
[preceding ObjectModel prototype](https://github.com/inaku-Gyan/PathWrap/blob/9a2f1593ecb116c4f1c4f0c7ffbdb5dc841983eb/docs/prototypes/native-dialog-navigation/README.md).
The prototype skill's HTML state-model demo cannot answer a provider capability
question; the runnable Rust source and visible API traces are the artifact here.
There is no production navigation or Ctrl+G change.

## Finding

**This MSAA access route did not produce `IFileDialog` in either measured
dialog.** The native folder-navigation stage was not reached.

| Observation | Open | Save |
| --- | ---: | ---: |
| Dialog and descendant HWNDs examined | 48 | 50 |
| `OBJID_NATIVEOM` requests, three IIDs per HWND | 144 | 150 |
| `S_OK` / non-null interface returns | 2 | 2 |
| `E_NOINTERFACE` / null returns | 1 | 1 |
| `E_FAIL` / null returns | 141 | 147 |
| Retrieved `IFileDialog` interfaces | 0 | 0 |
| External live-dialog `SetFolder` calls | 0 | 0 |
| Ordinary root `OBJID_CLIENT/IAccessible` diagnostic succeeded | yes | yes |
| Dialog remained open through the probe | yes | yes |

In both runs the only non-null native-object requests were `IUnknown` and
`IDispatch` on a hidden `ListBox` child, control ID 1120, window index 28.
Requesting `IFileDialog` directly on that same HWND returned `E_NOINTERFACE`
(`0x80004002`). Querying both returned interfaces for `IFileDialog` also
returned `E_NOINTERFACE`. Those two additional QueryInterface results are
logged separately and are not included in the request totals above.

The two interface returns are not two proven distinct native model objects,
and do not establish a usable dialog object. This experiment did not classify
the ListBox object's model or test its other interfaces/services. Root dialog,
address-band, breadcrumb, Shell view and other enumerated child HWNDs returned
`E_FAIL` (`0x80004005`) with null for all three requested IIDs. Receiving an
ordinary accessibility object in the diagnostic is not native navigation proof.

This is a bounded negative result for this access protocol, these IIDs, these
fixture states and this Windows build. It does not prove all native access
impossible, or resolve a service-provider, cooperating-host or helper design.

## Run

On the throwaway branch, on Windows:

```powershell
cargo run --bin msaa_native_dialog_prototype -- open
cargo run --bin msaa_native_dialog_prototype -- save
```

[Source](../../../src/bin/msaa_native_dialog_prototype.rs).
Each command creates its own temporary `PathWarp-MSAA-PROTOTYPE-*` directory
with folders A and B, opens its own fixture dialog, prints JSON lines, and closes
only that fixture after probing. Scratch folders remain for inspection.

## What the source actually does

1. Spawn a separate STA host process which creates a standard modern
   `FileOpenDialog` or `FileSaveDialog`. Configure filesystem folders, pending
   filename `pending-prototype-name.csv`, CSV filter index 2, and initial folder A.
   Its `IFileDialogEvents` observer independently reports folder snapshots.
   `OnFileOk` vetoes accepting an Open/Save operation.
2. Wait for the host's HWND and visible dialog. The external client uses MTA COM.
   The host supplies identity and observations, never a native interface/proxy.
3. Issue a separate diagnostic `OBJID_CLIENT` (-4) request for `IAccessible` on
   the root. Never use that object to drive navigation.
4. Enumerate the root plus all descendant HWNDs, including hidden windows.
   Record HWND, class, process/thread, control ID and visibility. Verify each
   window still belongs to the fixture process. Inventory is bounded at 512.
5. For each HWND request `IUnknown`, `IDispatch` and `IFileDialog` using
   `OBJID_NATIVEOM` (-16, DWORD `0xFFFFFFF0`). Record the exact IID, HRESULT and
   output-pointer nullness. A small raw ABI declaration preserves HRESULTs that
   the generated `Result<()>` wrapper would otherwise collapse on success.
6. Query any returned `IUnknown`/`IDispatch` for `IFileDialog`. If obtained,
   require its `IOleWindow` HWND to equal the original root and its `GetFolder`
   to read A before attempting a single `SetFolder(B)`.
7. The inherited, compiled navigation stage would compare native before/after
   snapshots, poll folder readback, check a matching independent host event and
   dialog lifetime. **That stage did not execute in either recorded run.**
   It uses fixture path strings, not a general Shell identity contract.
8. Close the owned fixture over its stdin command channel. Cancellation
   `0x800704C7` in `host_closed` is expected fixture cleanup, not a discovery
   failure. No synthetic keyboard/mouse, clipboard navigation, ObjectModel
   access, address-bar driving, injected helper or alternate backend is used.

The trace's `set_folder_calls` counts external navigation requests, excluding
the host's initial `SetFolder(A)` before `Show`. Its `native_interface_returns`
counts non-null responses to native-object requests, not `IFileDialog` objects.
The initial host folder callback occurs during dialog construction and reports
an empty filename. It is not a fully initialized filename baseline and proves
nothing about preservation. No filename/type or focus-handoff result is claimed.

## Evidence and coverage

- [Open trace](open.jsonl) and [Save trace](save.jsonl).
- [Captured environment](environment.json): Windows 11 Home Chinese,
  `10.0.26300`, build 26300, x64, `zh-CN`; Rust 1.100.0.
- One initial-state standard modern Open and one Save fixture, separate client
  and host PIDs. The host inherits the client token; no elevation was requested.
  Integrity levels were not independently queried.
- No claim about other apps, custom/legacy/brokered/elevated dialogs, other
  Windows versions/architectures, later dialog lifecycle states or all providers.
  The GUI capability checks are measurements, not production E2E coverage.
- Required checks passed: `just check --ci`, `just build`, `just test`.
  Existing tests: 47 passed; five interactive E2E/diagnostic tests ignored.
  No prototype tests were added and `just e2e` was not invoked.
- No prototype fixture processes remained after the two checks.

## Primary API contracts

Microsoft's [`AccessibleObjectFromWindow`](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-accessibleobjectfromwindow)
accepts the requested interface IID; unsupported interfaces can return
`E_NOINTERFACE`. Its
[native-object protocol](https://learn.microsoft.com/en-us/windows/win32/winauto/exposing-a-native-object-model-interface)
depends on a provider exposing an object through `WM_GETOBJECT/OBJID_NATIVEOM`.
The [provider guidance](https://learn.microsoft.com/en-us/windows/win32/winauto/using-objid-nativeom-to-expose-a-native-object-model-interface-for-a-window)
also calls out cross-process COM interface registration. These contracts do not
promise a file-dialog object on every window. The source and runtime traces
above supply the evidence for the limited verdict here.
