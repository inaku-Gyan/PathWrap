# THROWAWAY: external native file-dialog discovery

Question: can an external process obtain the existing Open/Save dialog's
`IFileDialog` through UIA ObjectModel, then navigate it with `SetFolder`?

This is a real Windows API capability prototype, not a simulated state model.
It is an asset for [Prototype direct native file-dialog navigation](https://github.com/inaku-Gyan/PathWrap/issues/66),
not production integration.

## Measured result

The chosen discovery path is unavailable in both tested initial dialog states.
Every sampled raw UIA element reported ObjectModel unavailable, and direct
pattern retrieval returned no interface. No native dialog interface was obtained.

| Controlled dialog | Raw elements inspected | ObjectModel available | External live-dialog SetFolder calls | Dialog still open after scan |
| --- | ---: | ---: | ---: | --- |
| Open | 107 | 0 | 0 | yes |
| Save | 108 | 0 | 0 | yes |

The probe stopped at that gate. Arrival at another folder, filename/type
preservation during navigation, and focus return were **not exercised**.
This is evidence for this access path and these samples; other native access
mechanisms, hosts, dialog states and Windows versions have not been ruled out.

## Reproduce

Check out this prototype branch and run one command per sample:

```powershell
cargo run --bin native_dialog_navigation_prototype -- open
cargo run --bin native_dialog_navigation_prototype -- save
```

Each command creates two clearly named scratch folders under the system temp
directory, launches its own visible file dialog in a separate hidden-console
process, prints JSON observations, and closes that fixture. It does not type,
paste, click a button, or drive an existing user application's dialog.

[Source](../../../src/bin/native_dialog_navigation_prototype.rs).
The fixture's internal `--host` mode is invoked automatically.

## Evidence and coverage

- [Open trace](open.jsonl)
- [Save trace](save.jsonl)
- [Environment](environment.json)

Measured on Windows 11 Home Chinese, version 10.0.26300, x64, process UI culture
zh-CN, with Rust 1.100.0. The client is MTA and its fixture is STA. They are
different processes; the fixture inherits the client's token without an elevation
request. Integrity levels were not independently queried.

The fixture creates ordinary `FileOpenDialog` / `FileSaveDialog` COM objects.
The client scans their UIA raw subtree, including the dialog root, address band,
file view, filename controls and navigation tree. It reads property 30112 and
also attempts pattern 10022 on each element. Its scan has a 2,000-element bound;
neither sample reached that bound.

The JSON field `pattern_failures` counts the Rust binding's failed interface
lookups. Here `HRESULT(0x00000000)` represents the **S_OK plus null interface**
case, not an HRESULT failure from Windows. The independent availability property
was false on all of the same elements. Empty child/sibling interfaces are normal
tree boundaries and do not truncate the scan.

The fixture initially requests `pending-prototype-name.csv` and file type 2.
Its first `OnFolderChange` snapshot is emitted during initial construction, when
the filename readback is still empty. That snapshot must not be interpreted as a
filename-preservation test or a fully initialized text-field baseline. Discovery
failed before the navigation checks could run.

`set_folder_calls` counts external navigation attempts while the dialog is open.
It excludes the fixture's initial setup of folder A before `Show`. The final
`host_closed` cancellation is the probe closing its own fixture after measurement.

## Implementation boundary

One access path is implemented:

1. UIA ObjectModel availability and pattern retrieval.
2. Underlying native object, then `QueryInterface(IFileDialog)`.
3. Original-dialog identity and current-folder readback.
4. One `SetFolder` call, followed by folder readback and independent host evidence.

Only the first gate was reached in the measured samples. Later code paths are
compiled but unvalidated. The controlled host does not hand its COM interface to
the external probe. Its callbacks provide observation only.

No alternate discovery/backend, address-bar navigation, injected helper or
synthetic input is attempted after failure. The prototype exposes the first
concrete blocker for human review before another strategy is selected.

Windows 10, legacy/custom dialogs, other applications, elevated targets,
alternative UI states and genuine native navigation remain untested.

## Validation

- `just check --ci`: passed.
- `just build`: passed.
- `just test`: 47 passed, 5 interactive tests ignored.
- Controlled Open and Save GUI scans: completed as recorded above.

The existing `just e2e` suite was not run. No new unit tests were added around the
throwaway probe. On this Windows checkout, `just --fmt` was needed once to
normalize the existing Justfile's checkout line endings before its format check;
the prototype commit does not change that file.

## API references

Microsoft documents [ObjectModel's native-object retrieval](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationobjectmodelpattern-getunderlyingobjectmodel)
and [cross-process object-model access](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingobjectmodel).
These contracts do not establish that a particular file-dialog provider exposes
the pattern; the traces above supply that runtime observation.

[`SetFolder`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-setfolder)
can navigate a displayed dialog when its interface is available. The prototype
did not obtain that interface, so it does not test this method's live behaviour.
