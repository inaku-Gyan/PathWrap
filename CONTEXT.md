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

## Theme terms

- **Theme preference:** the user's appearance policy: follow the Windows system
  theme, always use light, or always use dark. _Avoid_: resolved theme when
  referring to the user's choice.
- **Resolved theme:** the concrete light or dark appearance currently applied
  after resolving a theme preference against the Windows system setting.
  _Avoid_: theme preference when referring to the appearance on screen.

## Overlay header terms

- **Search frame:** the larger rounded boundary around the filter input. Its
  border carries the search hover/focus feedback.
- **Search control:** the clickable region inside the search frame containing
  the magnifier, query text, and caret. It has no separate hover border.
- **Theme control:** the compact sun/moon button beside the search frame. It
  keeps its own hover/focus feedback and context menu independently of the
  search frame.
