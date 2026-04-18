# Commit guidelines

Tmuxwright uses **small, byte-sized, conventional commits** so that every
change is easy to review in a single sitting and easy to bisect later.

## Format

```
type(scope): short imperative summary

Optional body explaining *why* the change is being made — context, the
problem it solves, and anything non-obvious about the approach.

Refs: <todo-id>
Validated via: tmp/<todo-id>/<script>   (when applicable)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
```

### Types

| Type       | Use for                                                        |
|------------|----------------------------------------------------------------|
| `feat`     | New user-visible capability                                    |
| `fix`      | Bug fix                                                        |
| `refactor` | Internal restructuring, no behavior change                     |
| `test`     | Adding or adjusting automated tests                            |
| `docs`     | Documentation only                                             |
| `chore`    | Repo hygiene (gitignore, configs, metadata)                    |
| `ci`       | Continuous integration changes                                 |
| `build`    | Build system, workspace, or dependency changes                 |

### Scope

A short noun naming the area affected. Examples: `core`, `tmux`, `term`,
`rpc`, `napi`, `sdk`, `runner`, `adapter-textual`, `adapter-bubbletea`,
`adapter-ratatui`, `examples`, `ci`, `docs`.

## Rules

1. **One logical change per commit.** If a diff does two things, split it.
2. **Feature commits and test commits are separate.** A reviewer should be
   able to read the feature commit first (seeing what was built) and then
   the test commit (seeing how it is locked in). This also lets a reviewer
   run the feature's tests against the *previous* state to confirm the test
   would have failed before the fix.
3. **Reference the todo id** in the commit body via `Refs: <id>` so commits
   can be tied back to the plan.
4. **Record manual validation.** When the change implements or fixes a
   feature that was first exercised via a script under `tmp/<todo-id>/`,
   list the script in the `Validated via:` line.
5. **Always include the Co-authored-by trailer** for Copilot-assisted work.
6. **Imperative summary under ~72 chars.** "add X", "fix Y", not "added"
   or "fixes".
7. **No mixed concerns.** Never combine a refactor with a behavior change.
   Never combine test additions with the feature they cover.

## Example

```
feat(tmux): support literal paste via load-buffer/paste-buffer

Direct send-keys mangles characters that tmux interprets as key names
(e.g. "Enter", "Space"). Routing literal text through load-buffer +
paste-buffer preserves bytes exactly, which matters for locator text
and typed credentials.

Refs: b3-input-inject
Validated via: tmp/b3-input-inject/literal_paste.sh

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
```
