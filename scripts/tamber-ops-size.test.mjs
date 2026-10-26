// A client's operations in one request (tamber/1.17, `tamber-ops-size`,
// scripts/tamber-ops-limit.mjs) against a held import past the limit: its
// accept and revert are the server's own; a client sending as many is
// refused (theme2's ruling: no re-hold exemption, after edit-held).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

let dir, server, tamber, origin, headers;
const gitIn = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir, encoding: 'utf8' });
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-ops-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  gitIn('init', '-q'); gitIn('add', '.'); gitIn('commit', '-q', '-m', 'design');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port, gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  server.on('request', async (req, res) => { if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const post = async request => {
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json()) };
};

test("past 1024 operations: a held import's accept and revert are the server's own; a client sending as many is refused", async () => {
  const n = 3000;
  const css = `@theme {\n${Array.from({ length: n }, (_, k) => `  --color-c${k}: #${(k + 0x100000).toString(16).slice(-6)};`).join('\n')}\n}\n`;
  const imported = await post({ method: 'import', kind: 'tailwind', client: 'big', source: css });
  expect(imported.status, JSON.stringify(imported).slice(0, 400)).toBe(200);
  const held = imported.ok;
  expect(held.ops.length).toBeGreaterThan(1024);
  // A client sending them is refused, whatever it names (a value edited in a
  // held proposal is edit-held's, which sends no operations).
  const ops = held.ops.map(o => o.op);
  const again = await post({ method: 'propose', hold: true, client: 'big', review: { ...held.review, from: held.proposal }, ops });
  expect([again.status, again.error?.id]).toEqual([400, 'tamber-ops-size']);
  // The held import accepted whole: the server applies its 3000 operations.
  const accepted = await post({ method: 'accept', client: 'person', proposal: held.proposal });
  expect(accepted.status, JSON.stringify(accepted).slice(0, 400)).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('tw.c2999 = ');
  // And reverted: the server holds the revert's operations, past 1024 too.
  gitIn('add', 'app.tamber');
  gitIn('commit', '-q', '-m', 'a large import');
  const reverted = await post({ method: 'revert', client: 'person', commit: gitIn('rev-parse', 'HEAD').stdout.trim() });
  expect(reverted.status, JSON.stringify(reverted).slice(0, 400)).toBe(200);
  expect(reverted.ok.ops.length).toBeGreaterThan(1024);
  gitIn('reset', '-q', '--hard', 'HEAD~1');
}, 300_000);
