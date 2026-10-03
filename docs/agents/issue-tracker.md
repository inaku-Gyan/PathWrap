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

For native sub-issues and dependencies, use `gh api` against the repository's
sub-issues and issue-dependencies endpoints.
