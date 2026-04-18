// Tmuxwright standalone test runner entry point.
//
// Exports the user-facing `test` / `expect` primitives that tests import.
// See plan.md workstream G1 for the full runner design: config discovery,
// parallelism, retries, reporters, and the HTML trace viewer (G3).

export const VERSION = '0.0.0';

export { test, expect, runAll, getRegisteredTests, clearRegistry } from './runner.js';
export type { Tmw, ExpectHandle } from './runner.js';
