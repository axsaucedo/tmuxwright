# Contributing to Tmuxwright

Thanks for your interest in Tmuxwright. This document captures the
non-negotiable working rules for contributions. Detailed architecture
lives in [`plan.md`](./plan.md) (in the session workspace during
bootstrap) and eventually in `docs/`.

## The working discipline

Every feature in Tmuxwright goes through the same five-step loop. Do
**not** skip steps.

1. **Implement** the feature in the appropriate crate/package.
2. **Manually validate** it using one or more scripts under
   `tmp/<todo-id>/`. The `tmp/` tree is gitignored — see
   [`tmp/README.md`](./tmp/README.md). The goal at this step is to
   exercise happy path, error paths, and relevant edge cases by hand
   until the behavior is clearly correct.
3. **Iterate** on the implementation until the manual scripts pass
   consistently.
4. **Write an automated test** that locks the behavior in. Unit tests
   live next to the code; integration tests live in the appropriate
   suite (tmux integration, adapter contract, e2e against examples).
5. **Commit** the feature and the test as **separate small commits**
   using the conventions in [`COMMITS.md`](./COMMITS.md).

A change is **not done** until all five steps are complete and CI is
green.

## Commit style

See [`COMMITS.md`](./COMMITS.md). TL;DR:

- Conventional format: `type(scope): summary`.
- One logical change per commit.
- Feature commit and test commit are always separate.
- Body references the todo id (`Refs: <id>`) and, where relevant,
  the manual validation script used (`Validated via: tmp/.../...`).
- Always include the `Co-authored-by: Copilot` trailer.

## Code style

- Rust: `cargo fmt` + `cargo clippy --all-targets -- -D warnings`.
- TypeScript: `prettier` + `eslint` (configs land in workstream A).
- Python (Textual adapter): `ruff` + `mypy`.
- Go (Bubble Tea adapter): `gofmt` + `go vet`.

## Repository layout

Authoritative layout is in `plan.md` §3. Please do not introduce
top-level directories outside that layout without updating the plan
first.
