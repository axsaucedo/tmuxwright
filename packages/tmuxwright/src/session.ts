// High-level session API. Wraps one engine-managed tmux session and
// mirrors a subset of the Playwright-style surface the plan calls for.

import { EngineClient, JsonValue } from './engine.js';

export interface LaunchOptions {
  command: string[];
  width?: number;
  height?: number;
  traceDir?: string;
  engine?: EngineClient;
}

export interface SnapshotResult {
  text: string;
  hash: string;
  width: number;
  height: number;
}

export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AssertTextResult {
  matched: boolean;
  region?: Region;
}

export interface WaitStableOptions {
  quietMs?: number;
  timeoutMs?: number;
}

export interface WaitStableResult {
  status: 'stable' | 'timeout';
  hash: string;
}

export interface WaitTextOptions {
  timeoutMs?: number;
}

export interface WaitTextResult {
  status: 'found' | 'timeout';
  matched: boolean;
  hash: string;
  region?: Region;
}

export interface WaitHashOptions {
  timeoutMs?: number;
}

export interface WaitHashResult {
  status: 'found' | 'timeout';
  hash: string;
}

export interface TraceResult {
  traceDir?: string;
  tracePath?: string;
}

export class TmuxwrightError extends Error {
  constructor(
    message: string,
    public readonly reconnect?: string,
  ) {
    super(message);
    this.name = 'TmuxwrightError';
  }
}

export class Session {
  private closed = false;
  private readonly ownsEngine: boolean;

  constructor(
    public readonly sessionId: string,
    public readonly socket: string,
    public readonly paneId: string,
    public readonly reconnect: string,
    public readonly traceDir: string | undefined,
    private readonly engine: EngineClient,
    ownsEngine: boolean,
  ) {
    this.ownsEngine = ownsEngine;
  }

  async sendKeys(keys: string[]): Promise<void> {
    await this.engine.call('engine.send_keys', { session_id: this.sessionId, keys });
  }

  async type(text: string): Promise<void> {
    await this.engine.call('engine.type', { session_id: this.sessionId, text });
  }

  async snapshot(withScrollback = false): Promise<SnapshotResult> {
    return this.engine.call<SnapshotResult>('engine.snapshot', {
      session_id: this.sessionId,
      with_scrollback: withScrollback,
    });
  }

  async waitForStable(opts: WaitStableOptions = {}): Promise<WaitStableResult> {
    return this.engine.call<WaitStableResult>('engine.wait_stable', {
      session_id: this.sessionId,
      quiet_ms: opts.quietMs ?? 250,
      timeout_ms: opts.timeoutMs ?? 5_000,
    });
  }

  async waitForText(contains: string, opts: WaitTextOptions = {}): Promise<WaitTextResult> {
    const raw = await this.engine.call<{
      status: 'found' | 'timeout';
      matched: boolean;
      hash: string;
      region?: [number, number, number, number];
    }>('engine.wait_text', {
      session_id: this.sessionId,
      contains,
      timeout_ms: opts.timeoutMs ?? 5_000,
    });
    if (raw.matched && raw.region) {
      const [x, y, width, height] = raw.region;
      return { status: raw.status, matched: true, hash: raw.hash, region: { x, y, width, height } };
    }
    return { status: raw.status, matched: raw.matched, hash: raw.hash };
  }

  async waitForHash(hash: string, opts: WaitHashOptions = {}): Promise<WaitHashResult> {
    return this.engine.call<WaitHashResult>('engine.wait_hash', {
      session_id: this.sessionId,
      hash,
      timeout_ms: opts.timeoutMs ?? 5_000,
    });
  }

  async assertText(contains: string): Promise<AssertTextResult> {
    const raw = await this.engine.call<{
      matched: boolean;
      region?: [number, number, number, number];
    }>('engine.assert_text', { session_id: this.sessionId, contains });
    if (raw.matched && raw.region) {
      const [x, y, width, height] = raw.region;
      return { matched: true, region: { x, y, width, height } };
    }
    return { matched: raw.matched };
  }

  async expectText(contains: string, opts: WaitTextOptions = {}): Promise<Region> {
    const r = await this.waitForText(contains, opts);
    if (!r.matched) {
      const snap = await this.snapshot();
      const preservation = await this.preserve();
      throw new TmuxwrightError(
        `expected terminal to contain ${JSON.stringify(contains)}\n--- visible ---\n${snap.text}`,
        preservation,
      );
    }
    return r.region!;
  }

  async preserve(): Promise<string> {
    const r = await this.engine.call<{ reconnect: string }>('engine.preserve', {
      session_id: this.sessionId,
    });
    return r.reconnect;
  }

  async trace(): Promise<TraceResult> {
    const r = await this.engine.call<{ trace_dir?: string; trace_path?: string }>('engine.trace', {
      session_id: this.sessionId,
    });
    return { traceDir: r.trace_dir, tracePath: r.trace_path };
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await this.engine.call('engine.close', { session_id: this.sessionId });
    if (this.ownsEngine) {
      await this.engine.shutdown();
    }
  }
}

export async function launch(opts: LaunchOptions): Promise<Session> {
  const ownsEngine = opts.engine === undefined;
  const engine = opts.engine ?? new EngineClient();
  // Handshake so we fail fast if the binary is missing / wrong protocol.
  const h = await engine.call<{ protocol: string }>('engine.handshake');
  if (h.protocol !== '1') {
    throw new TmuxwrightError(`unexpected engine protocol: ${h.protocol}`);
  }
  const res = await engine.call<{
    session_id: string;
    socket: string;
    pane_id: string;
    reconnect: string;
    trace_dir?: string;
  }>('engine.launch', {
    command: opts.command,
    width: opts.width ?? 120,
    height: opts.height ?? 40,
    trace_dir: opts.traceDir ?? null,
  });
  return new Session(
    res.session_id,
    res.socket,
    res.pane_id,
    res.reconnect,
    res.trace_dir,
    engine,
    ownsEngine,
  );
}

export type { JsonValue };
