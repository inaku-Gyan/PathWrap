# Domain Docs

This repository uses a single-context layout.

## Before exploring

- Read `GLOSSARY.md` at the repo root, if it exists.
- Read relevant architectural decision records under `docs/adr/`,
  if that directory exists.
- If either is absent, proceed silently. Create domain docs lazily
  through `/domain-modeling` when terms or durable decisions are resolved.

## File structure

- `GLOSSARY.md`: shared domain terms for the repository.
- `docs/adr/`: architectural decision records, such as
  `0001-descriptive-decision-name.md`.

## Vocabulary and decisions

Use the glossary's terms in issue titles, proposals, hypotheses, tests,
and code discussions. If a needed concept is missing, reconsider an
invented term or note the gap for `/domain-modeling`.

When a proposal contradicts an existing ADR, identify the conflict
explicitly and explain why the decision should be revisited.
