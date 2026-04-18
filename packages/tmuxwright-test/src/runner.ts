// Minimal Playwright-shaped test runner. One engine process per file,
// fresh session per test, no parallelism — a deliberate v1 floor.

import { EngineClient } from 'tmuxwright';
import { launch, Session, LaunchOptions } from 'tmuxwright';

export interface Tmw {
  launch(opts: Omit<LaunchOptions, 'engine'>): Promise<Session>;
}

type TestFn = (ctx: { tmw: Tmw }) => Promise<void> | void;

interface Registered {
  name: string;
  fn: TestFn;
}

const registry: Registered[] = [];

export function test(name: string, fn: TestFn): void {
  registry.push({ name, fn });
}

export function getRegisteredTests(): readonly Registered[] {
  return registry;
}

export function clearRegistry(): void {
  registry.length = 0;
}

export interface ExpectHandle {
  toContain(fragment: string): Promise<void>;
  toBeStable(opts?: { quietMs?: number; timeoutMs?: number }): Promise<void>;
}

export function expect(session: Session): ExpectHandle {
  return {
    async toContain(fragment: string) {
      await session.expectText(fragment);
    },
    async toBeStable(opts) {
      const r = await session.waitForStable(opts);
      if (r.status !== 'stable') {
        throw new Error(`expected session to stabilize, got ${r.status}`);
      }
    },
  };
}

interface RunResult {
  name: string;
  ok: boolean;
  error?: Error;
  reconnect?: string;
  durationMs: number;
}

export async function runAll(): Promise<RunResult[]> {
  const engine = new EngineClient();
  const results: RunResult[] = [];
  const created: Session[] = [];
  const tmw: Tmw = {
    async launch(opts) {
      const s = await launch({ ...opts, engine });
      created.push(s);
      return s;
    },
  };
  try {
    for (const t of registry) {
      const start = Date.now();
      let err: Error | undefined;
      let reconnect: string | undefined;
      const sessionsBefore = created.length;
      try {
        await t.fn({ tmw });
      } catch (e) {
        err = e as Error;
        // Preserve last session created in this test for post-mortem.
        const last = created[created.length - 1];
        if (last) {
          try {
            reconnect = await last.preserve();
          } catch {
            /* ignore */
          }
        }
      } finally {
        // Close every session created during this test (pass or fail).
        for (let i = created.length - 1; i >= sessionsBefore; i--) {
          try {
            await created[i]!.close();
          } catch {
            /* ignore */
          }
        }
        created.length = sessionsBefore;
      }
      results.push({
        name: t.name,
        ok: err === undefined,
        error: err,
        reconnect,
        durationMs: Date.now() - start,
      });
    }
  } finally {
    await engine.shutdown();
  }
  return results;
}
