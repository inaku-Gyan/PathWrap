# Issue tracker: GitHub

Issues and specs for this repo live as GitHub Issues. Use the `gh` CLI for all operations.

## Conventions

- Create an issue: `gh issue create --title "..." --body "..."`.
- Read an issue: `gh issue view <number> --comments`.
- List and filter: `gh issue list --state open --json number,title,body,labels,comments`.
- Comment: `gh issue comment <number> --body "..."`.
- Add or remove labels: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`.
- Close: `gh issue close <number> --comment "..."`.

The repository is `inaku-Gyan/PathWrap`. Pull requests are not a triage request surface.

## Issue roles

- **Map:** an index for one large effort. It records the destination, resolved decisions, remaining fog, and scope; it is not a duplicate task list.
- **Decision ticket:** a child of a map that resolves one choice. Use `wayfinder:grilling` for a live decision, `wayfinder:research` for an external fact, or `wayfinder:prototype` for a cheap concrete artifact.
- **Wayfinder task:** a manual migration or investigation prerequisite that must happen before a decision can be made. Reserve `wayfinder:task` for this role.
- **Product implementation issue:** an ordinary GitHub Issue for work such as a feature, bug fix, or documentation change. It may link to a decision or depend on it, but it is not made a child of a migration map just to appear in its backlog.

Each Wayfinder child has exactly one `wayfinder:*` type label. Triage labels describe current status and are mutually exclusive: use `needs-triage`, `ready-for-agent`, `ready-for-human`, `needs-info`, or `wontfix` as appropriate. Assignment records ownership; it does not replace the status label.

## Wayfinding operations

- **Map:** one issue labelled `wayfinder:map`, containing Destination, Notes,
  Decisions so far, Not yet specified, and Out of scope.
- **Child ticket:** a GitHub sub-issue of the map, labelled
  `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or
  `wayfinder:task`.
- **Claim:** assign the ticket to the driving developer before doing work.
- **Blocking:** use GitHub native issue dependencies. If dependencies are not
  available, put `Blocked by: #<number>` at the top of the child issue body.
- **Frontier:** open, unassigned map children with no open blockers.
- **Resolve:** comment the answer, close the ticket, then append its gist and
  link to the map's `Decisions so far` section.

Use `sub-issue` only for membership in a map. Use `blocked by` only for a hard
prerequisite; use links or comments for related work and soft ordering. A
grilling or prototype ticket is complete only after the human has confirmed
the decision. Research may be resolved by an agent. If a ticket is outside
the map's destination, close it and record it under `Out of scope` rather than
under `Decisions so far`.

The former `PLAN.md` was retired and removed during the migration. Do not
recreate it or add planning items to another local markdown file; the GitHub
map and Issues are canonical.

For native sub-issues and dependencies, use `gh api` against the repository's
sub-issues and issue-dependencies endpoints.
