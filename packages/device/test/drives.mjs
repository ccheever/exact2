// LLP 1133.031 §10's drives under the agent: Weatherlight's 1 to 5 and
// Expose's 6 to 9, each fact set with `prefer extension device.<source>=…`
// and read back from the tree, the state and the screen.
//
// - Where a source is `unsupported` (`powerState` on the web) the drives
//   that need it are skipped, and drive 5 checks the failure instead.
// - "A still sky" is a film of the agent's clock whose frames are the same
//   pixels; a moving one has frames that differ.
// - Weatherlight and Expose have no Linux host binary (each `*-linux` crate
//   is its render bin), so Linux's drives are the fixture's, in `linux.mjs`.
//
// usage: bun packages/device/test/drives.mjs <web|macos|ios> [--only 1,2,…] [--sim <udid>]
//   Builds nothing: build the two apps for the host first (`bun host/web/build.mjs weatherlight`,
//   `bun host/apple/build.mjs [--ios --sim <udid>] weatherlight-apple`, and the same for expose).
import { createHash } from 'node:crypto';
import { mkdtempSync, readdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { decodePng } from '../../../scripts/png.mjs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2);
const host = argv.find((a) => ['web', 'macos', 'ios'].includes(a)) ?? 'macos';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',') : null;
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
const web = host === 'web';
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const shots = mkdtempSync(join(tmpdir(), 'exact-device-drives-'));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `: ${detail}`}`);
  if (!ok) failed++;
};

/** The node with `testId`, and every node by id. */
async function node(s, testId) {
  const { nodes } = await s.tree();
  return { node: nodes.find((n) => n.props?.testId === testId), byId: new Map(nodes.map((n) => [n.id, n])) };
}
// The tree names the label `aria-label` on native hosts and `accessibilityLabel` on the web.
const label = async (s) => { const p = (await node(s, 'pause-sky')).node?.props; return p?.['aria-label'] ?? p?.accessibilityLabel; };
const revision = async (s) => (await s.state()).slots?.revision;
const prefer = async (s, source, value) => { await s.prefer({ extension: `device.${source}=${JSON.stringify(value)}` }); await s.clock('settle'); };

/** Whether the sky moves: a film of 600 ms of the agent's clock, its frames compared. */
async function moving(s, name) {
  const film = await s.screenshot(join(shots, `${name}.png`), { over: 600, every: 100 });
  const hashes = new Set(readdirSync(film.dir).map((f) => createHash('sha256').update(readFileSync(join(film.dir, f))).digest('hex')));
  return hashes.size > 1;
}

const weatherlight = () => open({ host, app: 'weatherlight', storage: `device-drives-${Date.now().toString(36)}` });
const drives = {
  async 1() {
    const s = await weatherlight();
    try {
      await s.clock('settle');
      check(1, 'nothing preferred: the sky offers pause', await label(s) === 'Pause sky animation', await label(s));
    } finally { await s.close(); }
  },
  async 2() {
    const s = await weatherlight();
    try {
      await s.clock('settle');
      await prefer(s, 'powerState', { lowPowerMode: true });
      check(2, 'Low Power Mode: the sky offers play', await label(s) === 'Play sky animation', await label(s));
      check(2, 'Low Power Mode: the sky is still', !(await moving(s, 'low-power')));
      await s.tap('pause-sky');
      await s.clock('settle');
      check(2, 'play pressed in Low Power Mode: the sky moves', await label(s) === 'Pause sky animation' && await moving(s, 'pressed-play'), await label(s));
      await prefer(s, 'powerState', { lowPowerMode: false });
      await s.tap('pause-sky');
      await s.clock('settle');
      check(2, 'Low Power Mode off, pause pressed: the sky stays paused', await label(s) === 'Play sky animation' && !(await moving(s, 'pressed-pause')), await label(s));
    } finally { await s.close(); }
  },
  async 3() {
    const s = await weatherlight();
    try {
      await s.clock('settle');
      for (const [value, want] of [[{ thermalState: 'serious' }, 'Play'], [{ thermalState: 'fair' }, 'Pause'], [{ lowPowerMode: true, thermalState: 'fair' }, 'Play']]) {
        await prefer(s, 'powerState', value);
        check(3, `${JSON.stringify(value)}: the sky ${want === 'Play' ? 'pauses' : 'moves'}`, await label(s) === `${want} sky animation`, await label(s));
      }
    } finally { await s.close(); }
  },
  async 4() {
    const s = await weatherlight();
    try {
      await s.clock('+1000');
      await s.clock('settle');
      const first = await revision(s);
      await s.clock('+600000');
      check(4, 'ten minutes: a refresh', await revision(s) === first + 1, `${first} then ${await revision(s)}`);
      await prefer(s, 'connection', { saveData: true });
      const saving = await revision(s);
      await s.clock('+600000');
      check(4, 'saving data, ten minutes: no refresh', await revision(s) === saving, `${saving} then ${await revision(s)}`);
      await s.clock('+1200000');
      check(4, 'saving data, thirty minutes: a refresh', await revision(s) === saving + 1, `${saving} then ${await revision(s)}`);
      // Reload is disabled while that refresh is pending.
      await s.clock('settle');
      const before = await revision(s);
      await s.tap('refresh-weather');
      check(4, 'Reload refreshes at once', await revision(s) === before + 1, `${before} then ${await revision(s)}`);
    } finally { await s.close(); }
  },
  async '4a'() {
    const s = await weatherlight();
    try {
      await prefer(s, 'connection', { saveData: true });
      await s.clock('+1000');
      check('4a', 'saving data from the start: the first forecast is asked at once', await revision(s) === 1, await revision(s));
    } finally { await s.close(); }
  },
  async 5() {
    const s = await weatherlight();
    try {
      await s.clock('settle');
      // The web's `state.extensions` lists readers, not answers: the failure is the resource's, in `state.failed`.
      const state = await s.state();
      check(5, 'powerState failed NotSupportedError', String(state.failed?.power ?? '').includes('NotSupportedError'), JSON.stringify(state.failed));
      check(5, 'the placeholder stands: no thermal state', state.resources?.power?.thermalState == null, JSON.stringify(state.resources?.power));
    } finally { await s.close(); }
  },
};

/** Expose, unlocked at its Settings, and the battery mark's fill and bolt. */
const expose = async () => {
  const s = await open({ host, app: 'expose', storage: `device-drives-${Date.now().toString(36)}` });
  await s.clock('settle');
  await s.tap('lock-unlock');
  await s.tap('app-settings');
  await s.clock('settle');
  return s;
};
async function mark(s) {
  const { node: svg, byId } = await node(s, 'battery-mark');
  const children = (svg?.children ?? []).map((id) => byId.get(id));
  const fill = children.filter((c) => c?.props?.width != null)[1];
  if (fill) return { width: Number(fill.props.width), color: fill.props.fill, bolt: children.some((c) => c?.props?.['stroke-linejoin'] === 'round') };
  return measured(s, children.length);
}
/** The web's tree carries no SVG attributes: the fill measured on the screen instead, to a quarter unit. In the mark's 28-unit viewBox
 * the fill starts at 2.5 and a full one ends at 20; a column is filled where it differs from the empty inside at 22.
 * The row is the fill's top quarter, above the bolt. The bolt is the mark's fourth child. */
async function measured(s, count) {
  const { space } = (await s.layout('battery-mark')).node;
  const shot = await s.screenshot(join(shots, `mark-${Date.now()}.png`));
  const image = decodePng(readFileSync(shot.screenshot));
  const k = image.width / shot.w, unit = space.viewport.w / 28;
  const at = (x) => { const px = Math.floor((space.viewport.x + x * unit) * k), py = Math.floor((space.viewport.y + 4 * unit) * k), i = (py * image.width + px) * 4; return [...image.data.subarray(i, i + 3)]; };
  const empty = at(22), filled = [];
  for (let x = 2.5; x < 21; x += 0.25) if (at(x).reduce((d, c, i) => d + Math.abs(c - empty[i]), 0) > 90) filled.push(x);
  const width = filled.length ? filled.at(-1) + 0.25 - 2.5 : 0;
  return { width, color: `rgb(${at(5).join(', ')})`, bolt: count === 4 };
}
const row = async (s) => {
  const { node: r, byId } = await node(s, 'row-battery');
  const text = (n) => n?.props?.text ?? (n?.children ?? []).map((c) => text(byId.get(c))).join(' ');
  return text(r).replace(/\s+/g, ' ').trim();
};
Object.assign(drives, {
  async 6() {
    const s = await expose();
    try {
      const m = await mark(s);
      check(6, 'nothing preferred: the full mark with a bolt', Math.abs(m.width - 17.5) <= 0.5 && m.bolt, JSON.stringify(m));
      check(6, 'Settings shows 100%', (await row(s)).includes('100%'), await row(s));
      await s.screenshot(join(shots, 'expose-default.png'));
    } finally { await s.close(); }
  },
  async 7() {
    const s = await expose();
    try {
      await prefer(s, 'getBattery', { level: 0.42, charging: false });
      const m = await mark(s);
      check(7, '0.42 discharging: Settings shows 42%', (await row(s)).includes('42%'), await row(s));
      check(7, '0.42 discharging: the fill is under half its width, no bolt', m.width < 17.5 / 2 && !m.bolt, JSON.stringify(m));
      await s.screenshot(join(shots, 'expose-42.png'));
      await s.clock('+60000');
      await prefer(s, 'getBattery', { level: 0.41, charging: false });
      check(7, 'a minute on, 0.41: 41%, the earlier answer replaced', (await row(s)).includes('41%'), await row(s));
    } finally { await s.close(); }
  },
  async 8() {
    const s = await expose();
    try {
      const before = (await mark(s)).color;
      await prefer(s, 'powerState', { lowPowerMode: true });
      const m = await mark(s);
      check(8, 'Low Power Mode: the fill is the saver yellow', m.color !== before && /saver|#ffcc00|#ffd60a|255, ?204|255, ?214/i.test(String(m.color)), `${before} then ${m.color}`);
      await s.screenshot(join(shots, 'expose-low-power.png'));
    } finally { await s.close(); }
  },
});

const want = web ? [1, 4, '4a', 5, 6, 7] : [1, 2, 3, 4, '4a', 6, 7, 8];
for (const n of want) if (!only || only.map(String).includes(String(n))) await drives[n]();
console.log(`screenshots and films: ${shots}`);
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
