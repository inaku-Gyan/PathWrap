# THROWAWAY: filename-field UIA folder jumps

The corrected owned **Open and Save cases both completed A → B and restored
`pending-prototype-name.csv`, with CSV filter index 2 unchanged and the original
dialog still live and visible**. Neither case emitted OnFileOk. These are two
controlled successes, awaiting human acceptance of
[the full-chain prototype](https://github.com/inaku-Gyan/PathWrap/issues/89).

| Case | Independent current folder | Pending operation after restoration | Dialog | OnFileOk callbacks |
| --- | --- | --- | --- | --- |
| Open | A → B | original filename, CSV index 2 | same live visible HWND | 0 |
| Save | A → B | original filename, CSV index 2 | same live visible HWND | 0 |

Each case used exactly two filename Value writes (directory and original text)
and one named IDOK Invoke. The external worker observed the fresh address-toolbar
Name change to B and used that observation to gate restoration. The toolbar still
reported B after the original filename had been restored. Independent retained
IFileDialog snapshots agreed with B and preservation; they were never inputs to
the worker. Six fresh native final polls in each case retained B, the original
filename/type and original live/visible HWND. The type did not drift, so no type
restoration action ran.

## Source and evidence

- [Exact corrected executed source](https://github.com/inaku-Gyan/PathWrap/blob/a918d46c5c156642e95c6dd035c479fa3594a835/src/bin/uia_filename_folder_jump_prototype.rs)
- [Corrected Open trace](corrected/open.jsonl), [first Save trace](corrected/save.jsonl)
- [Corrected environment and authorization](corrected/environment.json)
- [Machine-readable summary and transitions](summary.json)
- [Original Open preflight trace](open.jsonl), [original environment](environment.json)

The original source was
[50048d1216817ffdcb2cbea6276a21aa874e33b9](https://github.com/inaku-Gyan/PathWrap/blob/50048d1216817ffdcb2cbea6276a21aa874e33b9/src/bin/uia_filename_folder_jump_prototype.rs).
That first Open case stopped before all provider actions after 119 read-only
calls: my locator incorrectly required Address Band Root to be the toolbar's
grandparent. Existing evidence already showed an intervening msctls_progress32.
It was a prototype implementation defect, not a test of filename navigation.
Its raw trace and environment are unchanged.

The correction validates at most 16 ancestors through the owned address band to
the original dialog and records the chain. The human explicitly said
“你可以重试”; [authorization and correction](https://github.com/inaku-Gyan/PathWrap/issues/89#issuecomment-6991066943)
preceded the corrected Open retry. Save was its first case. Exactly three cases
ran in total; both corrected cases used the same checked source and executable.

## Actions and observations

| Case | Set B, ms | Invoke, ms | Restore filename, ms | B gate, ms | worker_start → worker_result, ms |
| --- | --- | --- | --- | --- | --- |
| Open | 34.620 | 16.648 | 0.679 | 151 | 1035 |
| Save | 13.007 | 16.982 | 30.060 | 70 | 1112 |

Open logged 383 explicit calls, Save 422;
all starts/ends matched, with zero query errors, bounds or timeouts. Raw successful
HRESULTs are null because the high-level binding does not preserve them. The
supervisor's raw worker_lifetime_ms additionally includes final native sampling;
the summary labels that interval accordingly. It is not the worker-only duration.

## Limits

Windows 26300.9974 / 26H2 / AMD64 / zh-CN / Rust 1.100.0. Each corrected case
began with another process foreground and ended with its owned dialog foreground.
No explicit foreground-forcing API ran; causality and product focus behavior were
not isolated. Listary desktop/helpers were absent in overall before/after samples,
while ListaryServiceV2 / Listary.Service PID 6176 remained in Session 0. The process
baseline timestamp is retained separately; it was not resampled before every case.
Integrity levels were not independently measured. All owned processes exited,
scratch directories were removed non-recursively and stderr was empty.

This covers one local existing B directory per dialog, one trailing-separator
path spelling, one nonempty filename and CSV index 2. Empty/multiple filenames,
other paths/hosts, restoration errors, stale sessions, user switching and broad
compatibility remain unevaluated. The full-path UIA Name agreed with native truth
here; [the external-arrival decision](https://github.com/inaku-Gyan/PathWrap/issues/90)
and [minimal contract](https://github.com/inaku-Gyan/PathWrap/issues/91) remain open.

## Run and bounds

```powershell
cargo run --bin uia_filename_folder_jump_prototype -- --case open
cargo run --bin uia_filename_folder_jump_prototype -- --case save
```

Each command runs a new owned case. There is no automatic action retry. The UIA
worker uses unique named controls and fresh identity/ownership/visibility guards.
The host retains native APIs only for fixture setup, independent truth and teardown.
The OnFileOk veto is counted so vetoed acceptance cannot masquerade as navigation.

Bounds: 64 elements per typed lookup, 16 ancestors, 1024 explicit calls,
12-second soft worker budget, 15-second supervisor, three-second soft B discovery,
100 ms native polling and fresh final native samples. Synchronous calls can exceed
soft deadlines. No synthetic input, clipboard, fallback, foreground forcing,
service shutdown, native navigation/restoration helper or product change.

Both source versions passed just check --ci, just build and just test: 51 passed,
five interactive/diagnostic tests ignored. Two new guard tests check exact command
captions and full-path equality. The Justfile was restored byte-for-byte. No just
e2e, product implementation, PR or merge. Hashes matched before both corrected
launches and after execution; committed source matches executed source. No Rust
change was made after the corrected runs.
