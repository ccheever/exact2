// @ref LLP 1080.002 §3 stage 1 — `tree --ax` on the web, the oracle: the
// fixture's controls as Chrome exposes them, joined to their views; the
// findings caught through the real collector and gone once repaired; a
// protected value omitted; the wasm and JS targets agreeing by testId; and
// Caltrain with complete coverage and no findings. It compiles the fixture
// and needs a wasm Caltrain dist, so the async lane's web-build-test step
// runs it; the glue step skips it.
import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open, assertWebDistApp } from '../../../scripts/agent.mjs';
import { chromium, refuseStale, webChanges } from '../../../scripts/agent-launch.mjs';
import { axClean, axParity } from '../../../scripts/agent-ax.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { jsTargetBuild } from '../serve.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const app = resolveApp('caltrain');
const wasmDist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));
const jsDist = process.env.EXACT_WEB_JS_DIST ? resolve(process.env.EXACT_WEB_JS_DIST) : null;

async function prerequisite() {
  if (process.env.EXACT_GLUE_FAST === '1') return 'the async glue step does not build apps; the web-build-test step runs this test';
  const { unavailable } = chromium();
  if (unavailable) return unavailable;
  try {
    await assertWebDistApp(wasmDist, app);
    if (jsTargetBuild(wasmDist)) return `${wasmDist} is a JS-target build; this test reads the wasm target (bun host/web/build.mjs caltrain-web --wasm)`;
    refuseStale('web', resolve(wasmDist, '.exact-build.json'), webChanges(wasmDist, app).all, 'bun host/web/build.mjs caltrain-web --wasm');
  } catch (error) { return error.message; }
  return null;
}
const unavailable = await prerequisite();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
// The async lane's own step requires it: a skip there would pass on nothing.
if (unavailable && process.env.EXACT_AX_REQUIRED === '1') test('tree --ax prerequisites', () => { throw new Error(unavailable); });
const check = unavailable ? test.skip : test;

let tmp, plan;
function fixture() {
  if (plan) return plan;
  tmp = mkdtempSync(resolve(tmpdir(), 'exact-ax-'));
  plan = resolve(tmp, 'accessibility.plan');
  const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/accessibility.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) throw new Error('the accessibility fixture does not compile: ' + c.stderr);
  return plan;
}
const CONTROLS = { first: ['button', 'Increment'], other: ['button', 'Other'], toggle: ['button', 'Toggle'], 'name-it': ['button', 'Name it'],
  chart: ['image', 'Chart'], heading: ['heading', 'Section'], guide: ['link', 'guide'], check: ['checkbox', 'Checked'], secret: ['textbox', 'Secret'] };
const find = (r, testId) => r.ax.elements.find(e => e.testId === testId && e.via !== 'ancestor');
const kinds = r => r.ax.findings.map(f => `${f.kind} ${f.testId}`).sort();

async function fixtureControls(s) {
  const r = await s.tree(null, { ax: true });
  expect(r.ax.source).toBe('chrome-cdp');
  expect(r.ax.coverage.complete).toBe(true);
  for (const [testId, [role, name]] of Object.entries(CONTROLS)) {
    const e = find(r, testId);
    expect(e && [e.role, e.name, e.via === 'self' || e.via === 'owner']).toEqual([role, name, true]);
  }
  // Coverage: one element per unignored node Chrome reported, the dropped kinds aside.
  const { nodes } = await s.carrier.call('Accessibility.getFullAXTree');
  expect(r.ax.elements.length).toBe(nodes.filter(n => !n.ignored && !['InlineTextBox', 'RootWebArea'].includes(n.role?.value)).length);
  // The decoration is aria-hidden, the inert box is inert: neither is exposed.
  for (const t of ['decoration', 'inert-box', 'inert-button']) expect(find(r, t)).toBeUndefined();
  // A focusable button inside an image role: present exactly when Chrome exposes it.
  const raw = nodes.some(n => !n.ignored && n.role?.value === 'button' && n.name?.value === 'Inside');
  expect(Boolean(find(r, 'chart-button'))).toBe(raw);
  return r;
}

check('tree --ax reads Chrome\'s tree for the fixture, and its findings fire and clear through the collector', async () => {
  const s = await open({ host: 'web', plan: fixture(), webDist: wasmDist });
  try {
    let r = await fixtureControls(s);
    expect(kinds(r)).toEqual(['unnamed unnamed']);
    expect(axClean(r, Object.keys(CONTROLS))).toBe(true);
    await s.tap('name-it');
    r = await s.tree(null, { ax: true });
    expect(find(r, 'unnamed')?.name).toBe('Add');
    expect(r.ax.findings).toEqual([]);
    // A negative through the real collector: the page loses `inert` while the plan still says inert.
    const box = (await s.tree('inert-box', { shallow: true })).nodes[0].id;
    await s.carrier.evaluate(`exact.views.get(${box}).removeAttribute('inert')`);
    r = await s.tree(null, { ax: true });
    expect(kinds(r)).toEqual(['exposed-hidden inert-button']);
    await s.carrier.evaluate(`exact.views.get(${box}).setAttribute('inert', '')`);
    expect((await s.tree(null, { ax: true })).ax.findings).toEqual([]);
    // A protected value is omitted, never read from the editor.
    await s.type('secret', 'hunter2');
    const secret = find(await s.tree(null, { ax: true }), 'secret');
    expect([secret.value, secret.states.protected]).toEqual([undefined, true]);
    // A targeted read keeps the target's subtree and the chain above it.
    const t = await s.tree('chart', { ax: true });
    expect(t.ax.elements.every(e => e.testId === 'chart' || e.id !== null)).toBe(true);
    expect(t.ax.ancestors).toBeArray();
  } finally { await s.close(); }
}, 180000);

check('tree --ax: a reload between the brackets is retried, never joined across documents', async () => {
  const s = await open({ host: 'web', plan: fixture(), webDist: wasmDist });
  try {
    const ask = s.carrier.ask;
    let reloads = 0;
    s.carrier.ask = async req => { const r = await ask(req); if (req.op === 'axStamp' && reloads < 6) { reloads++; return { ...r, nonce: r.nonce + reloads }; } return r; };
    const r = await s.tree(null, { ax: true });
    expect(r.spanned).toBe(true);
    s.carrier.ask = ask;
    expect((await s.tree(null, { ax: true })).spanned).toBeUndefined();
  } finally { await s.close(); }
}, 120000);

(jsDist ? check : test.skip)('tree --ax: the wasm and JS targets agree by testId on the fixture controls', async () => {
  const read = async dist => { const s = await open({ host: 'web', plan: fixture(), webDist: dist }); try { return await s.tree(null, { ax: true }); } finally { await s.close(); } };
  const [wasm, js] = [await read(wasmDist), await read(jsDist)];
  const { findings, unjoined } = axParity(wasm, js, Object.keys(CONTROLS));
  expect({ findings, unjoined }).toEqual({ findings: [], unjoined: [] });
}, 240000);

check('tree --ax on Caltrain: complete coverage, the station controls named, no findings', async () => {
  const s = await open({ host: 'web', webDist: wasmDist });
  try {
    await s.clock('settle');
    let r = await s.tree(null, { ax: true });
    expect(axClean(r, ['change-station'])).toBe(true);
    expect(r.ax.findings).toEqual([]);
    await s.tap('change-station'); await s.clock('settle');
    r = await s.tree(null, { ax: true });
    expect(axClean(r, ['station-search'])).toBe(true);
    expect(find(r, 'station-search')?.name).toBe('Search stations');
    expect(r.ax.findings).toEqual([]);
  } finally { await s.close(); if (tmp) rmSync(tmp, { recursive: true, force: true }); }
}, 120000);
