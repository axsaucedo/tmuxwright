// JSON-RPC 2.0 client over framed stdio to the `tmuxwright-engine`
// binary. Single-threaded: one in-flight request at a time, matching
// the engine's synchronous serve loop.

import { ChildProcess, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export type JsonValue = null | boolean | number | string | JsonValue[] | { [k: string]: JsonValue };

interface Pending {
  resolve: (v: JsonValue) => void;
  reject: (e: Error) => void;
  id: number;
}

export class EngineClient {
  private proc: ChildProcess;
  private nextId = 1;
  private pending: Pending | null = null;
  private rxBuffer = Buffer.alloc(0);
  private closed = false;

  constructor(binary?: string) {
    const bin = binary ?? EngineClient.resolveBinary();
    this.proc = spawn(bin, [], {
      stdio: ['pipe', 'pipe', 'inherit'],
    });
    this.proc.stdout!.on('data', (chunk: Buffer) => this.onData(chunk));
    this.proc.on('exit', () => {
      this.closed = true;
      if (this.pending) {
        const p = this.pending;
        this.pending = null;
        p.reject(new Error('engine exited before responding'));
      }
    });
  }

  private static resolveBinary(): string {
    const fromEnv = process.env.TMUXWRIGHT_ENGINE;
    if (fromEnv) return fromEnv;
    // Repo layout: packages/tmuxwright/dist/engine.js -> workspace root -> target/debug/tmuxwright-engine
    const repoRoot = path.resolve(__dirname, '..', '..', '..');
    return path.join(repoRoot, 'target', 'debug', 'tmuxwright-engine');
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
        if (this.pending) {
          this.pending.reject(err);
          this.pending = null;
        }
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
      pending.reject(new Error(`engine rpc error ${parsed.error.code}: ${parsed.error.message}`));
      return;
    }
    pending.resolve(parsed.result ?? null);
  }

  async call<T = JsonValue>(method: string, params: JsonValue = {}): Promise<T> {
    if (this.closed) throw new Error('engine closed');
    if (this.pending) throw new Error('a call is already in flight');
    const id = this.nextId++;
    const body = JSON.stringify({ jsonrpc: '2.0', method, params, id });
    return new Promise<T>((resolve, reject) => {
      this.pending = {
        id,
        resolve: (v) => resolve(v as T),
        reject,
      };
      this.proc.stdin!.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
    });
  }

  async shutdown(): Promise<void> {
    if (this.closed) return;
    try {
      await this.call('engine.shutdown');
    } catch {
      // ignore — shutdown is best-effort
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
