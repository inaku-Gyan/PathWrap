# PathWarp development

## Verification

Add meaningful behavior tests and regression tests where practical.

For substantive Rust changes, use these checks on Windows:

```sh
just check --ci
just build
just test
```

Use Cargo directly when a recipe does not cover a focused test or needed
option. Use lighter checks for documentation, formatting, and small changes.

For dialog, focus, docking, keyboard, or IME changes, announce and run
relevant E2E or GUI checks. Use `just e2e` when appropriate.
But ask the user before each time `just e2e`, in case it conflicts with what the user is doing.

For release work, follow the README and release workflow.

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues for `inaku-Gyan/PathWrap`; use `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: root `GLOSSARY.md` and `docs/adr/`. See `docs/agents/domain.md`.
