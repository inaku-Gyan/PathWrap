# Context-menu address edit entry — throwaway prototype

**The explicitly authorized corrected rerun still did not demonstrate
visible address editing.** One ShowContextMenu request returned
successfully; within the 3021 ms discovery interval, the corrected native
popup selections were empty and all 15 Menu-only array reads returned
length zero. No Edit-address command was invoked. The original dialog, A,
pending filename and type were preserved.

This is [Prototype address edit entry with corrected context-menu
observation](https://github.com/inaku-Gyan/PathWrap/issues/85). It directly
tests a proposed entry route toward an address-bar folder jump, not another
broad pattern inventory. Arbitrary A-to-B navigation remains unverified.

## The two recorded cases

| Case | Executed source | Result and interpretation |
| --- | --- | --- |
| Initial case | [6d21c4a4c93c2c1d903904f9f550976943fe6a5b](https://github.com/inaku-Gyan/PathWrap/blob/6d21c4a4c93c2c1d903904f9f550976943fe6a5b/src/bin/uia_address_context_edit_prototype.rs) | One successful request, no Edit Invoke; its reused-window filter left incomplete popup coverage. |
| Human-authorized corrected case | [f68a7ac19f775ec6bcf950cf510a3041f193e020](https://github.com/inaku-Gyan/PathWrap/blob/f68a7ac19f775ec6bcf950cf510a3041f193e020/src/bin/uia_address_context_edit_prototype.rs) | One successful request, no observed menu/command in the sampled scopes; the pre-existing-HWND exclusion was removed. |

The human first selected [the one-case protocol](https://github.com/inaku-Gyan/PathWrap/issues/84#issuecomment-6988637950),
then explicitly answered “允许” to [one additional corrected
case](https://github.com/inaku-Gyan/PathWrap/issues/85#issuecomment-6989409657). Exactly two GUI
cases belong to this prototype. No further automatic rerun occurred.

## Evidence

The [corrected rerun report](rerun/README.md) contains its exact observations,
timing, provenance and limitations:

- [Corrected trace](rerun/open.jsonl).
- [Corrected summary](rerun/summary.json).
- [Corrected environment and executed source/binary hashes](rerun/environment.json).

The first case remains unchanged:

- [Original limited trace](open.jsonl), [summary](summary.json) and
  [environment](environment.json).
- [Original report and exact source at its captured commit](https://github.com/inaku-Gyan/PathWrap/blob/6d21c4a4c93c2c1d903904f9f550976943fe6a5b/docs/prototypes/uia-address-context-edit/README.md).

The original summary's
`popup_observation_complete_under_selected_intent=false` remains valid for
that first case. Its program field `complete=true` concerns execution and
budgets, not complete popup coverage. Those historical records were not
rewritten to imply that the first case used the corrected filter.

## What was corrected and tested

The original native selection excluded HWNDs present in the baseline unless
their class was #32768. That could miss a pre-existing hidden nonstandard
popup made visible later; 18 hidden baseline windows were subject to the
exclusion. The trace does not establish that such a window actually
appeared.

The corrected source removes both baseline-membership exclusions. Every
visible fixture-owned top-level window other than the original dialog is
eligible regardless of earlier membership or class. Membership is recorded
as provenance only. Native shadow evidence is retained, while SysShadow
decorations are excluded from semantic menu binding. Explicit Menu-array
lengths are now logged.

The source fresh-binds the original dialog PID/HWND and address toolbar
beneath Breadcrumb Parent. A separate MTA action path requests its context
menu while the observer records native windows before potentially blocking
UIA queries. It also performs Menu-only lookup inside the original owned
dialog. No unrelated application UIA names/tree or full pattern catalog is
captured.

Only a unique owned live visible/enabled MenuItem whose normalized caption
is exactly 编辑地址 or Edit address can receive one Invoke after a fresh
uniqueness, identity, state and ancestor check. No candidate reached that
path in either case. The runtime command and post-command editor paths
therefore remain unexercised.

Limits remain 16 native binding seeds, 64 menu/address elements, depth 8,
512 explicit UIA calls, a ten-second soft worker budget and fifteen-second
supervisor. At most one context request and one conditional Edit-address
Invoke per separately authorized case. No fallback action, simulated input,
clipboard, SetFocus, setter, service shutdown or explicit foreground
forcing.

## Validation and disposition

The executed corrected source had already passed `just check --ci`,
`just build` and `just test`: 47 existing tests plus two exact-caption
guard tests, with five interactive/diagnostic tests ignored. Its source and
binary hashes matched that checked revision immediately before the rerun
and remained unchanged afterward. The corrected source was not edited for
this data-only publication; required checks were not needlessly repeated.
JSON, call-pairing, preservation, privacy and diff checks were performed on
the new evidence. No `just e2e`, product change, PR or merge.

This result is a bounded observation, not proof that all context-menu or UIA
navigation mechanisms are impossible. Native polling is sampled; very
brief transient states, other process scopes and different foreground/
dialog states are not excluded. No additional visible popup occurred, so
the reused-popup binding path had no positive runtime example.

The corrected case did not enter a visible editor or attempt B assignment
or commit. Read-only preservation around a menu request does not establish
successful-jump preservation or production-available arrival verification.
Keep this prototype open for human acceptance; the navigation contract
remains blocked, and no further experiment is selected by this report.
