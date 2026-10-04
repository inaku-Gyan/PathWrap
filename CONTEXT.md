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
- **Package verification:** for the personal-use first release, generation and
  readback of the ZIP's SHA-256; the package contract is documented separately
  and no additional ZIP-structure or GUI smoke check is required.
- **Release version:** the exact Cargo package version mirrored by a stable
  `vX.Y.Z` tag; the first formal release uses `v0.1.0`.
- **Release notes:** GitHub-generated change notes only; runtime instructions
  live in the README and are not a package file.
- **Published release:** the public GitHub Release record. A draft may be
  repaired by a rerun, but published assets are never overwritten by the
  release workflow.
- **Release build:** the locked `PathWarp` binary built from the pinned Rust
  toolchain and x64 MSVC target with Cargo's default release profile.
- **Build provenance:** the commit, tag, toolchain, target, and runner
  metadata recorded in Actions and Release notes; it is not another package
  file or release asset.
- **Interactive acceptance record:** optional evidence from the Windows
  clean-desktop check; it does not block the release workflow.
- **Release repair:** maintainer-only recovery for a bad published release;
  the workflow never deletes or overwrites public assets.

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

## Portable persistence terms

- **Storage directory:** the directory selected for persisted PathWarp data. By
  default it is `pathwrap-store` beside the running `PathWarp.exe`;
  `--storage-dir <PATH>` overrides it, with relative overrides resolved beside
  the executable.
- **Persisted configuration:** the current settings file at `<storage
  directory>\\config.json`. Each executable directory has its own default
  storage directory; an explicit override may intentionally select another
  location.
- **Memory-only mode:** the safe runtime mode entered when the storage
  directory cannot be created or written. Startup logs a warning, the
  application continues with in-memory settings, and it does not fall back to
  `%APPDATA%` or migrate legacy configuration.
- **Configuration format:** the persisted JSON is a small, non-versioned
  object. A missing `theme` uses the default preference; unknown fields,
  malformed content, and invalid values warn and restore the full default
  configuration instead of blocking startup or invoking a migration.
- **Persistence failure:** configuration writes use a same-directory temporary
  file and atomic replacement. A failed save preserves the last formal file,
  warns once, and switches the current process to memory-only mode until the
  next launch; leftover temporary files are never read as configuration.
