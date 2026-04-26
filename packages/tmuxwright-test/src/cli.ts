#!/usr/bin/env node
// Tmuxwright test runner CLI.
//
// Discovers `*.tmw.mjs` / `*.tmw.js` files under the given path (or cwd),
// imports them (which registers tests via `test(...)`), runs them all
// against a single engine process, and prints a line reporter.

import path from 'node:path';
import fs from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { runAll, clearRegistry } from './runner.js';

async function* walk(dir: string): AsyncGenerator<string> {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  for (const e of entries) {
    if (e.name.startsWith('.') || e.name === 'node_modules') continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else if (/\.tmw\.(mjs|js)$/.test(e.name)) yield p;
  }
}

async function main(): Promise<void> {
  const root = path.resolve(process.argv[2] ?? '.');
  const files: string[] = [];
  const stat = await fs.stat(root).catch(() => null);
  if (!stat) {
    console.error(`tmuxwright-test: path not found: ${root}`);
    process.exit(2);
  }
  if (stat.isFile()) files.push(root);
  else for await (const f of walk(root)) files.push(f);

  if (files.length === 0) {
    console.error('tmuxwright-test: no *.tmw.{mjs,js} files found');
    process.exit(2);
  }

  let passed = 0;
  let failed = 0;
  for (const file of files) {
    clearRegistry();
    console.log(`\n# ${path.relative(process.cwd(), file)}`);
    await import(pathToFileURL(file).href);
    const results = await runAll();
    for (const r of results) {
      if (r.ok) {
        passed++;
        console.log(`  ✓ ${r.name} (${r.durationMs}ms)`);
      } else {
        failed++;
        console.log(`  ✗ ${r.name} (${r.durationMs}ms)`);
        if (r.error) console.log(`      ${r.error.message.split('\n').join('\n      ')}`);
        if (r.reconnect) console.log(`      reconnect: ${r.reconnect}`);
        if (r.tracePath) console.log(`      trace: ${r.tracePath}`);
      }
    }
  }
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
