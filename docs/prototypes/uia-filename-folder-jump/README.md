# THROWAWAY: filename-field UIA folder jumps

Question: can one filename Value setter, one named Open/Save button Invoke and
one filename restoration move the same dialog from A to B while preserving its
pending filename/type?

This branch is evidence for [the full-chain prototype](https://github.com/inaku-Gyan/PathWrap/issues/89),
not product implementation. The first Open case stopped before any provider
action because this prototype incorrectly required Address Band Root to be the
toolbar's grandparent. The existing native chain contains an intermediate
msctls_progress32. This is an implementation defect, not evidence of failed
filename navigation. Its source is immutable commit
50048d1216817ffdcb2cbea6276a21aa874e33b9; its original open.jsonl and
environment.json are preserved here.

The corrected locator validates bounded ancestry to the original dialog and
records that chain. The human explicitly answered “你可以重试”. The remaining
execution is one corrected Open retry and the first Save case, using the same
checked correction and separate evidence under corrected/. There has been no
Save case yet at this preparation point. Further automatic repeats are excluded.

## Run

```powershell
cargo run --bin uia_filename_folder_jump_prototype -- --case open
cargo run --bin uia_filename_folder_jump_prototype -- --case save
```

Each invocation creates an owned fixture with A/B marker directories,
`pending-prototype-name.csv` and CSV filter index 2. The native host is setup,
independent observations and teardown only. OnFileOk is counted and vetoed;
an acceptance attempt fails the folder-jump condition. Native current-folder,
filename, type, original HWND, visibility and lifetime are sampled independently.

The external MTA worker finds a unique semantic filename Edit and a unique named
IDOK Button/SplitButton, with live original-dialog ancestry/ownership/visibility
checks. It captures the original Value, writes the existing B directory with a
trailing separator, verifies assignment, invokes once, and waits up to three
seconds for the fresh full-path address-toolbar Name to match B. Only that
external observation gates filename restoration; native host truth never enters
the worker. The toolbar is observed read-only. After restoration, Value and the
external B observation are checked again. Native final samples decide the
independent preservation assertion. The initial text is a controlled nonempty
sample, not a general filename-state compatibility claim.

Bounds: 64 elements per typed lookup, 16 ancestor steps, 1024 explicit calls,
12-second soft worker budget and 15-second supervisor deadline. Synchronous
provider calls can exceed soft limits. High-level successful returns do not
preserve raw success HRESULTs; those fields remain null. Exactly three action
call sites exist: target Value write, button Invoke and original Value restore.
Unsupported stages stop with an explicit result. There is no automatic action
retry. No extra type-restoration action is selected if type drift occurs.

The owned fixture uses a 100 ms native timer and at least one fresh final sample
is required. Logs distinguish worker process exit, worker chain result and the
native assertion. A successful process exit can contain an unsupported result.

No synthetic input, clipboard navigation, foreground forcing, service shutdown,
native navigation/restoration helper or alternative route. Existing product code
is unchanged. Required checks passed before GUI execution: `just check --ci`,
`just build`, `just test` (51 tests passed, five interactive/diagnostic tests
ignored). The Justfile was restored byte-for-byte after its temporary formatting.
Two new tests guard exact command captions and full-path arrival matching.
