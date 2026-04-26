// Minimal Playwright-shaped test runner. One engine process per file,
// fresh session per test, no parallelism — a deliberate v1 floor.

import os from 'node:os';
import path from 'node:path';

import { EngineClient } from 'tmuxwright';
import { launch, Session, LaunchOptions } from 'tmuxwright';

export interface Tmw {
  launch(opts: Omit<LaunchOptions, 'engine'>): Promise<Session>;
}

type TestFn = (ctx: { tmw: Tmw }) => Promise<void> | void;

interface Registered {
  name: string;
  fn: TestFn;
  timeoutMs: number;
}

const registry: Registered[] = [];

export function test(name: string, fn: TestFn, opts: { timeoutMs?: number } = {}): void {
  registry.push({ name, fn, timeoutMs: opts.timeoutMs ?? 30_000 });
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

export interface RunResult {
  name: string;
  ok: boolean;
  error?: Error;
  reconnect?: string;
  tracePath?: string;
  durationMs: number;
}

function safeName(name: string): string {
  return name.replace(/[^a-zA-Z0-9_.-]+/g, '-').replace(/^-|-$/g, '') || 'test';
}

function withTimeout<T>(name: string, timeoutMs: number, work: Promise<T>): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`test timed out after ${timeoutMs}ms: ${name}`));
    }, timeoutMs);
  });
  return Promise.race([work, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}

export async function runAll(): Promise<RunResult[]> {
  const engine = new EngineClient();
  const results: RunResult[] = [];
  const created: Session[] = [];
  let currentTraceDir = '';
  const tmw: Tmw = {
    async launch(opts) {
      const s = await launch({ ...opts, traceDir: opts.traceDir ?? currentTraceDir, engine });
      created.push(s);
      return s;
    },
  };
  try {
    for (const t of registry) {
      const start = Date.now();
      let err: Error | undefined;
      let reconnect: string | undefined;
      let tracePath: string | undefined;
      let preserved: Session | undefined;
      currentTraceDir = path.join(os.tmpdir(), 'tmuxwright-traces', `${safeName(t.name)}-${start}`);
      const sessionsBefore = created.length;
      try {
        await withTimeout(t.name, t.timeoutMs, Promise.resolve(t.fn({ tmw })));
      } catch (e) {
        err = e as Error;
        // Preserve last session created in this test for post-mortem.
        const last = created[created.length - 1];
        if (last) {
          try {
            reconnect = await last.preserve();
            preserved = last;
            tracePath = (await last.trace()).tracePath;
          } catch {
            /* ignore */
          }
        }
      } finally {
        // Close every non-preserved session created during this test.
        for (let i = created.length - 1; i >= sessionsBefore; i--) {
          if (created[i] === preserved) continue;
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
        tracePath,
        durationMs: Date.now() - start,
      });
    }
  } finally {
    await engine.shutdown();
  }
  return results;
}
