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
