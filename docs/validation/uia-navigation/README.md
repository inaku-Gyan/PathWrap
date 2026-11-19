# Verified filename-field UIA navigation: Windows acceptance

The implementation follows the [accepted minimal contract](https://github.com/inaku-Gyan/PathWrap/issues/91#issuecomment-6992918045). Final owned Open and Save cases passed using the actual product `PathWarp.exe` worker. This is bounded Windows evidence, not a general host/path compatibility guarantee.

| Check | Open | Save |
| --- | --- | --- |
| Read selected type externally | `CSV (*.csv)` | `CSV (*.csv)` |
| Independent host type | index 2 | index 2 |
| Already-at-target preflight | zero writes / zero Invoke | zero writes / zero Invoke |
| Missing target directory | zero writes / zero Invoke | zero writes / zero Invoke |
| Folder change | A to B | A to B |
| Filename writes / named Invoke | 2 / 1 | 2 / 1 |
| Final pending filename | `pending-prototype-name.csv` | same |
| Type after navigation | original external value / native index 2 | same |
| Original dialog | live, visible, same HWND | same |
| Fresh final independent samples | 6 | 6 |
| File-accept attempts | 0 | 0 |
| Owned worker retired | confirmed | confirmed |
| Fixture cleanup | passed | passed |

The external worker uses fresh full-path address-toolbar observations before and after filename reassertion. Retained native `IFileDialog` is fixture setup, independent truth and teardown only. It never supplies the worker's arrival, filename or selected-type gate. Native samples are adjacent observations, not an atomic snapshot or proof of all intermediate states.

## Final evidence and reproducibility

- [Final Open result and native samples](final-open.jsonl), [stderr](final-open.stderr.txt).
- [Final Save result and native samples](final-save.jsonl), [stderr](final-save.stderr.txt).
- [Final source and executable SHA256 values](final-environment.json).
- [Acceptance runner](../../../src/bin/navigation_acceptance.rs), [navigation policy](../../../src/core/navigation.rs), [provider adapter](../../../src/os/dialog/uia.rs), [supervisor](../../../src/os/dialog.rs).

Windows 26300.9974 / 26H2 / zh-CN. Both final cases used the same product and acceptance executable hashes. The metadata records every changed Rust/Cargo input used by the final build; working-file byte hashes may differ from Git's normalized line endings after a fresh checkout. The product executable also matches the version used by the authorized E2E run. Fixture paths are replaced with `FIXTURE_ROOT`/`PROTOTYPE_ROOT`; these markers are not production completion rules.

Build all targets, then run the cases sequentially:

```powershell
just build
cargo run --bin navigation_acceptance -- --case open
cargo run --bin navigation_acceptance -- --case save
```

Announce these GUI checks. If normal activation is denied, activate the named owned fixture by clicking its title bar, then wait for it to close automatically. Do not type into or switch away from the fixture while a case runs. The harness waits up to 120 seconds for setup activation, then applies the product worker's bounds. It checks the exact initial pending filename before selecting navigation. It closes only its owned host and removes only the five known empty fixture directories, non-recursively.

## Checks

- `just check --ci`, `just build`, `just test`: passed on the final source.
- Noninteractive tests: 64 passed. Six ignored entries are the owned blocking-worker test fixture (launched explicitly by its parent timeout regression) and five interactive/diagnostic E2E entries.
- The user explicitly permitted **one** `just e2e` invocation. All five entries passed in 12.11 seconds: overlay activation/docking, close/reopen recovery, reopen tracking and two diagnostics. This tests the app/overlay lifecycle; it does not establish every navigation/focus race.
- Policy regressions cover missing type, no-op, wrong/relative path matching, failed Invoke without cleanup, actual mock-state filename repair, type drift, filename interference and preservation failure. Mock repair is not native provider recovery evidence.
- Controller/UI regressions cover one in-flight transaction, wrong/stale/late completion, process identity changes, Escape, unrelated foreground apps, visible recovery filename and blocking new mutations if worker retirement is unconfirmed.
- An actual owned blocking child checks watchdog termination/reaping and retention of the original pending filename. The actual `PathWarp.exe --navigation-worker` entry point rejects an invalid target without starting the GUI or attempting mutation.
- Justfile was restored byte-for-byte after required checks. No new service configuration, product keyboard/mouse simulation or navigation fallback was added.

## Locator correction and preserved diagnostic attempts

The old Save background trace exposed `FileTypeControlHost`. The current owned modern dialogs instead expose the native ComboBox representation with AutomationId `1136` and a file-type label. The original exact-ID locator returned unsupported with zero writes/Invoke ([initial type attempt](open-type-first.jsonl), [requested retest](open-type-retest.jsonl)). The implementation now identifies one semantic type picker using that guarded native representation or `FileTypeControlHost`. Both use the same sole `ValuePattern.CurrentValue` getter; there is no second getter, type-selection action or runtime fallback chain. The successful dedicated [Open](open.jsonl) and [Save](save.jsonl) type-gate runs, with [their executable/module hashes](environment.json), precede the final app/controller integration checks.

Three setup activation failures are retained separately ([first](open-activation-first.jsonl), [second](open-activation-second.jsonl), [third](open-activation-third.jsonl)). They never selected navigation. Normal `SetForegroundWindow` was denied. Computer Use attempts ended, and the successful fixture setups required a user title-bar click. This is a setup limitation; it does not establish product activation behavior for arbitrary foreground applications.

A final-build Open attempt captured an unexpected initial filename `#`, different from the configured fixture. It reached B, then stopped at the filename ownership/reassertion guard instead of overwriting an unrecognized pending value. [That incomplete result](final-open-interrupted.jsonl) and [its build metadata](final-environment-before-setup-guard.json) are preserved. No cause is assigned to the unexpected input. The harness now rejects an unexpected initial filename before selecting navigation; the later final Open/Save successes are separate cases. Mutation counters count operation attempts, including an operation stopped by its guard, rather than proving a successful provider write.

The earlier setup/locator diagnostics have limited source provenance and are not positive capability evidence. Positive final claims use the final hashed executables and the final result files above.

## Limits and remaining delivery boundaries

Coverage is a controlled local directory and one stable nonempty filename/type selection per modern dialog on this Windows build. Empty/multiple filenames, UNC/shell/alias path forms, other locales/hosts, ambiguous duplicate type values, arbitrary editing states and exhaustive stale/foreground races are not established. The adapter fails closed on missing or unreliable observations. A failed provider call, timeout or interruption can leave a changed directory/filename; no return-to-A or automatic filename/type cleanup is promised. In-flight provider effects cannot be assumed undone by retiring a worker.

Navigation runs in an owned process with a three-second soft arrival observation, twelve-second soft worker budget, fifteen-second supervisor boundary, at most 2048 budgeted UIA calls after initial binding, 64 elements per typed lookup, 16 ancestry steps and 256 native children. Successful completion also requires confirmed child retirement. App shutdown signals cancellation and joins its supervisor; unconfirmed retirement blocks another mutation.

Completion/error focus follows the accepted simple focus policy. [Session-scoped Ctrl+G](https://github.com/inaku-Gyan/PathWrap/issues/67) remains its separate implementation ticket; this change does not add a global keyboard hook or claim that shortcut is delivered. Product release, PR publication and merge into main are separate from this checked implementation branch.

## PR review follow-up

Independent Standards review found no material issues. Spec review found two gaps: Escape notified the navigation supervisor only on the next UI frame, and recovery text omitted separately observed operation state. Both are corrected. UI events now fence navigation before their effects run; cancellation does not depend on native foreground changing. Failure and unretired-worker notices retain the original filename, stopping stage and explicitly last-read folder/filename/type, with unread observations marked unknown.

`just check --ci`, `just build` and `just test` passed after these fixes: **66 noninteractive tests passed, 6 ignored** (34 library, 31 app/config/UI and one actual product-worker entry-point test). Expanded UI/controller checks retain the recovery observations, an unread-state regression checks unknown markers, and an actual owned blocking-child test checks session cancellation and confirmed retirement while foreground is unchanged. Both review axes rechecked the fixes with no remaining findings.

The full native Open/Save evidence above remains tied to its original source/executable hashes; those GUI cases were not repeated for this follow-up. Same-frame cancellation signalling reduces an avoidable delay and does not guarantee cancellation or undo of a provider call already in flight.
