# PathWrap Context

This glossary records stable workflow terms. GitHub Issues remain the source
of truth for plans, decisions, dependencies, and the product backlog.

## Workflow terms

- **Wayfinder map:** an index for one large effort. It names the destination,
  points to child tickets, and keeps a short index of resolved decisions.
- **Decision ticket:** a map child that resolves one choice or investigation.
  Its type is `wayfinder:grilling`, `wayfinder:research`, or
  `wayfinder:prototype`.
- **Wayfinder task:** a manual prerequisite needed before a decision can be
  made. Its type is `wayfinder:task`; it is not a general feature label.
- **Product implementation issue:** an ordinary GitHub Issue for a feature,
  bug fix, or documentation change. It can depend on a decision ticket without
  becoming a child of the migration map.
- **Frontier:** the open, unassigned map children whose hard blockers are all
  closed.
- **Resolution:** the answer comment, closure, and one-line pointer appended
  to the map. A human confirmation is part of resolving a HITL ticket.
- **Quality workflow:** read-only automation that verifies a source revision;
  it never publishes a release.
- **Release workflow:** tag-driven automation that invokes the quality
  workflow, prepares a release artifact, and coordinates its publication.
- **Release job:** the isolated publication step with write authority for the
  GitHub Release; ordinary quality runs do not have that authority.
- **Protected release tag:** a repository-governed `vX.Y.Z` reference accepted
  as the trigger for a formal release.

## Distribution terms

- **Portable package:** the user-facing `PathWarp-vX.Y.Z-windows-x64.zip`
  with a same-named root directory and exactly `PathWarp.exe`, `LICENSE`, and
  `README.md`.
- **Release asset:** an individual file attached to a GitHub Release. The
  portable package and its `.sha256` checksum are separate release assets.
- **Package verification:** structural checks for the expected files, paths,
  architecture label, and extraction; byte-level ZIP reproducibility is not
  required.

## Theme terms

- **Theme preference:** the user's appearance policy: follow the Windows system
  theme, always use light, or always use dark. _Avoid_: resolved theme when
  referring to the user's choice.
- **Resolved theme:** the concrete light or dark appearance currently applied
  after resolving a theme preference against the Windows system setting.
  _Avoid_: theme preference when referring to the appearance on screen.

## Overlay header terms

- **Search frame:** the larger rounded boundary around the filter input. Its
  neutral border carries a subtle hover feedback; focus is represented by the
  editor caret rather than a thick outer ring.
- **Search control:** the clickable region inside the search frame containing
  the magnifier and the `egui::TextEdit` query editor. It has no separate hover
  border and owns the query buffer used for path filtering.
- **Theme control:** the compact sun/moon button beside the search frame. It
  keeps its own hover/focus feedback and context menu independently of the
  search frame; completing a theme action returns focus to the search editor.
- **Overlay focus:** the real Windows foreground/focus state of the activating
  overlay. Clicking the overlay makes it the foreground window; the controller
  keeps the current file-dialog session visible while that focus is held.
- **Search editor:** the focused `egui::TextEdit` bound directly to the
  controller query. Its blinking caret, selection, clipboard, and text/IME
  events are the input surface; no custom caret or global keyboard hook is used.
