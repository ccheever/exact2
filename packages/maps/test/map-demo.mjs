// LLP 1133.021 §13's map drives 1 to 4 on Map Demo, under the agent, where
// the map is the real package view with plain tiles (§12), and drive 5, the
// web build refused without a tile source.
//
// 1. Select on the map: `tap map/ferry`, real input at the marker.
// 2. Select by button: the map's selection follows `selected`, no `change`.
// 3. The camera reported: the zoom buttons (shown under `prefer pointer
//    none`) zoom from where `moveend` last left the camera.
// 4. Plain tiles: a screenshot, kept for a person to read.
// 5. A missing source: the web build without `tiles` fails `maps-tiles-missing`.
//
// usage: bun packages/maps/test/map-demo.mjs <web|macos|ios> [--browser chromium|firefox|webkit] [--sim <udid>] [--out <dir>]
//   (Map Demo built for that host first: host/web/build.mjs map-demo, or host/apple/build.mjs [--ios] map-demo-apple)
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { textOf } from '../../../scripts/agent-test.mjs';

const here = import.meta.dir, root = resolve(here, '../../..');
const argv = process.argv.slice(2), host = argv[0] ?? 'web';
const flag = (name) => argv.includes(name) ? argv[argv.indexOf(name) + 1] : null;
const browser = flag('--browser'), sim = flag('--sim');
const out = flag('--out') ?? resolve(root, 'target/maps-drives');
mkdirSync(out, { recursive: true });
let failures = 0;
const check = (name, ok, detail) => { if (!ok) failures++; console.log(`${ok ? 'PASS' : 'FAIL'} ${host}${browser ? `/${browser}` : ''} ${name}${ok ? '' : `: ${detail}`}`); };

/** One launch: each op's JSON reply. */
function drive(ops, extra = []) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'map-demo', '--size', '600x800',
    ...(browser ? ['--browser', browser === 'chromium' ? 'chrome' : browser] : []), ...extra, ...ops];
  const env = { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-', ...(sim ? { EXACT_SIM: sim } : {}) };
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => { try { return JSON.parse(l); } catch { return l; } });
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}
const text = (tree, id) => {
  const nodes = tree?.nodes ?? [], node = nodes.find(n => n.props?.testId === id);
  return node ? textOf(nodes, node) ?? '' : '';
};
const lines = r => r.find(x => Array.isArray(x?.lines))?.lines ?? [];
const maps = r => r.find(x => x?.maps?.implementation)?.maps.implementation.maps ?? null;
const platform = host === 'ios' ? ['--touch', 'platform'] : [];

// 1: a marker selected on the map is the app's `change`.
{
  const r = drive(['clock settle', 'tap map/ferry', 'clock settle', 'tree', 'state extensions', 'logs'], platform);
  const tree = r[3];
  check('1: tap map/ferry selects it', text(tree, 'selected') === 'Selected: ferry', text(tree, 'selected') || String(JSON.stringify(r[1] ?? r)).slice(0, 400));
  check('1: one pick from the map', text(tree, 'picks').endsWith(': 1'), text(tree, 'picks'));
  const m = maps(r);
  if (host === 'web') check("1: state.extensions shows the marker selected", m?.[0]?.selected === 'ferry', JSON.stringify(m));
  else check('1: the map reports the person\'s selection', lines(r).some(l => /map \d+: selected ferry/.test(l)), lines(r).filter(l => /map /.test(l)).join(' | '));
}
// 2: a button selects through the prop; the map raises no `change`.
{
  const r = drive(['clock settle', 'tap pick-coit', 'clock settle', 'tree', 'state extensions', 'logs']);
  const tree = r[3];
  check('2: the button selects coit', text(tree, 'selected') === 'Selected: coit', text(tree, 'selected'));
  check('2: no pick from the map', text(tree, 'picks').endsWith(': 0'), text(tree, 'picks'));
  const m = maps(r);
  if (host === 'web') check("2: the map's selected marker is coit", m?.[0]?.selected === 'coit', JSON.stringify(m));
  else check('2: no selection event from the map', !lines(r).some(l => /map \d+: selected/.test(l)), lines(r).filter(l => /map /.test(l)).join(' | '));
}
// 3: the zoom buttons zoom from the camera `moveend` reported.
{
  const pointer = host === 'web' ? [] : ['prefer pointer none', 'clock settle'];
  const zoomIn = host === 'web' ? [] : ['tap zoom-in', 'clock +600', 'clock settle', 'tree', 'tap zoom-in', 'clock +600', 'clock settle', 'tree'];
  // The web map's `moveend` waits for the gesture to rest on the page's own clock, not the agent's: wait for it there.
  const rested = host === 'web' ? 'clock until state extensions.maps.implementation.maps.0.camera.zoom == 15.8 within 3000' : 'clock +600';
  // A person's zoom by the wheel, where there is one (the web, macOS).
  const wheel = host === 'ios' ? [] : ['tap map wheel 0 -400', rested, 'clock settle', 'tree'];
  const r = drive(['clock settle', ...pointer, 'clock +600', 'clock settle', 'tree', ...zoomIn, ...wheel, 'logs']);
  const trees = r.filter(x => Array.isArray(x?.nodes));
  const zooms = trees.map(t => Number(text(t, 'zoom').replace('Zoom: ', '')));
  check('3: the first camera is reported at 15', Math.abs(zooms[0] - 15) < 0.01, zooms);
  if (host !== 'web') {
    check('3: zoom in moves the map to 16, and moveend says so', Math.abs(zooms[1] - 16) < 0.01, zooms);
    check('3: and again to 17', Math.abs(zooms[2] - 17) < 0.01, zooms);
  }
  const last = zooms.at(-1), before = zooms.at(-2);
  if (host !== 'ios') check("3: a person's wheel moves the camera, and moveend reports where it rests", Number.isFinite(last) && Math.abs(last - before) > 0.05, zooms);
}
// 4: plain tiles, a screenshot for a person.
{
  const shot = resolve(out, `map-demo-${host}${browser ? `-${browser}` : ''}.png`);
  const r = drive(['clock settle', 'tap pick-ferry', 'clock +600', 'clock settle', `screenshot ${shot}`, 'state extensions', 'logs']);
  const m = maps(r);
  check('4: the tiles are plain under the agent', host === 'web' ? m?.[0]?.tiles === 'plain' : lines(r).some(l => /map \d+: plain tiles/.test(l)), JSON.stringify(m) + lines(r).filter(l => /tiles/.test(l)).join(' | '));
  console.log(`     screenshot: ${shot}`);
}
// 5: no tile source, no web build.
if (host === 'web') {
  // Under the checkout, so the copy finds the workspace's `node_modules` and its `@exact/maps`.
  const dir = mkdtempSync(resolve(root, 'target/maps-tiles-'));
  try {
    const app = join(dir, 'map-demo');
    cpSync(resolve(root, 'apps/map-demo'), app, { recursive: true, filter: f => !f.includes('/target') && !f.includes('/.exact') });
    const manifest = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
    delete manifest.packages['@exact/maps'].web.tiles;
    writeFileSync(join(app, 'app.json'), JSON.stringify(manifest, null, 2));
    const r = spawnSync('bun', ['host/web/build.mjs', 'map-demo', '--render', 'none'], { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: app }, maxBuffer: 1 << 26 });
    const said = `${r.stdout}\n${r.stderr}`;
    check('5: the web build without tiles fails maps-tiles-missing', r.status !== 0 && said.includes('maps-tiles-missing'), said.slice(-800));
  } finally { rmSync(dir, { recursive: true, force: true }); }
}
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
