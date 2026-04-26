import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '..', '..', '..');
const engineBin = path.join(repoRoot, 'target', 'debug', 'tmuxwright-engine');
const cliBin = path.join(repoRoot, 'packages', 'tmuxwright-test', 'dist', 'cli.js');
const runnerIndex = path
  .join(repoRoot, 'packages', 'tmuxwright-test', 'dist', 'index.js')
  .replaceAll(path.sep, '/');

test('tmuxwright-test CLI runs a real tmux-backed test file', async () => {
  assert.equal(spawnSync('tmux', ['-V']).status, 0, 'tmux must be available');
  const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'tmuxwright-cli-e2e-'));
  const spec = path.join(tmp, 'plain.tmw.mjs');
  await fs.writeFile(
    spec,
    [
      `import { test, expect } from 'file://${runnerIndex}';`,
      '',
      "test('plain terminal output', async ({ tmw }) => {",
      "  const app = await tmw.launch({ command: ['bash', '-lc', 'echo cli-e2e-ok; sleep 5'] });",
      "  await expect(app).toContain('cli-e2e-ok');",
      '});',
      '',
    ].join('\n'),
  );

  const result = spawnSync(process.execPath, [cliBin, tmp], {
    cwd: repoRoot,
    env: { ...process.env, TMUXWRIGHT_ENGINE_BIN: engineBin },
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /plain terminal output/);
  assert.match(result.stdout, /1 passed, 0 failed/);

  await fs.rm(tmp, { recursive: true, force: true });
});
