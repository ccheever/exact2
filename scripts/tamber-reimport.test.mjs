// Stage 10b on the wire (LLP 1121 D14; §5 10b's gate): a client at propose
// scope pulls an export, edits it as a tool would (a Figma plugin), and
// imports it with the baseline it pulled. A changed accent is proposed as the
// dial's change, every token it moves listed; a changed folded value as a
// pinned override, marked; a baseline this server never gave out is refused.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

const DESIGN = `tamber t

axes
  dark-style = black | dim  under scheme=dark

dials
  accent: color = #1083fe
  lift: number = 0.1  in 0..0.5

theme
  surface.page = light-dark(#ffffff, #000000)
  accent.fill = accent
  accent.soft = oklch(from accent calc(l + lift) c h)
  surface.raised = oklch(from #3366cc calc(l + lift) c h)
  surface.sunk = oklch(from #224488 calc(l - lift) c h)
`;

let dir, server, tamber, origin, local, flags = null, peers = 0;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-reimport-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a" color="accent.fill"\n');
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
}, 600_000); // a cold checkout builds the compiler first
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
/** A paired client at `scope`, as a remote tool pairs. */
const paired = async scope => {
  const { code } = (await post({ method: 'pair-code', scope, name: 'figma' })).ok;
  flags = { local: false, peer: `203.0.113.${++peers}` };
  try { return (await post({ code }, null, '/__tamber/pair')).ok.token; } finally { flags = null; }
};
/** The export's tree for each context, edited by `f`, as text. */
const edited = (text, f) => {
  const doc = JSON.parse(text);
  for (const [name, c] of Object.entries(doc.modifiers.mode.contexts)) f(name, c[0]);
  return JSON.stringify(doc);
};
const RED = { colorSpace: 'srgb', components: [1, 0, 0], hex: '#ff0000' };

test('a propose client\'s changed accent is the dial\'s change, every token it moves listed; a changed folded value is a marked pin', async () => {
  const token = await paired('propose');
  const pulled = await post({ method: 'export', kind: 'dtcg' }, token);
  expect(pulled.status).toBe(200);
  expect(typeof pulled.ok.digest).toBe('string');
  expect(pulled.ok.baseline).toBe(null); // below write, none is kept
  // Someone else writes meanwhile: the client's baseline is still the one it pulled.
  const files = (await post({ method: 'hello', client: 'person' })).ok.files;
  const wrote = await post({ method: 'transaction', client: 'person', files, ops: [{ op: 'set-token', name: 'surface.page', value: 'light-dark(#fafafa, #000000)' }] });
  expect(wrote.status).toBe(200);
  const accent = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(pulled.ok.text, (_, t) => { t.accent.fill.$value = RED; }), baseline: pulled.ok.text }, token);
  expect(accent.status).toBe(200);
  const review = accent.ok.review;
  expect(review.dials.map(d => [d.dial, d.value, d.from, d.items])).toEqual([['accent', '#ff0000', ['accent.fill'], [0]]]);
  expect(new Set(review.dials[0].impact.map(i => i.token))).toEqual(new Set(['accent.fill', 'accent.soft']));
  expect(review.pins).toBeUndefined();
  const held = (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals.find(p => p.proposal === accent.ok.proposal);
  // Planned against the design as it is: the other client's edit is not reverted.
  expect(held.ops.map(o => o.op)).toEqual([{ op: 'set-dial', name: 'accent', value: '#ff0000' }]);
  // A changed folded value: a literal over the expression, marked with what it followed.
  const soft = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(pulled.ok.text, (_, t) => { t.accent.soft.$value = RED; }), baseline: pulled.ok.text }, token);
  expect(soft.status).toBe(200);
  expect(soft.ok.review.dials).toBeUndefined();
  expect(soft.ok.review.pins).toEqual([{ token: 'accent.soft', when: [], follows: ['accent', 'lift'], items: [0] }]);
}, 120_000);

test('a baseline this server never gave out, nor keeps, nor is the design\'s own, is refused: pull first', async () => {
  const token = await paired('propose');
  const pulled = (await post({ method: 'export', kind: 'dtcg' }, token)).ok.text;
  const forged = JSON.parse(pulled);
  const stamp = JSON.stringify(forged).match(/"digest":"([0-9a-f]+)"/)[1];
  const text = JSON.stringify(forged).replace(stamp, 'f'.repeat(stamp.length));
  const refused = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(text, (_, t) => { t.accent.fill.$value = RED; }), baseline: text }, token);
  expect(refused.status).toBe(409);
  expect(refused.error.message).toMatch(/^pull first/);
  const typed = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: pulled, baseline: 7 }, token);
  expect(typed.status).toBe(400);
}, 120_000);

test('a file whose header a tool dropped reads against the baseline the client names', async () => {
  const token = await paired('propose');
  const pulled = (await post({ method: 'export', kind: 'dtcg' }, token)).ok.text;
  // Another client writes, so the design's own export differs from the one pulled.
  const files = (await post({ method: 'hello', client: 'person' })).ok.files;
  expect((await post({ method: 'transaction', client: 'person', files, ops: [{ op: 'set-token', name: 'surface.raised', value: '#123456' }] })).status).toBe(200);
  const doc = JSON.parse(edited(pulled, (_, t) => { t.accent.fill.$value = RED; }));
  const strip = v => { if (v && typeof v === 'object') { delete v.$extensions; for (const k of Object.keys(v)) strip(v[k]); } };
  strip(doc);
  const got = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: JSON.stringify(doc), baseline: pulled }, token);
  expect(got.status).toBe(200);
  expect(got.ok.review.dials?.[0]?.dial).toBe('accent');
  // A baseline for a source that is no Tamber export: 400.
  const css = await post({ method: 'import', client: 'figma', kind: 'shadcn', source: ':root { --primary: #ff0000; }', baseline: pulled }, token);
  expect(css.status).toBe(400);
}, 120_000);

test('a baseline with the design\'s digest but contexts mapped otherwise is stale: 409, pull first', async () => {
  const token = await paired('propose');
  const pulled = (await post({ method: 'export', kind: 'dtcg' }, token)).ok.text;
  const forged = JSON.parse(pulled);
  const contexts = forged.sets.exact.sources[0].$extensions.exact.contexts, [a, b] = Object.keys(contexts);
  [contexts[a], contexts[b]] = [contexts[b], contexts[a]];
  const text = JSON.stringify(forged);
  const refused = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(text, (_, t) => { t.accent.fill.$value = RED; }), baseline: text }, token);
  expect([refused.status, refused.error?.stale]).toEqual([409, true]);
  expect(refused.error.message).toMatch(/^pull first/);
}, 120_000);

test('some items accepted: the dials and pins kept are marked again on the design as it is; a review\'s marks are shaped', async () => {
  const token = await paired('propose');
  const pulled = (await post({ method: 'export', kind: 'dtcg' }, token)).ok.text;
  const BLUE = { colorSpace: 'srgb', components: [0, 0, 1], hex: '#0000ff' };
  const both = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(pulled, (_, t) => { t.accent.fill.$value = RED; t.accent.soft.$value = BLUE; }), baseline: pulled }, token);
  expect(both.status).toBe(200);
  const { dials, pins } = both.ok.review;
  expect([dials?.length, pins?.map(p => p.token)]).toEqual([1, ['accent.soft']]);
  // The dial alone: held again, its impact read afresh, the pin gone with its operation.
  const some = await post({ method: 'accept', client: 'person', proposal: both.ok.proposal, items: dials[0].items });
  expect(some.ok?.recomputed).toBe(true);
  const held = (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals.find(p => p.proposal === some.ok.proposal);
  expect(held.review.pins).toBeUndefined();
  expect(held.review.dials.map(d => [d.dial, d.from, d.items])).toEqual([['accent', ['accent.fill'], [0]]]);
  expect(new Set(held.review.dials[0].impact.map(i => i.token))).toEqual(new Set(['accent.fill', 'accent.soft']));
  // A review's dials and pins carry indices, as its offers do.
  const files = (await post({ method: 'hello', client: 'person' })).ok.files;
  const bad = await post({ method: 'propose', hold: true, client: 'person', files, ops: [{ op: 'set-dial', name: 'accent', value: '#00ff00' }], review: { dials: [{ items: ['x'] }] } });
  expect(bad.status).toBe(400);
}, 120_000);

test('round 3: a re-import with no marks, recomputed after the design moved, is marked again on the design as it is', async () => {
  const token = await paired('propose');
  const pulled = (await post({ method: 'export', kind: 'dtcg' }, token)).ok.text;
  const got = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(pulled, (_, t) => { t.surface.sunk.$value = RED; }), baseline: pulled }, token);
  expect(got.ok.review.pins?.map(p => p.token)).toEqual(['surface.sunk']); // it followed `lift`
  const plain = await post({ method: 'import', client: 'figma', kind: 'dtcg', source: edited(pulled, (_, t) => { t.surface.page.$value = RED; }), baseline: pulled }, token);
  expect(plain.ok.review.pins).toBeUndefined();
  // Meanwhile surface.page comes to follow the accent: the accept recomputes, and says so.
  const files = (await post({ method: 'hello', client: 'person' })).ok.files;
  expect((await post({ method: 'transaction', client: 'person', files, ops: [{ op: 'set-token', name: 'surface.page', value: 'light-dark(accent, #000000)' }] })).status).toBe(200);
  const again = await post({ method: 'accept', client: 'person', proposal: plain.ok.proposal });
  expect(again.ok?.recomputed).toBe(true);
  const held = (await post({ after: 0, wait: false }, local, '/__tamber/events')).proposals.find(p => p.proposal === again.ok.proposal);
  expect(held.review.pins?.map(p => [p.token, p.follows])).toEqual([['surface.page', ['accent']]]);
}, 120_000);
