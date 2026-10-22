// A linked package's design on the local wire (LLP 1121 D11, stage 11 A2
// part 2, *1.14*): listed read-only by `hello` and `describe`, a change to
// it on disk announced as a hand edit is, and a held proposal planned on
// the old fold checked again when it is accepted.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

let dir, server, tamber, origin, local;
const pkg = () => join(dir, 'node_modules/@acme/reading/reading.tamber');
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-packages-'));
  mkdirSync(join(dir, 'node_modules/@acme/reading'), { recursive: true });
  writeFileSync(join(dir, 'node_modules/@acme/reading/package.json'), JSON.stringify({ name: '@acme/reading', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(dir, 'node_modules/@acme/reading/index.contract'), 'component ReadingCard\n  view\n    column background-color="reading.card"\n      text "x"\n');
  writeFileSync(pkg(), 'tamber reading\n\ntheme\n  reading.card = #112233\n  reading.ink = #222222\n');
  // A second package with no design yet.
  mkdirSync(join(dir, 'node_modules/@acme/notes'), { recursive: true });
  writeFileSync(join(dir, 'node_modules/@acme/notes/package.json'), JSON.stringify({ name: '@acme/notes', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(dir, 'node_modules/@acme/notes/index.contract'), 'component NotesCard\n  view\n    text "n"\n');
  writeFileSync(join(dir, 'app.contract'), 'use ReadingCard from "@acme/reading"\nuse NotesCard from "@acme/notes"\ncomponent App\n  view\n    column background-color="brand"\n      ReadingCard()\n      NotesCard()\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n  brand = #123456\n');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 100 });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
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
const post = async (request, path = '/__tamber') => {
  const res = await fetch(`${origin}${path}`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': local }, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json().catch(() => ({}))) };
};
const until = async (pass, ms = 20_000) => { const end = Date.now() + ms; for (;;) { const got = await pass(); if (got) return got; if (Date.now() > end) throw new Error('not seen'); await new Promise(ok => setTimeout(ok, 100)); } };

test('hello and describe list the package designs read-only; a change to one on disk is announced, and a held proposal on the old fold is checked again', async () => {
  const hello = await post({ method: 'hello' });
  expect(hello.ok.packages).toEqual([{ package: '@acme/reading', file: '@acme/reading/reading.tamber', digest: expect.any(String) }]);
  expect(hello.ok.packages[0].path).toBeUndefined();
  expect((await post({ method: 'describe' })).ok.packages[0].file).toBe('@acme/reading/reading.tamber');
  // Held: the app's brand as the package's reading.ink.
  const held = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'brand', value: 'reading.ink' }] });
  expect(held.ok?.proposal).toBeTruthy();
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  // The package drops reading.ink, on disk.
  writeFileSync(pkg(), 'tamber reading\n\ntheme\n  reading.card = #112233\n');
  const changed = await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/reading/reading.tamber')));
  expect(changed.client).toBe(null);
  expect(changed.packages[0].digest).not.toBe(hello.ok.packages[0].digest);
  // Accepted now, the proposal is planned on the package as it is: refused.
  const taken = await post({ method: 'accept', client: 'person', proposal: held.ok.proposal });
  expect(taken.ok).toBeUndefined();
  expect(JSON.stringify(taken)).toContain('reading.ink');
}, 120_000);

test('a package SVG its design reads, edited, is announced as a change of the design (stage 13)', async () => {
  const star = d => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="${d}"/></svg>`;
  mkdirSync(join(dir, 'node_modules/@acme/reading/assets'), { recursive: true });
  writeFileSync(join(dir, 'node_modules/@acme/reading/assets/star.svg'), star('M1 1h22v22H1Z'));
  let { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(pkg(), 'tamber reading\n\ntheme\n  reading.card = #112233\n\nicons\n  reading.star = icon(sf "star", svg "star.svg")\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/reading/reading.tamber')));
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  writeFileSync(join(dir, 'node_modules/@acme/reading/assets/star.svg'), star('M2 2h20v20H2Z'));
  const drawn = await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/reading/reading.tamber')));
  expect(drawn.client).toBe(null);
}, 120_000);

test('a package with no design whose SVG the build reads, edited, is announced (stage 13)', async () => {
  const star = d => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="${d}"/></svg>`;
  const notes = join(dir, 'node_modules/@acme/notes');
  mkdirSync(join(notes, 'assets'), { recursive: true });
  writeFileSync(join(notes, 'assets/pin.svg'), star('M1 1h22v22H1Z'));
  let { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(join(notes, 'index.contract'), 'component NotesCard\n  params\n    glyph: icon = "icon(svg \\"pin.svg\\")"\n  view\n    image "symbol:param.glyph"\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/assets/')));
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  writeFileSync(join(notes, 'assets/pin.svg'), star('M2 2h20v20H2Z'));
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/assets/')));
  writeFileSync(join(notes, 'index.contract'), 'component NotesCard\n  view\n    text "n"\n');
}, 120_000);

test('a package vendored in the app, reached by a relative use, has its SVG edit announced (stage 13)', async () => {
  const star = d => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="${d}"/></svg>`;
  const vendor = join(dir, 'vendor/shelf');
  mkdirSync(join(vendor, 'assets'), { recursive: true });
  writeFileSync(join(vendor, 'package.json'), JSON.stringify({ name: '@acme/shelf', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(vendor, 'index.contract'), 'component ShelfCard\n  params\n    glyph: icon = "icon(svg \\"pin.svg\\")"\n  view\n    image "symbol:param.glyph"\n');
  writeFileSync(join(vendor, 'assets/pin.svg'), star('M1 1h22v22H1Z'));
  const app = join(dir, 'app.contract');
  const before = readFileSync(app, 'utf8');
  let { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(app, `use ShelfCard from "./vendor/shelf/index.contract"\n${before.replace('      NotesCard()\n', '      NotesCard()\n      ShelfCard()\n')}`);
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/shelf/assets/')));
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  writeFileSync(join(vendor, 'assets/pin.svg'), star('M2 2h20v20H2Z'));
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/shelf/assets/')));
  writeFileSync(app, before);
}, 120_000);

test('a package vendored in the app whose SVG is not there yet has it announced when made (stage 13)', async () => {
  const star = d => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="${d}"/></svg>`;
  const vendor = join(dir, 'vendor/rack');
  mkdirSync(join(vendor, 'assets'), { recursive: true });
  writeFileSync(join(vendor, 'package.json'), JSON.stringify({ name: '@acme/rack', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(vendor, 'index.contract'), 'component RackCard\n  params\n    glyph: icon = "icon(svg \\"pin.svg\\")"\n  view\n    image "symbol:param.glyph"\n');
  const app = join(dir, 'app.contract');
  const before = readFileSync(app, 'utf8');
  let { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(app, `use RackCard from "./vendor/rack/index.contract"\n${before.replace('      NotesCard()\n', '      NotesCard()\n      RackCard()\n')}`);
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/rack/assets/')));
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  writeFileSync(join(vendor, 'assets/pin.svg'), star('M1 1h22v22H1Z'));
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/rack/assets/')));
  writeFileSync(app, before);
}, 120_000);

test('a package design made where none was is announced; a client\'s hello never carries paths', async () => {
  const hello = await post({ method: 'hello', paths: true });
  expect(hello.ok.watch).toBeUndefined();
  expect(hello.ok.packages.every(p => p.path === undefined)).toBe(true);
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(join(dir, 'node_modules/@acme/notes/notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #010101\n');
  const made = await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')));
  expect(made.packages.map(p => p.package).sort()).toEqual(['@acme/notes', '@acme/reading']);
}, 120_000);

test('a client cannot ask for a restore; a package\'s design moved by its package.json is seen', async () => {
  const hello = await post({ method: 'hello' });
  const refused = await post({ method: 'transaction', client: 'agent', files: hello.ok.files, restore: true, ops: [{ op: 'add-token', name: 'reading.card', value: '#999999' }] });
  expect(JSON.stringify(refused)).toContain('`set-token` sets it');
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  mkdirSync(join(dir, 'node_modules/@acme/reading/dist'), { recursive: true });
  writeFileSync(join(dir, 'node_modules/@acme/reading/dist/reading.tamber'), 'tamber reading\n\ntheme\n  reading.card = #0a0a0a\n');
  writeFileSync(join(dir, 'node_modules/@acme/reading/package.json'), JSON.stringify({ name: '@acme/reading', version: '1.0.0', exports: { '.': './index.contract' }, tamber: 'dist/reading.tamber' }));
  const moved = await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.some(f => f.includes('dist/reading.tamber'))));
  expect(moved.packages.find(p => p.package === '@acme/reading').file).toBe('@acme/reading/dist/reading.tamber');
}, 120_000);

test('a local import\'s held add-token of a package name is the override line; packages removed and installed again are followed', async () => {
  const held = await post({ method: 'propose', hold: true, client: 'import:x', review: { importer: 'x', counted: {}, unmapped: [], offers: [], tokens: 1 }, ops: [{ op: 'add-token', name: 'reading.card', value: '#121212' }] });
  expect(held.ok?.proposal).toBeTruthy();
  await post({ method: 'drop', client: 'import:x', proposal: held.ok.proposal });
  // node_modules/@acme removed, then made again: its packages' designs are followed after.
  const keep = join(dir, '..', `acme-${process.pid}`);
  cpSync(join(dir, 'node_modules/@acme'), keep, { recursive: true });
  let { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  rmSync(join(dir, 'node_modules/@acme'), { recursive: true, force: true });
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && (e.packages ?? []).length === 0), 30_000).catch(() => null);
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  cpSync(keep, join(dir, 'node_modules/@acme'), { recursive: true });
  rmSync(keep, { recursive: true, force: true });
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && (e.packages ?? []).length > 0), 30_000);
  ({ seq } = await post({ after: 0, wait: false }, '/__tamber/events'));
  writeFileSync(join(dir, 'node_modules/@acme/notes/notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #020202\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
}, 180_000);

test('a package whose export fails for a while is still watched: restored, its design edits are seen', async () => {
  const entry = join(dir, 'node_modules/@acme/notes/index.contract');
  rmSync(entry);
  // A rescan with the export missing (the app's load fails; its packages unknown).
  await new Promise(ok => setTimeout(ok, 2000));
  writeFileSync(entry, 'component NotesCard\n  view\n    text "n"\n');
  await new Promise(ok => setTimeout(ok, 2000));
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(join(dir, 'node_modules/@acme/notes/notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #030303\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
}, 120_000);

test('a package whose package.json is missing for a while is still watched: mended, its design edits are seen', async () => {
  const manifest = join(dir, 'node_modules/@acme/notes/package.json');
  const text = JSON.stringify({ name: '@acme/notes', version: '1.0.0', exports: { '.': './index.contract' } });
  rmSync(manifest);
  await new Promise(ok => setTimeout(ok, 2000));
  writeFileSync(manifest, text);
  await new Promise(ok => setTimeout(ok, 2000));
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(join(dir, 'node_modules/@acme/notes/notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #040404\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
}, 120_000);

test('a package retargeted to a directory with no package.json yet is watched there: made, its design is seen', async () => {
  const target = mkdtempSync(join(tmpdir(), 'exact-tamber-notes-'));
  writeFileSync(join(target, 'index.contract'), 'component NotesCard\n  view\n    text "n"\n');
  rmSync(join(dir, 'node_modules/@acme/notes'), { recursive: true, force: true });
  symlinkSync(target, join(dir, 'node_modules/@acme/notes'));
  await new Promise(ok => setTimeout(ok, 2500));
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  writeFileSync(join(target, 'package.json'), JSON.stringify({ name: '@acme/notes', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(target, 'notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #050505\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
  // The target removed and made again, the link as it was: followed.
  rmSync(target, { recursive: true, force: true });
  await new Promise(ok => setTimeout(ok, 3000));
  mkdirSync(target);
  writeFileSync(join(target, 'index.contract'), 'component NotesCard\n  view\n    text "n"\n');
  writeFileSync(join(target, 'package.json'), JSON.stringify({ name: '@acme/notes', version: '1.0.0', exports: { '.': './index.contract' } }));
  await new Promise(ok => setTimeout(ok, 3000));
  const again = (await post({ after: 0, wait: false }, '/__tamber/events')).seq;
  writeFileSync(join(target, 'notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #060606\n');
  await until(async () => (await post({ after: again, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
  rmSync(target, { recursive: true, force: true });
}, 120_000);

test('a package link to a directory not made yet is followed once it is made', async () => {
  const target = join(tmpdir(), `exact-tamber-later-${process.pid}`);
  rmSync(target, { recursive: true, force: true });
  rmSync(join(dir, 'node_modules/@acme/notes'), { recursive: true, force: true });
  symlinkSync(target, join(dir, 'node_modules/@acme/notes'));
  await new Promise(ok => setTimeout(ok, 3000));
  const { seq } = await post({ after: 0, wait: false }, '/__tamber/events');
  mkdirSync(target);
  writeFileSync(join(target, 'index.contract'), 'component NotesCard\n  view\n    text "n"\n');
  writeFileSync(join(target, 'package.json'), JSON.stringify({ name: '@acme/notes', version: '1.0.0', exports: { '.': './index.contract' } }));
  writeFileSync(join(target, 'notes.tamber'), 'tamber notes\n\ntheme\n  notes.ink = #070707\n');
  await until(async () => (await post({ after: seq, wait: false }, '/__tamber/events')).events?.find(e => e.event === 'changed' && e.edited?.includes('@acme/notes/notes.tamber')), 30_000);
  rmSync(target, { recursive: true, force: true });
}, 120_000);

