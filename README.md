# Tmuxwright

> **Deterministic E2E testing for terminal software.**
> Drives any TUI app as a black box inside an isolated tmux pane.

> ⚠️ Status: pre-v1. The current focus is a small terminal-mode v1:
> tmux orchestration, deterministic waits, traces, a Rust engine daemon,
> a TypeScript SDK, and a minimal test runner.

## What Tmuxwright is

- A **Playwright-shaped** test framework for terminal user interfaces,
  starting with a small serial runner rather than a full browser-test
  platform.
- A **Rust engine daemon** (`tmuxwright-engine`) exposed to the
  **TypeScript SDK** over local JSON-RPC on stdio. This is the v1
  boundary; native Node bindings are deferred until there is evidence
  they are needed.
- **tmux-native** — tmux is the runtime substrate for pane lifecycle,
  input injection, and screen capture. Failing tests can be inspected
  live by reconnecting to the preserved session.
- **Adapter-ready, not adapter-first** — framework-native adapters for
  Textual, Bubble Tea, and Ratatui remain a future direction. They do
  not block terminal-mode v1.

## Execution modes

| Mode     | What drives it                       | v1 status |
|----------|--------------------------------------|-----------|
| Terminal | tmux + ANSI screen model (black box) | Active    |
| Adapter  | framework-native semantics           | Future    |
| Hybrid   | tmux pane + adapter semantics        | Future    |

## Platforms

- macOS and Linux. Windows is out of scope for v1.
- Requires a recent `tmux` (target: 3.3+).

## Repository layout

```
crates/
  tmuxwright-tmux/      tmux process/session/input/capture primitives
  tmuxwright-term/      terminal grid, ANSI parsing, hashes, locators
  tmuxwright-rpc/       JSON-RPC 2.0 wire types and stdio framing
  tmuxwright-engine/    daemon used by TypeScript clients
  tmuxwright-core/      shared engine concepts under active consolidation
packages/
  tmuxwright/           TypeScript SDK
  tmuxwright-test/      minimal test runner
```

Future or experimental surfaces such as `tmuxwright-napi`,
`tmuxwright-adapter-ratatui`, `create-tmuxwright`, rich reporters, the
HTML trace viewer, and framework adapters are intentionally deferred
until terminal-mode v1 is coherent.

## Contributing

This project follows a strict working discipline:

- **Manual validation before automated tests.** See [`CONTRIBUTING.md`](./CONTRIBUTING.md)
  and [`tmp/README.md`](./tmp/README.md).
- **Small, byte-sized, conventional commits.** See [`COMMITS.md`](./COMMITS.md).

## License

See [`LICENSE`](./LICENSE).
