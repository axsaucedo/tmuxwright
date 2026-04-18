# `tmp/` — Manual validation sandbox

This directory is **gitignored** except for this README. It is the designated
home for one-off, exploratory, and manual-validation scripts used while
building Tmuxwright.

## Why this exists

Per `plan.md` §2a, every feature in Tmuxwright is **first proven manually**
before any automated test is written. The flow for each todo is:

1. Implement the feature.
2. Write one or more scripts under `tmp/<todo-id>/` that drive the feature
   end-to-end — happy path, error path, edge cases.
3. Iterate on the implementation until those scripts behave correctly.
4. **Only then** translate the validated behavior into a persistent
   automated test (unit / tmux integration / adapter contract / e2e).
5. Commit the feature and the test in **separate** small commits.

## Conventions

- Create one subdirectory per todo, named after the todo id:
  `tmp/b3-input-inject/`, `tmp/h1-textual/`, `tmp/e1-rpc/`, etc.
- Every script starts with a header comment that explains:
  - What todo/feature it validates
  - How to run it
  - What the expected observable outcome is
- Capture outputs (logs, snapshots, traces) alongside the script when
  useful for later reference.
- Scripts can be in any language — shell, Rust bin, Node, Python, Go —
  whichever is most direct for the thing under test.

## What does *not* go here

- Automated tests. Those live in their proper language-idiomatic
  locations (`cargo test`, `vitest`, `pytest`, `go test`).
- Any production code.
- Anything that must survive in git history — this whole tree is
  gitignored by design.

## The final report

When all todos reach `done`, a `REPORT.md` (also gitignored) will be
written at the repo root summarizing the manual-validation evidence
that lives here.
