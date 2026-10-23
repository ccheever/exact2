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
// The final activity archive phase requires --build for its gallery variant;
// without it, an installed app without the gallery skips that phase with a reason.
//   bun scripts/smoke-touch.mjs [--build] [--sim <udid|name>]
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from './agent.mjs';
import { HOST_DEV, resolveApp } from './app.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { chooseSimulator } from '../host/apple/devices.mjs';
import { openTouches } from '../host/apple/touches.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
const shell = (command, args) => {
  const result = spawnSync(command, args, {cwd: ROOT, encoding: 'utf8', timeout:15_000});
  if (result.status !== 0) throw Error(`${command} ${args.join(' ')}: ${result.error?.message || result.stderr || result.stdout}`);
  return result.stdout.trim();
};
const simctl = (...args) => shell('xcrun', ['simctl', ...args]);
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const t0 = Date.now();
const IPHONE_17_PRO = 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro';
let ownedSim, usedTouches = false;
try {
  if (!process.env.EXACT_SIM) {
    const sdk = shell('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-version']);
    const version = (a, b) => a.localeCompare(b, undefined, {numeric:true});
    const runtime = JSON.parse(simctl('list', 'runtimes', '-j')).runtimes.filter(r => r.isAvailable && r.identifier.includes('.iOS-') && version(r.version, sdk) <= 0 && r.supportedDeviceTypes?.some(t => t.identifier === IPHONE_17_PRO)).sort((a, b) => version(b.version, a.version))[0];
    if (!runtime) throw Error(`ios-touch needs an available iOS runtime, no newer than the selected simulator SDK ${sdk}, that runs iPhone 17 Pro`);
    ownedSim = simctl('create', `exact-ios-touch-${process.pid}`, IPHONE_17_PRO, runtime.identifier);
    process.env.EXACT_SIM = ownedSim;
  }
  const device = chooseSimulator(process.env.EXACT_SIM);

  console.log(`ios-touch: ${device.name} ${device.udid}, ${device.runtime}`);
if (argv.includes('--build')) {
  const built = spawnSync('bun', ['host/apple/build.mjs', '--ios', 'caltrain-apple', '--no-install'], { cwd: ROOT, stdio: 'inherit' });
  if (built.status !== 0) throw Error('FAIL ios-touch build: host/apple/build.mjs --ios caltrain-apple --no-install');
  // WP9's refusal scan uses this iOS app, its extension and its packaged arm.
  const bundle = appleArtifacts(resolveApp('caltrain'), { destination: 'ios-simulator' }).bundle;
  const refused = spawnSync('bun', ['test', 'host/apple/link.test.mjs'], { cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_ACTIVITY_APP: bundle } });
  if (refused.status !== 0) throw Error('FAIL ios-touch activity refusals');
}
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
for (const timing of ['agent', 'platform']) {
  const opened = Date.now();
  let s;
  try { s = await open({ host: 'ios', app: 'caltrain', touch: 'platform', timing }); usedTouches = true; }
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
    const checkpoints = [];
    const heldRow = async () => { const y = await row(); checkpoints.push(y); return y; };
    const reversed = await s.tap('stations-screen', { drag: {
      dx: 0, dy: 120, over: 300, hold: 500, during: [heldRow],
      path: [{ dx: 0, dy: -120, over: 300, hold: 500, during: [heldRow] }],
    } });
    check(reversed.delivery === 'platform' && reversed.touch?.moved > 0 &&
      Math.abs(reversed.touch?.travel?.[1]) < 2 && checkpoints.length === 2 && checkpoints[0] - checkpoints[1] > 70,
      `${timing}: held reversal did not keep one touch and move back: ${JSON.stringify({reversed, checkpoints})}`);
    console.log(`${timing}: held reversal checkpoints ${JSON.stringify(checkpoints)}, travel ${JSON.stringify(reversed.touch?.travel)}`);

    const stationary = [];
    const held = await s.tap('stations-screen', { drag: {
      dx: 0, dy: -120, over: 300,
      path: [{ dx: 0, dy: 0, hold: 500, during: [async () => {
        await s.clock('+50 real');
        stationary.push(await row());
        await s.clock('+200 real');
        stationary.push(await row());
      }] }],
    } });
    const lifted = await row();
    await s.clock('settle');
    const rested = await row();
    check(held.delivery === 'platform' && Math.abs(stationary[1] - stationary[0]) < 3 && Math.abs(rested - lifted) < 3,
      `${timing}: a final stationary leg holds and releases at rest: ${JSON.stringify({stationary, lifted, rested})}`);
    console.log(`${timing}: stationary hold ${JSON.stringify(stationary)}, release ${lifted} → ${rested}`);

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
      s = await open({ host: 'ios', app: 'caltrain', plan, touch: 'platform' }); usedTouches = true;
      // Settle the launch first: a real tap during it can land on a switch that does not act yet
      await s.clock('settle');
      const facts = async () => byTestId(await s.tree(), 'facts')?.props.text;
      const tap = async (target) => {
        const r = await s.tap(target);
        check(r.delivery === 'platform' && r.touch?.type === 'direct', `grouped: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
        // When the window had the down, which `downAfterLift` below compares with the lift
        check(typeof r.touch?.dispatched === 'number', `grouped: tap ${target}'s reply has no time the window had its down: ${JSON.stringify(r.touch)}`);
        return r;
      };
      let was = 'false 0 0';
      for (const [target, want] of [['toggle', 'true 0 0'], ['toggle', 'false 0 0'], ['info', 'false 1 0'], ['open', 'false 1 1']]) {
        let r = await tap(target);
        // A switch whose down reaches the window after the lift gets one retry;
        // detail buttons must keep their control through the whole contact.
        const dropped = target === 'toggle' && r.downAfterLift
          ? `the window had the switch's down ${Math.round((r.touch.dispatched - r.touch.ended) * 1000)} ms after the lift` : null;
        if (dropped) {
          await s.clock('settle');
          if ((await facts()) === was) { console.log(`grouped: ${dropped}; tapped again`); r = await tap(target); }
        }
        was = want;
        await s.clock('settle');
        const seen = await facts();
        check(seen === want, `grouped: after a real tap on ${target} the facts read ${seen}, not ${want} (landed ${JSON.stringify(r.landed)}${r.viewGone ? ', its view gone before the lift' : ''}${r.downAfterLift ? ', its down after the lift' : ''}, touch ${JSON.stringify(r.touch)})`);
      }
      // Loading accessibility can replace a native list accessory under a held finger.
      await s.tree(undefined, { ax: true });
      for (let count = 2; count <= 9; count++) {
        const r = await s.tap('info', { drag: { dx: 0, dy: 0, hold: 300 } });
        await s.clock('settle');
        const seen = await facts();
        check(r.delivery === 'platform' && !r.viewGone && seen === `false ${count} 1`,
          `grouped: held detail touch ${count} read ${seen}, viewGone=${!!r.viewGone}`);
      }
      console.log('grouped: a real switch, detail button and row tap each landed on UIKit\'s cell');
    } catch (error) {
      check(false, `grouped: ${error.message}`);
    } finally { await s?.close(); }
  }
  // A command-only invoker remains activatable when UIKit hides its authored grouped row.
  if (c.status === 0) for (const touch of ['agent', 'platform']) {
    const s = await open({ host: 'ios', app: 'caltrain', plan, touch });
    try {
      await s.clock('settle');
      const tap = await s.tap('delete-item');
      await s.clock('settle');
      const shown = (await s.state()).navigation?.popover;
      check(shown?.kind === 'confirmation' && shown.phase === 'open', `grouped modal (${touch}): the command opened a confirmation: ${JSON.stringify(shown)}`);
      await s.tap('remove-confirm');
      await s.clock('settle');
      const state = await s.state();
      check(state.slots.removes === 1 && state.navigation.popover == null, `grouped modal (${touch}): confirm removes once and closes`);
      console.log(`grouped modal (${touch}): ${tap.delivery}, removes ${state.slots.removes}`);
    } catch (error) { check(false, `grouped modal (${touch}): ${error.message}`); }
    finally { await s.close(); }
  }
}

// No foreground drive follows the archive: only this phase needs the gallery build.
if (argv.includes('--build')) {
  const built = spawnSync('bun', ['host/apple/build.mjs', '--ios', 'caltrain-apple', '--widget-gallery', '--no-install'], { cwd: ROOT, stdio: 'inherit' });
  if (built.status !== 0) throw Error('activity archive: the gallery build failed');
}
if (device.type === 'iPhone-17-Pro') await archiveActivities(device.udid, check);
else console.log(`activity archive: skipped on ${device.type}; Island and Lock Screen points require iPhone 17 Pro`);
} catch (error) {
  check(false, error.message);
} finally {
  if (ownedSim) {
    if (usedTouches) {
      let runner;
      try {
        runner = await openTouches({udid:ownedSim, appId:resolveApp('caltrain').id});
        const reply = await runner.ask({op:'quit'});
        check(reply.done === true, `owned simulator touch runner quit: ${reply.error ?? JSON.stringify(reply)}`);
      } catch (error) { check(false, `owned simulator touch runner quit: ${error.message}`); }
      finally { if (runner) { const error = await runner.end(); if (error) check(false, `owned simulator touch runner end: ${error.message}`); } }
    }
    // Shutting down a device already shut down is CoreSimulator error 405, not a failure.
    try { simctl('shutdown', ownedSim); }
    catch (error) { if (!/current state: Shutdown/.test(error.message)) check(false, `owned simulator shutdown: ${error.message}`); }
    try { simctl('delete', ownedSim); }
    catch (error) { check(false, `owned simulator delete: ${error.message}`); }
    delete process.env.EXACT_SIM;
  }
}
console.log(`touch smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
for (const f of failures) console.error(`FAIL ios-touch ${f}`);
process.exitCode = failures.length ? 1 : 0;

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
  async function archive(name, expected, {cover = false, locked = false, islandLabel} = {}) {
    if (!expected.length) throw Error(`${name}: no stable gallery labels to compare`);
    const deadline = performance.now() + 12_000;
    let tree, missing, coverMissing, lockMissing, islandMissing;
    do {
      ({tree} = await spring('tree'));
      const nodes = elements(tree);
      missing = expected.filter(label => !nodes.some(node => node.label === label));
      coverMissing = (cover || locked) && !nodes.some(n => n.identifier === 'SBCoverSheetWindow');
      lockMissing = locked && (!nodes.some(n => n.identifier === 'proud-lock') || !nodes.some(n => n.label === 'Swipe up to unlock'));
      islandMissing = islandLabel && !nodes.some(n => n.identifier.startsWith('jindo-container-view:') && elements(n).some(label => label.label === islandLabel && label.frame[2] > 0 && label.frame[3] > 0));
      if (!missing.length && !coverMissing && !lockMissing && !islandMissing) break;
      await Bun.sleep(100);
    } while (performance.now() < deadline);
    record(name, tree);
    simctl('io', udid, 'screenshot', resolve(dir, `${name}.png`));
    const valid = !missing.length && !coverMissing && !lockMissing && !islandMissing;
    check(valid, `${name}: after 12 s, missing exact element labels ${JSON.stringify(missing)}${coverMissing ? '; cover sheet was not exposed' : ''}${lockMissing ? '; locked state was not exposed' : ''}${islandMissing ? `; leading label ${JSON.stringify(islandLabel)} is not shown in the Island` : ''}`);
    if (valid) comparisons.push({name, labels:expected});
    return tree;
  }
  const alive = activity => ['active','stale'].includes(activity.activityState);
  async function waitActivities(session, accepts, failure) {
    const deadline = performance.now() + 12_000;
    let timer;
    try {
      return await Promise.race([
        (async () => {
          while (performance.now() < deadline) {
            const clock = await session.clock('data'), activities = (await session.state()).resources.activities.activities;
            if (clock.settled !== false && accepts(activities)) return activities;
            await Bun.sleep(100);
          }
          throw Error(failure);
        })(),
        new Promise((_, reject) => { timer = setTimeout(() => reject(Error(failure)), 12_000); }),
      ]);
    } finally { clearTimeout(timer); }
  }
  async function endActivity(session, id) {
    const activities = await waitActivities(session, list => list.some(a => a.id === id && ['active','stale','ended','dismissed'].includes(a.activityState)), `activity ${id} never appeared in the fresh launch within 12 s`);
    const activity = activities.find(a => a.id === id);
    if (!alive(activity)) return {id, observed:activity.activityState, ended:0, final:activity.activityState};
    // Caltrain's end button addresses the first live activity of its kind.
    if (activities.find(a => a.kind === activity.kind && alive(a))?.id !== id) throw Error(`activity ${id} is not the one Caltrain's end button addresses`);
    await session.tap(activity.kind === 'departure' ? 'end-activity' : 'end-alert');
    const remaining = await waitActivities(session, list => !list.some(a => a.id === id && alive(a)), `activity ${id} is still active or stale, or its end did not settle within 12 s`);
    return {id, observed:activity.activityState, ended:1, final:remaining.find(a => a.id === id)?.activityState ?? 'absent'};
  }
  async function endActivities(session) {
    const activities = await waitActivities(session, () => true, 'opening activity read did not settle within 12 s');
    const live = activities.filter(alive);
    for (const activity of live) await endActivity(session, activity.id);
    return live.length;
  }
  for (const kind of ['departure', 'service-alert']) {
    s = null;
    let started;
    try {
      // App controls use agent activation; the existing drag mode also opens the SpringBoard runner.
      s = await open({host:'ios', app:'caltrain', touch:'drag'}); usedTouches = true;
      if (!argv.includes('--build')) {
        try { await s.op({op:'state', activity:{entry:kind}, availability:true}); }
        catch (error) {
          if (!error.message.includes('widget gallery is unavailable')) throw error;
          console.log('activity archive: skipped; the installed app has no gallery (--build is required for the archive phase)');
          return;
        }
      }
      const opening = await endActivities(s);
      if (opening) throw Error(`${kind}: opening cleanup ended ${opening} unexpected activities`);
      console.log(`${kind}: opening cleanup found no live activities`);
      await s.tap('change-station');
      await s.type('station-search', 'Palo');
      await s.tap('station-paloalto');
      await s.tap(kind === 'departure' ? 'start-activity' : 'start-alert');
      await s.clock('data');
      started = (await s.state()).resources.activities.activities.find(a => a.kind === kind && ['active','stale'].includes(a.activityState));
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
      await archive(`${kind}-compact`, [...leading, ...trailing], {islandLabel:kind === 'departure' ? leading[0] : undefined});
      const found = await spring('find', {label:leading[0]});
      if (!found.elements.some(n => n.label === leading[0] && n.frame[2] > 0 && n.frame[3] > 0 && n.frame[1] < 100)) throw Error(`${kind}: no compact Island element labelled ${JSON.stringify(leading[0])}`);
      await spring('hold', {point:[201,32], seconds:1.2});
      await archive(`${kind}-expanded`, labels(gallery.expanded));
      await spring('tap', {point:[20,400]});
      await spring('swipe', {point:[15,2], to:[15,650]});
      await archive(`${kind}-lockScreen`, labels(gallery.lockScreen), {cover:true});
      await spring('lock'); await spring('home');
      await archive(`${kind}-locked`, labels(gallery.lockScreen), {locked:true});
    } catch (error) {
      check(false, `${kind} archive: ${error.message}`);
    } finally {
      if (s) {
        let restored = false;
        try {
          const covered = tree => elements(tree).some(n => n.identifier === 'SBCoverSheetWindow');
          if (covered((await spring('tree')).tree)) await spring('swipe', {point:[200,850], to:[200,200]});
          else await spring('home');
          const tree = (await spring('tree')).tree;
          if (covered(tree)) throw Error('SBCoverSheetWindow is still present after the unlock swipe');
          if (!elements(tree).some(n => n.identifier === 'Home screen icons')) throw Error('the Home Screen was not exposed');
          restored = true;
          console.log(`${kind}: SpringBoard restored to Home Screen`);
        } catch (error) { check(false, `${kind} archive restore SpringBoard: ${error.message}`); }
        try { simctl('terminate', udid, resolveApp('caltrain').id); }
        catch (error) { check(false, `${kind} archive terminate: ${error.message}`); }
        try { await s.close(); }
        catch (error) { check(false, `${kind} archive close: ${error.message}`); }
        let fresh;
        try {
          if (started) {
            if (!restored) throw Error(`activity ${started.id}: fresh cleanup requires the Home Screen`);
            fresh = await open({host:'ios', app:'caltrain'});
            const result = await endActivity(fresh, started.id);
            cleanups.push({kind, ...result});
            console.log(`${kind}: fresh launch cleanup ${JSON.stringify(result)}`);
          }
        } catch (error) { check(false, `${kind} archive cleanup: ${error.message}`); }
        finally { await fresh?.close(); }
      }
      record('result', {milliseconds:performance.now() - start, comparisons, cleanups});
    }
  }
  console.log(`activity archive: ${comparisons.length} exact-label comparisons in ${((performance.now() - start) / 1000).toFixed(1)} s`);
}
