// Writes stub/dials.json, the stub server's canned replies, from the stub's
// design (stub/dials.tamber) and app (stub/app.contract, its components'
// params) through a real `contract tamber serve`: `describe`, and `resolve`
// and `explain` for every token and param. Run after changing either:
//   bun apps/design-tool/stub/regen.mjs   (CONTRACT=<binary> for another build)
import { spawn } from 'node:child_process';
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = new URL('.', import.meta.url).pathname;
const dir = mkdtempSync(join(tmpdir(), 'design-tool-stub-'));
copyFileSync(resolve(here, 'dials.tamber'), join(dir, 'app.tamber'));
copyFileSync(resolve(here, 'app.contract'), join(dir, 'app.contract'));
const serve = spawn(process.env.CONTRACT ?? resolve(here, '../../../target/debug/contract'), ['tamber', 'serve', '--stdio', dir], { stdio: ['pipe', 'pipe', 'inherit'] });
const waiting = new Map();
let buf = '', id = 0;
serve.stdout.on('data', d => { buf += d; const lines = buf.split('\n'); buf = lines.pop(); for (const l of lines) { const m = JSON.parse(l); waiting.get(m.id)?.(m); } });
const ask = r => new Promise(ok => { waiting.set(++id, ok); serve.stdin.write(`${JSON.stringify({ id, ...r })}\n`); });
try {
  const described = await ask({ method: 'describe' });
  if (!described.ok) throw new Error(JSON.stringify(described.error));
  const ir = described.ok.ir;
  const names = [...ir.tokens.map(t => t.name), ...(ir.components ?? []).flatMap(c => c.params.map(p => p.name))];
  const resolved = {}, explained = {};
  for (const token of names) {
    const r = await ask({ method: 'resolve', token });
    if (!r.ok) throw new Error(`resolve ${token}: ${JSON.stringify(r.error)}`);
    resolved[token] = r.ok;
    const e = await ask({ method: 'explain', token });
    if (e.ok) explained[token] = e.ok;
  }
  writeFileSync(resolve(here, 'dials.json'), `${JSON.stringify({ describe: described.ok, resolve: resolved, explain: explained }, null, 1)}\n`);
} finally {
  serve.kill();
  rmSync(dir, { recursive: true, force: true });
}
