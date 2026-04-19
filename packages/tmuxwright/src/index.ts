// Tmuxwright TypeScript SDK.
//
// Public surface:
//   launch({ command, width?, height? }) -> Session
//   Session.sendKeys / type / snapshot / waitForStable / assertText /
//           expectText / preserve / close
//
// Under the hood: spawns the tmuxwright-engine binary, speaks JSON-RPC
// 2.0 over LSP-framed stdio.

export const VERSION = '0.0.0';

export { EngineClient } from './engine.js';
export type { JsonValue } from './engine.js';
export { launch, Session, TmuxwrightError } from './session.js';
export type {
  LaunchOptions,
  SnapshotResult,
  WaitStableOptions,
  WaitStableResult,
  AssertTextResult,
  Region,
} from './session.js';
