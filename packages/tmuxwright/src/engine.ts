// JSON-RPC 2.0 client over framed stdio to the `tmuxwright-engine`
// binary. Requests are serialized through a small promise queue because
// the engine's synchronous serve loop handles one request/response at a time.

import { ChildProcess, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export type JsonValue = null | boolean | number | string | JsonValue[] | { [k: string]: JsonValue };

interface Pending {
  resolve: (v: JsonValue) => void;
  reject: (e: Error) => void;
  id: number;
  method: string;
}

export class EngineRpcError extends Error {
  constructor(
    public readonly method: string,
    public readonly code: number,
    message: string,
  ) {
    super(`engine rpc error ${code} from ${method}: ${message}`);
    this.name = 'EngineRpcError';
  }
}

export class EngineClient {
  private proc: ChildProcess;
  private nextId = 1;
  private pending: Pending | null = null;
  private rxBuffer = Buffer.alloc(0);
  private closed = false;
  private closeError: Error | null = null;
  private queue: Promise<void> = Promise.resolve();

  constructor(binary?: string) {
    const bin = binary ?? EngineClient.resolveBinary();
    if (!fs.existsSync(bin)) {
      throw new Error(
        `tmuxwright engine binary not found: ${bin}. Build it with "cargo build -p tmuxwright-engine" or set TMUXWRIGHT_ENGINE_BIN.`,
      );
    }
    this.proc = spawn(bin, [], {
      stdio: ['pipe', 'pipe', 'inherit'],
    });
    this.proc.stdout!.on('data', (chunk: Buffer) => this.onData(chunk));
    this.proc.on('error', (err) => {
      this.rejectPending(err);
      this.closeError = err;
      this.closed = true;
    });
    this.proc.on('exit', () => {
      this.closed = true;
      this.rejectPending(new Error('engine exited before responding'));
    });
  }

  private static resolveBinary(): string {
    const fromEnv = process.env.TMUXWRIGHT_ENGINE_BIN ?? process.env.TMUXWRIGHT_ENGINE;
    if (fromEnv) return fromEnv;
    // Repo layout: packages/tmuxwright/dist/engine.js -> workspace root -> target/debug/tmuxwright-engine
    const repoRoot = path.resolve(__dirname, '..', '..', '..');
    return path.join(repoRoot, 'target', 'debug', 'tmuxwright-engine');
  }

  private rejectPending(err: Error): void {
    if (!this.pending) return;
    const p = this.pending;
    this.pending = null;
    p.reject(err);
  }

  private onData(chunk: Buffer): void {
    this.rxBuffer = Buffer.concat([this.rxBuffer, chunk]);
    // Try to extract as many framed messages as are present.
    for (;;) {
      const headerEnd = this.rxBuffer.indexOf('\r\n\r\n');
      if (headerEnd < 0) return;
      const header = this.rxBuffer.subarray(0, headerEnd).toString('utf8');
      const match = /Content-Length:\s*(\d+)/i.exec(header);
      if (!match) {
        const err = new Error(`malformed header: ${header}`);
        this.rejectPending(err);
        this.rxBuffer = Buffer.alloc(0);
        return;
      }
      const len = Number(match[1]);
      const bodyStart = headerEnd + 4;
      if (this.rxBuffer.length < bodyStart + len) return;
      const body = this.rxBuffer.subarray(bodyStart, bodyStart + len).toString('utf8');
      this.rxBuffer = this.rxBuffer.subarray(bodyStart + len);
      this.dispatchResponse(body);
    }
  }

  private dispatchResponse(body: string): void {
    if (!this.pending) return;
    const pending = this.pending;
    this.pending = null;
    let parsed: { id?: number; result?: JsonValue; error?: { code: number; message: string } };
    try {
      parsed = JSON.parse(body);
    } catch (err) {
      pending.reject(new Error(`bad json from engine: ${(err as Error).message}`));
      return;
    }
    if (parsed.id !== pending.id) {
      pending.reject(new Error(`id mismatch: sent ${pending.id} got ${parsed.id}`));
      return;
    }
    if (parsed.error) {
      pending.reject(new EngineRpcError(pending.method, parsed.error.code, parsed.error.message));
      return;
    }
    pending.resolve(parsed.result ?? null);
  }

  private async callNow<T = JsonValue>(method: string, params: JsonValue = {}): Promise<T> {
    if (this.closed) throw this.closeError ?? new Error('engine closed');
    const id = this.nextId++;
    const body = JSON.stringify({ jsonrpc: '2.0', method, params, id });
    return new Promise<T>((resolve, reject) => {
      this.pending = {
        id,
        method,
        resolve: (v) => resolve(v as T),
        reject,
      };
      this.proc.stdin!.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
    });
  }

  async call<T = JsonValue>(method: string, params: JsonValue = {}): Promise<T> {
    const run = () => this.callNow<T>(method, params);
    const next = this.queue.then(run, run);
    this.queue = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  async shutdown(): Promise<void> {
    if (this.closed) return;
    try {
      await this.call('engine.shutdown');
    } catch (err) {
      if (!this.closed) throw err;
    }
    await new Promise<void>((resolve) => {
      if (this.closed) return resolve();
      this.proc.once('exit', () => resolve());
      // Force-kill after a grace period.
      setTimeout(() => {
        if (!this.closed) this.proc.kill('SIGKILL');
      }, 2_000).unref();
    });
  }
}
