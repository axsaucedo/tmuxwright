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

## Current API shape

The SDK launches commands through the Rust engine daemon:

```ts
import { launch } from 'tmuxwright';

const app = await launch({
  command: ['bash', '-lc', 'echo hello; sleep 5'],
  traceDir: 'tmp/traces/hello',
});

await app.waitForText('hello');
await app.expectText('hello');
console.log(await app.snapshot());
console.log(await app.trace());
await app.close();
```

The runner provides the same surface through a tiny fixture:

```js
import { test, expect } from 'tmuxwright-test';

test('prints hello', async ({ tmw }) => {
  const app = await tmw.launch({
    command: ['bash', '-lc', 'echo hello; sleep 5'],
  });
  await expect(app).toContain('hello');
});
```

Run compiled `*.tmw.js` / `*.tmw.mjs` tests with:

```sh
tmuxwright-test path/to/tests
```

During local development the SDK resolves `target/debug/tmuxwright-engine`
by default. Set `TMUXWRIGHT_ENGINE_BIN=/path/to/tmuxwright-engine` to use a
specific binary.

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

## Architecture

- `tmuxwright-tmux` owns the tmux server/socket/session lifecycle, input
  injection, capture, geometry, resize, and reconnect metadata.
- `tmuxwright-term` turns captured terminal state into a deterministic grid,
  hashes, locators, and stability checks.
- `tmuxwright-engine` is the product boundary: a local JSON-RPC daemon that
  combines tmux control, terminal parsing, waits, assertions, traces, and
  cleanup.
- `packages/tmuxwright` is the TypeScript SDK that spawns the daemon and
  exposes sessions to user tests.
- `packages/tmuxwright-test` is the minimal serial runner that discovers
  `*.tmw.js` / `*.tmw.mjs` files and preserves failing sessions.

Future or experimental surfaces such as `tmuxwright-napi`,
`tmuxwright-adapter-ratatui`, `create-tmuxwright`, rich reporters, the HTML
trace viewer, and framework adapters are intentionally deferred until
terminal-mode v1 is coherent.

## Validation

Local validation mirrors CI:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
```

## Contributing

This project follows a strict working discipline:

- **Manual validation before automated tests.** See [`CONTRIBUTING.md`](./CONTRIBUTING.md)
  and [`tmp/README.md`](./tmp/README.md).
- **Small, byte-sized, conventional commits.** See [`COMMITS.md`](./COMMITS.md).

## License

See [`LICENSE`](./LICENSE).
