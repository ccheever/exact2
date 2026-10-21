// Stage 13 B on the wire (LLP 1121 D14): a paired client imports a Pencil
// `.pen` file (`import {kind: "pen"}`), planned in its own process as every
// import is, and held with its review: the axes, the tokens, the notes on
// the kinds it chose, the unmapped report. A literal over a token that
// followed another is marked as a pin, against the design as it is (no
// `.pen` export to compare with).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

const DESIGN = `tamber t

dials
  accent: color = #1083fe

theme
  accent.fill = accent
  pen.primary = accent
`;

const PEN = JSON.stringify({
  version: '2.21',
  themes: { Mode: ['Light', 'Dark'], density: ['roomy', 'tight'] },
  variables: {
    '--primary': { type: 'color', value: '#ff0000' },
    surface: { type: 'color', value: [{ value: '#ffffff' }, { value: '#000000', theme: { Mode: 'Dark' } }] },
    gap: { type: 'number', value: [{ value: 12 }, { value: 8, theme: { density: 'tight' } }] },
    flag: { type: 'boolean', value: true },
  },
  children: [{ type: 'frame', fill: '$surface', gap: '$gap', children: [{ type: 'text', fill: '$--primary' }] }],
});

let dir, server, tamber, origin, local, flags = null;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-pen-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a" color="pen.primary"\n');
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 100 });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), flags ?? gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  local = (await (await fetch(`${origin}/__tamber/token`)).json()).token;
  await tamber.ready;
}, 600_000);
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const post = async (request, token = local, path = '/__tamber') => {
  const res = await fetch(`${origin}${path}`, { method: 'POST', headers: { 'content-type': 'application/json', ...(token ? { 'x-exact-tamber-token': token } : {}) }, body: JSON.stringify(request) });
  const text = await res.text();
  let body = {};
  try { body = JSON.parse(text); } catch {}
  return { status: res.status, ...body };
};
const paired = async scope => {
  const { code } = (await post({ method: 'pair-code', scope, name: 'pencil' })).ok;
  flags = { allowed: true, local: false, remote: true, peer: '203.0.113.7' };
  try { return (await post({ code }, null, '/__tamber/pair')).ok.token; } finally { flags = null; }
};

test('a .pen file imports through the wire: the axes and tokens proposed, the notes and the report in its review, a literal over a followed token pinned', async () => {
  const token = await paired('propose');
  const got = await post({ method: 'import', client: 'pencil', kind: 'pen', name: 'design.pen', source: PEN }, token);
  expect(got.status).toBe(200);
  const review = got.ok.review;
  expect(review.importer).toBe('pen');
  expect(review.counted).toEqual({ source: 6, mapped: 5, reported: 1 });
  expect(review.unmapped.map(u => [u.name, u.why])).toEqual([['flag', 'no Tamber kind: a string or a boolean']]);
  expect(review.notes).toEqual(["the file's frames and components are not imported (its variables and themes are)", '`gap` is a length (px): Pencil reads it as gap']);
  // `pen.primary` followed the dial; the file's literal over it is a pin, marked with what it followed.
  expect(review.pins).toEqual([{ token: 'pen.primary', when: [], follows: ['accent'], items: expect.any(Array) }]);
  const held = (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals.find(p => p.proposal === got.ok.proposal);
  const ops = held.ops.map(o => `${o.op.op} ${o.op.name}`);
  expect(ops).toContain('add-axis density');
  expect(ops).toContain('set-token pen.primary');
  expect(ops).toContain('add-token pen.surface');
  expect(ops).toContain('add-token pen.gap');
}, 120_000);

test('a .pen of another version is refused with its reason', async () => {
  const token = await paired('propose');
  const got = await post({ method: 'import', client: 'pencil', kind: 'pen', name: 'old.pen', source: JSON.stringify({ version: '1.0', children: [] }) }, token);
  expect(got.status).not.toBe(200);
  expect(JSON.stringify(got.error)).toContain('a `.pen` of version 1.0: this importer reads 2.x');
}, 60_000);

test('round 1: a .pen that multiplies (a long name over many entries) is refused by the import\'s real budget in its own process', async () => {
  const token = await paired('propose');
  const entries = Array(16_384).fill({ value: '#000000' });
  const source = JSON.stringify({ version: '2.21', variables: { ['n'.repeat(64 * 1024)]: { type: 'color', value: entries } } });
  const got = await post({ method: 'import', client: 'pencil', kind: 'pen', name: 'big.pen', source }, token);
  expect(got.status).not.toBe(200);
  expect(JSON.stringify(got.error)).toContain('expands past what one import may do');
}, 120_000);
