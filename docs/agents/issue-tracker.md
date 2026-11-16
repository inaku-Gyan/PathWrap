# Issue tracker: GitHub

Issues and specs live in GitHub Issues for `inaku-Gyan/PathWrap`.
Use the `gh` CLI from this checkout.

Before the first issue creation of a session, run
`gh repo view --json nameWithOwner` and verify the target is
`inaku-Gyan/PathWrap`. If needed, pass `--repo inaku-Gyan/PathWrap`
explicitly to issue commands.

## Conventions

- Create an issue: `gh issue create --title "..." --body-file <body-file>`.
- Read an issue: `gh issue view <number> --json number,title,body,labels,comments`.
- List issues: `gh issue list --state open --json number,title,body,labels,comments`.
- Comment: `gh issue comment <number> --body-file <body-file>`.
- Apply or remove labels: `gh issue edit <number> --add-label "..."`
  or `gh issue edit <number> --remove-label "..."`.
- Close: post the explanation as a comment, then run
  `gh issue close <number>`. This also preserves the explanation when
  the issue was already closed.
- For multiline bodies and comments, write the exact text to a temporary
  file and use `--body-file`.

## Sub-issues and blockers

- Create a child: `gh issue create --parent <parent> --title "..." --body-file <body-file>`.
- Attach an existing child: `gh issue edit <parent> --add-sub-issue <child>`.
- Add a blocker: `gh issue edit <child> --add-blocked-by <blocker>`.
  New issues also support `--blocked-by <number>,<number>`.
- Count open blockers:
  `gh api repos/inaku-Gyan/PathWrap/issues/<number> --jq .issue_dependencies_summary.blocked_by`.
  Zero means no open blockers.
- If sub-issues are unavailable, use a task list in the parent and
  `Part of #<parent>` at the top of the child body.
- If native dependencies are unavailable, put
  `Blocked by: #<number>, #<number>` at the top of the child body.
  Work is unblocked when every blocker is closed.

Use sub-issues for parent membership and dependencies for hard prerequisites.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## Skill operations

When a skill says "publish to the issue tracker", create a GitHub issue.
When it says "fetch the relevant ticket", read the issue body and comments.

## Wayfinding operations

- Map: one issue labelled `wayfinder:map`, holding the destination,
  notes, decisions so far, remaining fog, and scope.
- Child: a decision or prerequisite ticket linked to the map as a
  sub-issue, with exactly one type label: `wayfinder:research`,
  `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`.
  Reserve `wayfinder:task` for manual migration or investigation prerequisites.
- Product implementation issues may link to decisions or depend on them.
  Keep ordinary implementation work outside the map's decision tickets.
- Frontier: open, unassigned map children with no open blockers;
  choose the first in map order.
- Claim: assign the ticket to the driving developer before starting,
  using `gh issue edit <number> --add-assignee "@me"` when appropriate.
- Resolve: comment the answer, close the ticket, and add its gist and
  link to the map's decisions so far.
- Grilling and prototype decisions require human confirmation.
  Research tickets may be resolved by an agent.
- Keep at most one triage status label on an issue. Assignment records
  ownership; `wayfinder:*` labels record ticket type.
