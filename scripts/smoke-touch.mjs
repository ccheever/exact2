#!/usr/bin/env bun
// Real touches on an iOS simulator (LLP 1080.000 §5–§6): Caltrain driven
// through the touch runner (`--touch platform`), in both timing modes. Each
// `tap` must reply `delivery: platform` with the touch the app's window
// dispatched; what the tap did is read from `tree`, never from the reply.
// A held contact is one real touch played at its lift (§14); a whole real drag
// (`tap … drag`, §11) scrolls the stations, with a read while the finger is
// down, and a still press opens them as a tap does. Prints the runner's
// start and per-tap times (G2) as observations, never a gate. A runner that
// does not start fails: this simulator is the supported destination. Then a
// grouped list (LLP 1084, `contract/corpus/grouped-touch.contract`, in
// Caltrain's bundle): a real tap on a row's switch, detail button or row
// lands on the cell or accessory UIKit draws, not the hidden authored node.
//   bun scripts/smoke-touch.mjs [--build] [--sim <udid|name>]
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from './agent.mjs';
import { HOST_DEV, resolveApp } from './app.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { chooseSimulator } from '../host/apple/devices.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
const shell = (command, args) => {
  const result = spawnSync(command, args, {cwd: ROOT, encoding: 'utf8', timeout:15_000});
  if (result.status !== 0) throw Error(`${command} ${args.join(' ')}: ${result.error?.message || result.stderr || result.stdout}`);
  return result.stdout.trim();
};
const simctl = (...args) => shell('xcrun', ['simctl', ...args]);
let ownedSim;
try {
  if (!process.env.EXACT_SIM) {
    const sdk = shell('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-version']);
    const version = (a, b) => a.localeCompare(b, undefined, {numeric:true});
    const runtime = JSON.parse(simctl('list', 'runtimes', '-j')).runtimes.filter(r => r.isAvailable && r.identifier.includes('.iOS-') && version(r.version, sdk) <= 0).sort((a, b) => version(b.version, a.version))[0];
    if (!runtime) throw Error(`ios-touch needs an available iOS runtime supported by the selected simulator SDK ${sdk}`);
    ownedSim = simctl('create', `exact-ios-touch-${process.pid}`, 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro', runtime.identifier);
    process.env.EXACT_SIM = ownedSim;
  }
  const device = chooseSimulator(process.env.EXACT_SIM);
  if (device.type !== 'iPhone-17-Pro') throw Error(`ios-touch refuses ${device.name} (${device.type}): the Island hold and Lock Screen swipe points were measured on iPhone 17 Pro`);
  console.log(`ios-touch: ${device.name} ${device.udid}, ${device.runtime}`);
if (argv.includes('--build')) {
  const built = spawnSync('bun', ['host/apple/build.mjs', '--ios', 'caltrain-apple', '--widget-gallery', '--no-install'], { cwd: ROOT, stdio: 'inherit' });
  if (built.status !== 0) throw Error('FAIL ios-touch build: host/apple/build.mjs --ios caltrain-apple --widget-gallery --no-install');
  // WP9's refusal scan uses this iOS app, its extension and its packaged arm.
  const bundle = appleArtifacts(resolveApp('caltrain'), { destination: 'ios-simulator' }).bundle;
  const refused = spawnSync('bun', ['test', 'host/apple/link.test.mjs'], { cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_ACTIVITY_APP: bundle } });
  if (refused.status !== 0) throw Error('FAIL ios-touch activity refusals');
}
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const t0 = Date.now();

for (const timing of ['agent', 'platform']) {
  const opened = Date.now();
  let s;
  try { s = await open({ host: 'ios', app: 'caltrain', touch: 'platform', timing }); }
  catch (error) { check(false, `${timing}: the session did not open: ${error.message}`); continue; }
  try {
    console.log(`${timing}: opened in ${Date.now() - opened} ms (the runner's start, then the app's)`);
    const taps = [];
    const tap = async (target) => {
      const at = Date.now();
      const r = await s.tap(target);
      taps.push(Date.now() - at);
      check(r.delivery === 'platform' && r.landed?.session === 'main' && r.touch?.type === 'direct', `${timing}: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
      return r;
    };
    // A still press (§11): the finger down 300 ms, a read inside that hold, then up, presses as a tap does.
    const still = await s.tap('change-station', { drag: { dx: 0, dy: 0, hold: 300, during: [async () => (await s.tree()).nodes.length] } });
    check(still.delivery === 'platform' && still.touch?.moved === 0 && still.during?.[0] > 0, `${timing}: a still press was not one real touch with a read while down: ${JSON.stringify(still)}`);
    // Settle the push the press started in either timing: a real touch played while it moves can be lost
    await s.clock('settle');
    let tree = await s.tree();
    check(byTestId(tree, 'station-search'), `${timing}: a still real press on change-station did not open the stations (landed on ${JSON.stringify(still.landed)})`);
    // A whole real drag up the stations scrolls them: the row moves up the screen.
    const row = async () => (await s.layout()).nodes.find((n) => n.props?.testId === 'station-paloalto' || n.testId === 'station-paloalto')?.y;
    const y0 = await row();
    const dragged = await s.tap('stations-screen', { drag: { dx: 0, dy: -250, over: 400, hold: 100 } });
    await s.clock('settle');
    const y1 = await row();
    check(dragged.delivery === 'platform' && dragged.touch?.moved > 0 && y0 - y1 > 150, `${timing}: a real drag of 250 pt moved the stations ${y0 - y1} pt: ${JSON.stringify(dragged)}`);
    console.log(`${timing}: drag moved the stations ${y0 - y1} pt in ${dragged.touch?.moved} moves`);
    await s.type('station-search', 'Palo');
    await tap('station-paloalto');
    if (timing === 'platform') await s.clock('settle');
    tree = await s.tree();
    check(byTestId(tree, 'station-name')?.props.text === 'Palo Alto', `${timing}: after a real tap on station-paloalto the station is ${byTestId(tree, 'station-name')?.props.text}`);
    // A held contact is one real touch, played whole at its lift (LLP 1080.000 §14): the runner holds none across requests (P3).
    const down = await s.tap('station-name', { down: true });
    await s.pointer('hold', { ms: 600 });
    const up = await s.pointer('up');
    check(down.delivery === 'platform' && up.delivery === 'platform' && up.touch?.type === 'direct' && up.touch.ended - up.touch.began > 0.5, `${timing}: a held contact was not one real touch held 600 ms: ${JSON.stringify({ down, up })}`);
    console.log(`${timing}: taps ${taps.join(', ')} ms`);
  } catch (error) {
    check(false, `${timing}: ${error.message}`);
  } finally { await s.close(); }
}

// A grouped list's rows under real touches: each tap aims at what UIKit
// draws (the switch, the detail button, the cell) and does what a finger does.
{
  const plan = resolve(mkdtempSync(resolve(ROOT, 'target/exact-touch-')), 'grouped-touch.plan');
  const c = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/grouped-touch.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  let s;
  if (check(c.status === 0, `grouped: the fixture did not compile: ${c.stderr}`)) {
    try {
      s = await open({ host: 'ios', app: 'caltrain', plan, touch: 'platform' });
      // Settle the launch first: a real tap during it can land on a switch that does not act yet
      await s.clock('settle');
      const facts = async () => byTestId(await s.tree(), 'facts')?.props.text;
      for (const [target, want] of [['toggle', 'true 0 0'], ['toggle', 'false 0 0'], ['info', 'false 1 0'], ['open', 'false 1 1']]) {
        const r = await s.tap(target);
        check(r.delivery === 'platform' && r.touch?.type === 'direct', `grouped: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
        await s.clock('settle');
        const seen = await facts();
        check(seen === want, `grouped: after a real tap on ${target} the facts read ${seen}, not ${want} (landed ${JSON.stringify(r.landed)})`);
      }
      console.log('grouped: a real switch, detail button and row tap each landed on UIKit\'s cell');
    } catch (error) {
      check(false, `grouped: ${error.message}`);
    } finally { await s?.close(); }
  }
}

// No foreground drive follows the archive: each kind ends from a fresh app launch.
await archiveActivities(device.udid, check);

console.log(`touch smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
for (const f of failures) console.error(`FAIL ios-touch ${f}`);
process.exitCode = failures.length ? 1 : 0;
} finally {
  if (ownedSim) {
    spawnSync('xcrun', ['simctl', 'shutdown', ownedSim]);
    simctl('delete', ownedSim);
    delete process.env.EXACT_SIM;
  }
}

// LLP 1126.001 WP14: compare through the touch step's runner, then end from a fresh app.
async function archiveActivities(udid, check) {
  const dir = mkdtempSync(resolve(ROOT, 'target/ios-touch-archive-'));
  console.log(`activity archive: ${dir}`);
  const start = performance.now(), comparisons = [], cleanups = [];
  let s;
  const spring = (action, rest = {}) => s.springboard({action, ...rest});
  const elements = tree => [tree, ...(tree.children ?? []).flatMap(elements)];
  const labels = tree => tree.nodes.flatMap(node => {
    if (node.props.semanticTag === 'time' || node.type === 'time') return [];
    const label = node.props.accessibilityLabel || (node.type === 'text' ? node.lines.map(line => line.text.trimEnd()).join(' ') : '');
    return label ? [label] : [];
  });
  const record = (name, value) => writeFileSync(resolve(dir, `${name}.json`), JSON.stringify(value, null, 2));
  async function archive(name, expected, locked = false) {
    if (!expected.length) throw Error(`${name}: no stable gallery labels to compare`);
    const deadline = performance.now() + 12_000;
    let tree, missing, lockMissing;
    do {
      ({tree} = await spring('tree'));
      const nodes = elements(tree);
      missing = expected.filter(label => !nodes.some(node => node.label === label));
      lockMissing = locked && (!nodes.some(n => n.identifier === 'SBCoverSheetWindow') || !nodes.some(n => n.identifier === 'proud-lock') || !nodes.some(n => n.label === 'Swipe up to unlock'));
      if (!missing.length && !lockMissing) break;
      await Bun.sleep(100);
    } while (performance.now() < deadline);
    record(name, tree);
    simctl('io', udid, 'screenshot', resolve(dir, `${name}.png`));
    if (missing.length || lockMissing) throw Error(`${name}: after 12 s, missing exact element labels ${JSON.stringify(missing)}${lockMissing ? '; locked cover sheet was not exposed' : ''}`);
    comparisons.push({name, labels:expected});
    return tree;
  }
  async function endActivities(session) {
    await session.clock('data');
    const active = state => state.resources.activities.activities.filter(a => ['active','stale'].includes(a.activityState));
    const activities = active(await session.state());
    for (const activity of activities) {
      await session.tap(activity.kind === 'departure' ? 'end-activity' : 'end-alert');
      const deadline = performance.now() + 12_000;
      while (true) {
        await session.clock('data');
        if (!active(await session.state()).some(a => a.id === activity.id)) break;
        if (performance.now() >= deadline) throw Error(`activity ${activity.id} did not end within 12 s`);
        await Bun.sleep(100);
      }
    }
    const remaining = active(await session.state());
    if (remaining.length) throw Error(`Caltrain did not end activities: ${JSON.stringify(remaining)}`);
    return activities.length;
  }
  for (const kind of ['departure', 'service-alert']) {
    s = null;
    try {
      // App controls use agent activation; the existing drag mode also opens the SpringBoard runner.
      s = await open({host:'ios', app:'caltrain', touch:'drag'});
      await endActivities(s);
      await s.tap('change-station');
      await s.type('station-search', 'Palo');
      await s.tap('station-paloalto');
      await s.tap(kind === 'departure' ? 'start-activity' : 'start-alert');
      await s.clock('data');
      const started = (await s.state()).resources.activities.activities.find(a => a.kind === kind && ['active','stale'].includes(a.activityState));
      if (!started) throw Error(`${kind}: Caltrain did not start the activity`);
      const gallery = {};
      for (const placement of ['lockScreen','compactLeading','compactTrailing','minimal','expanded']) {
        const activity = {entry:kind, placement, state:started.state};
        gallery[placement] = await s.op({op:'tree', activity});
        record(`${kind}-gallery-${placement}`, gallery[placement]);
        await s.op({op:'screenshot', activity, path:resolve(dir, `${kind}-gallery-${placement}.png`)});
      }
      await spring('home');
      const leading = labels(gallery.compactLeading), trailing = labels(gallery.compactTrailing);
      await archive(`${kind}-compact`, [...leading, ...trailing]);
      const found = await spring('find', {label:leading[0]});
      if (!found.elements.some(n => n.label === leading[0] && n.frame[1] < 100)) throw Error(`${kind}: no compact Island element labelled ${JSON.stringify(leading[0])}`);
      await spring('hold', {point:[201,32], seconds:1.2});
      await archive(`${kind}-expanded`, labels(gallery.expanded));
      await spring('tap', {point:[20,400]});
      await spring('swipe', {point:[15,2], to:[15,650]});
      await archive(`${kind}-lockScreen`, labels(gallery.lockScreen));
      await spring('lock'); await spring('home');
      await archive(`${kind}-locked`, labels(gallery.lockScreen), true);
      await spring('swipe', {point:[200,850], to:[200,200]});
    } catch (error) {
      check(false, `${kind} archive: ${error.message}`);
    } finally {
      if (s) {
        try { simctl('terminate', udid, resolveApp('caltrain').id); }
        catch (error) { check(false, `${kind} archive terminate: ${error.message}`); }
        try { await s.close(); }
        catch (error) { check(false, `${kind} archive close: ${error.message}`); }
        let fresh;
        try {
          fresh = await open({host:'ios', app:'caltrain'});
          const ended = await endActivities(fresh);
          cleanups.push({kind, ended});
          console.log(`${kind}: fresh launch ended ${ended} activities`);
        } catch (error) { check(false, `${kind} archive cleanup: ${error.message}`); }
        finally { await fresh?.close(); }
      }
      record('result', {milliseconds:performance.now() - start, comparisons, cleanups});
    }
  }
  console.log(`activity archive: ${comparisons.length} exact-label comparisons in ${((performance.now() - start) / 1000).toFixed(1)} s`);
}
