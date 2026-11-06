// What the JS target reports to presence (rt.js over shared.js `presence`): a held
// tree update reports its own gains and writes when it runs, and a refused or
// poisoned commit publishes none of its writes. rt.js runs beside stand-ins for
// the modules it imports; shared.js's bridge is real, its view transition holds
// every tree update, and presence-glue.js records what it is told.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { standIns } from './stand-ins.mjs';

// The runtime in a `web-js` directory, as built: modules.js imports the web host's `../web/address-space.js`, stood in.
const root = mkdtempSync(resolve(tmpdir(), 'exact-js-presence-')), dir = resolve(root, 'web-js');
mkdirSync(dir);
mkdirSync(resolve(root, 'web'));
writeFileSync(resolve(root, 'web/address-space.js'), 'export const setSpaceState = () => {};');
const webJs = name => resolve(new URL(`../../web-js/${name}`, import.meta.url).pathname);
// regions.js is real: its mapper's stand-in would erase the values `S` writes.
const REAL = ['rt.js', 'uploads.js', 'held.js', 'font-family.js', 'observers.js', 'grid.js', 'regions.js', 'overlay.js', 'roster.js', 'router.js', 'route-pattern.js', 'schedule.js', 'budget.js', 'shape.js', 'modules.js', 'samples.js', 'spans.js', 'update.js', 'arrival.js', 'devices.js', 'wakes.js', 'reach.js', 'members.js', 'resource-inspection.js', 'sheets.js'];
for (const f of REAL) copyFileSync(webJs(f), resolve(dir, f));
for (const [file, source] of standIns(REAL)) writeFileSync(resolve(dir, file), source);
const shared = readFileSync(webJs('shared.js'), 'utf8');
writeFileSync(resolve(dir, 'shared.js'), `${shared.slice(shared.indexOf('export function presence('), shared.indexOf('const NAME ='))}\nexport const commit = (tail) => { globalThis.heldTail = tail; return true; };`);
writeFileSync(resolve(dir, 'presence-glue.js'), `const record = (phase, batch, views) => globalThis.presenceReports?.push({ phase, ops: batch.ops.map(op => ({ op: op.op, name: views.get(op.id).name })) });
globalThis.exact.presence = () => ({ before(batch, views) { record('before', batch, views); }, after(batch, views) { record('after', batch, views); }, exit() {} });`);
writeFileSync(resolve(dir, 'names.js'), 'export const sourceTypes = {};');

test('presence reports follow the actual held tail and discard refused or poisoned writes', async () => {
  const saved = Object.fromEntries(['document', 'requestAnimationFrame', 'heldTail', 'presenceReports'].map(key => [key, globalThis[key]]));
  globalThis.document = { getElementById: () => ({}) };
  globalThis.requestAnimationFrame = f => setTimeout(f, 0);
  globalThis.presenceReports = [];
  const rt = await import(resolve(dir, 'rt.js'));
  const { S, sig, memo, effect, W, act, commit, pr, pieces, settled, clock, journal } = rt;
  const agent = clock.agent; clock.agent = true;
  try {
    pr({}); await pieces();
    const element = name => {
      const values = new Map();
      return { name, isConnected: true, style: { getPropertyValue: p => values.get(p) ?? '', removeProperty: p => values.delete(p), setProperty: (p, v) => values.set(p, v) } };
    };
    const first = element('first'), second = element('second'), a = sig(false), b = sig(false), calls = [0, 0];
    S(first, '--exact-layout-transition', '', () => { calls[0]++; return a() ? '1000 0 linear' : null; });
    S(second, '--exact-layout-transition', '', () => { calls[1]++; return b() ? '1000 0 linear' : null; });
    commit(() => {}); heldTail(); presenceReports.length = 0;
    act(() => W(a, true))(); const tailA = heldTail;
    act(() => W(b, true))(); const tailB = heldTail;
    const held = { calls: [...calls], reports: [...presenceReports], first: first.style.getPropertyValue('--exact-layout-transition') };
    tailA(); const landed = [...presenceReports]; presenceReports.length = 0;
    tailB(); const drained = [...presenceReports]; presenceReports.length = 0;

    const gate = sig(0), valid = memo(() => gate() ? NaN : 0, 'n'); valid();
    const refusedResult = act(() => { W(gate, 1); W(a, false); })();
    const refused = { result: refusedResult, value: a(), css: first.style.getPropertyValue('--exact-layout-transition'), reports: [...presenceReports] };
    presenceReports.length = 0;
    const boom = sig(false);
    S(first, 'top', 'px', () => boom() ? 10 : 0);
    effect(() => { if (boom()) throw Error('owned style trap'); });
    commit(() => {}); heldTail(); presenceReports.length = 0;
    act(() => W(boom, true))(); const poisoned = heldTail();
    const poisonReports = [...presenceReports]; presenceReports.length = 0;
    settled(false); // a later publication must not replay the discarded write
    expect({ held, landed, drained, calls, refused, poisoned, poisonReports, afterPoison: [...presenceReports],
      poisonNamed: journal.some(line => line.includes('poisoned: owned style trap')) }).toEqual({
      held: { calls: [1, 1], reports: [], first: '' },
      landed: [{ phase: 'before', ops: [{ op: 'style', name: 'first' }, { op: 'style', name: 'second' }] },
        { phase: 'after', ops: [{ op: 'style', name: 'first' }, { op: 'style', name: 'second' }] }],
      drained: [{ phase: 'before', ops: [] }, { phase: 'after', ops: [] }], calls: [3, 2],
      refused: { result: false, value: true, css: '1000 0 linear', reports: [{ phase: 'before', ops: [{ op: 'style', name: 'first' }] }, { phase: 'after', ops: [] }] },
      poisoned: false, poisonReports: [{ phase: 'before', ops: [] }], afterPoison: [], poisonNamed: true,
    });
  } finally {
    clock.agent = agent;
    for (const [key, value] of Object.entries(saved)) if (value === undefined) delete globalThis[key]; else globalThis[key] = value;
  }
});
