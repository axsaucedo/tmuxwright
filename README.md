# Tmuxwright

> **Deterministic E2E testing for terminal software.**
> Drives any TUI app as a black box inside an isolated tmux pane, with
> optional semantic adapters for **Textual**, **Bubble Tea**, and **Ratatui**.

> ⚠️ Status: pre-v1. The workspace is being set up; nothing is released yet.
> See [`plan.md`](./plan.md) in the session workspace for the full build plan.

## What Tmuxwright is

- A **Playwright-style** test framework — standalone runner, config file,
  rich reporters, HTML trace viewer — but for terminal user interfaces.
- A **Rust core** (actions, waits, screen model, traces) exposed to a
  **TypeScript SDK** via [napi-rs](https://napi.rs/).
- **tmux-native** — tmux is the runtime substrate for pane lifecycle,
  input injection, and screen capture. Failing tests can be inspected
  live by reconnecting to the preserved session.
- **Adapter-aware** — when the app is built with Textual, Bubble Tea, or
  Ratatui, the same top-level API transparently uses framework-native
  semantics (focus, widget identity, role/name locators) via local
  JSON-RPC adapters.

## Execution modes

| Mode         | What drives it                              | Works with                 |
|--------------|---------------------------------------------|----------------------------|
| Terminal     | tmux + ANSI screen model (black box)        | Any TUI                    |
| Adapter      | framework-native semantics                  | Textual / Bubble Tea / Ratatui |
| Hybrid       | tmux pane + adapter semantics (recommended) | Supported frameworks       |

## Platforms

- macOS and Linux. Windows is out of scope for v1.
- Requires a recent `tmux` (target: 3.3+).

## Repository layout

See [`plan.md`](./plan.md) §3 for the authoritative layout. High level:

```
crates/      Rust engine, tmux control, terminal model, RPC, Ratatui adapter, napi bindings
packages/    TypeScript SDK, standalone test runner, scaffolder
adapters/    Python (Textual) and Go (Bubble Tea) RPC adapter packages
examples/    Sample apps + tests for each mode
```

## Contributing

This project follows a strict working discipline:

- **Manual validation before automated tests.** See [`CONTRIBUTING.md`](./CONTRIBUTING.md)
  and [`tmp/README.md`](./tmp/README.md).
- **Small, byte-sized, conventional commits.** See [`COMMITS.md`](./COMMITS.md).

## License

See [`LICENSE`](./LICENSE).
