# One address ComboBox Invoke — throwaway capability probe

Question: does one Invoke on the exact address ComboBox child change address
state or expose an editor/menu in a controlled modern Open dialog?

This artifact supports [Prototype the address ComboBox button's UIA Invoke
action](https://github.com/inaku-Gyan/PathWrap/issues/74), following the user's
[selected experiment](https://github.com/inaku-Gyan/PathWrap/issues/73#issuecomment-6930629133).
It reuses the initialized owned fixture/observer from the accepted prototype at
`657de8701358f7a6bdc45a198b7629b0dd9214bb`. Production code is unchanged.

## Finding

**One Invoke opened a visible address dropdown. The native address ComboBox
and Edit remained hidden, and no folder jump was attempted or observed.**

The call returned S_OK in **38.043 ms**. The following observations, rather
than that HRESULT, establish the bounded state change:

| Observation | Before | After the one Invoke |
| --- | --- | --- |
| Native ComboBoxEx32 / ComboBox / Edit visibility | All false | All still false in every captured frame |
| UIA ComboBox subtree | 2 elements | 33 elements, including a ComboLBox and 25 ListItems |
| Native visible fixture-owned popup windows | 0 | ComboLBox and SysShadow |
| Exact child Button's Legacy default action | `打开` (Open) | `关闭` (Close) |
| UIA focused element | Another process | Address Edit in the first sample, then address ComboBox |
| Independent current folder | A | A throughout |
| Pending filename / type index | `pending-prototype-name.csv` / 2 | Unchanged |

This supports a **dropdown-opening effect in this fixture**. It does not
establish visible address-edit mode: the native editor remains hidden despite
the UIA focus report. The ListItems expose Invoke with a Legacy `双击`
(double-click) description, and the Button now exposes its Close action.
Neither was invoked. No recognized Go action appeared in the captured main
address subtree. The ComboBox's own Value readback stayed empty.

The full main-dialog UIA tree had 108 elements before and in each after frame.
The new list was reached through the exact ComboBox subtree and read-only
native popup discovery. Desktop-root UIA discovery exposed the shadow alone;
native EnumWindows plus ElementFromHandle exposed the ComboLBox tree. A
desktop-root count alone would miss the substantive popup in this sample.

## Run and artifacts

On this throwaway branch, on Windows:

```powershell
cargo run --bin uia_address_combo_invoke_prototype
```

- [Exact executed Rust source](../../../src/bin/uia_address_combo_invoke_prototype.rs).
- [Scrubbed chronological JSONL trace](open.jsonl).
- [Extracted observations](summary.json).
- [Environment](environment.json).

The command owns a separate STA Common Item Open fixture and an external MTA
UIA worker. It initializes a scratch A folder, the nonempty pending filename
and CSV filter 2 before choosing the baseline. Its retained IFileDialog is
used only for owned setup and independent, read-only native observations.
It is not an interface acquired by an ordinary external client.

Discovery is scoped to the live fixture's address band and its native
ComboBox descendants. The target must be the unique direct UIA child Button
with the observed zh-CN fixture metadata: name `地址`, Legacy role 43, default
action `打开`, and an available Invoke pattern. Raw-walker parent comparison
confirms its exact ComboBox parent. The trace retains native ancestors, HWNDs,
process identities and provider descriptions. These names and classes are
fixture evidence, not universal selectors or a claim that the Button is Go.
An absent or ambiguous target stops the probe without substitution.

The only client provider action is **one Invoke call**. Discovery and all
subsequent operations are read-only. There are no SetFocus/SetValue calls,
follow-up actions, Save comparison, input simulation, clipboard operation or
alternate navigation mechanism. Owned fixture setup and WM_CLOSE teardown
are separate from that action count.

The supervisor bounds the entire UIA worker to 15 seconds, then closes its
owned fixture; a hung client is killed without retry and its HRESULT remains
unknown. The normal run did not time out. Fixture cancellation and both
process exits were observed; no prototype processes remained afterward.
Scratch folders are retained locally for inspection.

## Timing, independent truth and limits

- The observation schedule targets three seconds after Invoke returns, with
  read-only snapshots starting at offsets 0, 1000, 2000 and 2900 ms. The final
  synchronous snapshot finished at **3419 ms**, so this capture exceeded the
  requested three-second observation window by 419 ms. The phase labels are
  scheduled offsets, not promises that every read completed at that offset.
  It was one action and one Open case; no timing-adjustment rerun occurred.
- There were 31 fresh native host polls during that measured interval. Every
  one reported A, the initialized filename/type and the same live, visible
  dialog. A further three fresh polls after the client exited confirmed that
  state before teardown. This validates preservation through a dropdown
  experiment, **not preservation after successful navigation**.
- The desktop was not clean: a Listary-class window owned the foreground and
  UIA focus in the before sample. After Invoke, the fixture dialog owned the
  foreground and its address controls were reported focused. No separate
  focus action was issued. The run does not isolate Listary's contribution
  or establish behavior without third-party dialog integrations. Other apps'
  window titles and UI trees were not captured.
- Coverage is one initial-state modern Open fixture on Windows build
  26300.9974/x64, zh-CN, with inherited tokens; integrity levels were not
  independently queried. Save, other applications/providers, elevated or
  brokered dialogs, typing into a visible editor, committing an arbitrary B
  destination, external completion verification, and successful-jump focus
  handoff remain untested.
- `probe_start.client_pid` is the inherited label for the launching supervisor;
  the MTA worker runs in its own child process. Fixture target PIDs and
  provider descriptions are recorded independently.

## Publication and verification

The trace preserves chronological records, tree shape, identities, patterns,
action/result counts and zero/unchanged observations. Runtime normalization
replaces the fixture root and user-profile prefix. Before publication, 104
remaining profile-relative path strings in popup names were additionally
replaced with `NON_FIXTURE_PATH`; other absolute non-fixture paths had already
been redacted by the executed source. These redactions hide list-entry paths,
not their control counts, roles or exposed actions. No additional GUI run or
source behavior change was made after capture.

`just check --ci`, `just build` and `just test` passed. All 47 existing tests
passed; five interactive/diagnostic tests remained ignored. No new prototype
tests were added. The announced controlled GUI capability check above ran;
`just e2e` was not invoked. A CRLF-only Justfile checkout difference was
normalized locally for the formatter check.

This is evidence for human review. It does not select a next experiment,
resolve the HITL decision by itself, or make the UIA navigation contract
implementation-ready. Publish on a throwaway branch; no PR or merge into main.
