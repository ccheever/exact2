// The JS target over a baked plan (host/web-js/rt.js `res`): a build-time
// answer is the first frame. A source not ready yet (a Rust module loads after
// first paint) leaves it shown, not pending, and asks it at `ready`, as a native
// runner asks at data_ready (review B3); a settled answer (an `else` row's, a dev
// reload's) is not asked again (review B4). rt.js runs beside stand-ins for the
// modules it imports, with the real shape.js.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-baked-'));
const webJs = name => resolve(new URL(`../../web-js/${name}`, import.meta.url).pathname);
for (const f of ['rt.js', 'shape.js']) copyFileSync(webJs(f), resolve(dir, f));
for (const [file, names] of Object.entries({ 'navigation.js': ['renderMarkup', 'reportPlace', 'animationClocks', 'launchLocation'], 'pointer.js': ['pointer'], 'commands.js': ['commands'],
  'media.js': ['media', 'mediaProp', 'mediaOn', 'mediaPiece'], 'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'],
  'svg-transform.js': ['svgTransform'], 'dataset.js': ['ds'], 'hooks.js': ['hk'], 'perf.js': ['pf'], 'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber'] }))
  writeFileSync(resolve(dir, file), names.map(n => `export const ${n} = () => {};`).join('\n') + (file === 'media.js' ? '\nexport const MEDIA_EVENTS = new Set();' : ''));

test('a baked answer shows until the source is ready, then is asked; a settled one is not', async () => {
  const { res, data } = await import(resolve(dir, 'rt.js'));
  const asked = [];
  data.answer = (source) => { asked.push(source); return null; }; // not ready: no value, no request
  const baked = res('stamp', 'stamp', () => [], 0, [], 'n', 0);
  const kept = res('preview', 'preview', () => [], 5, [], 'n', 0, true);
  expect([baked(), baked.p(), kept(), kept.p()]).toEqual([0, false, 5, false]);
  expect(asked).toEqual(['stamp']); // the settled row is never asked
  expect(data.q.length).toBe(1);
  data.answer = (source) => { asked.push(source); return { v: source === 'stamp' ? 42 : 6 }; };
  for (const f of data.q.splice(0)) f();
  expect([baked(), kept()]).toEqual([42, 5]);
  expect(asked).toEqual(['stamp', 'stamp']);
});
