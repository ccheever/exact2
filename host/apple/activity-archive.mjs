// @ref LLP 1126.001 WP14 — the real ActivityKit archive, in the existing ios-touch step.
// EXACT_SIM is a caller-owned iPhone 17 Pro; without it the test creates and deletes one.
import {cpSync, existsSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {open} from '../../scripts/agent.mjs';

const root = resolve(import.meta.dir, '../..');
export async function archiveActivities({build = true} = {}) {
  const dir = resolve(root, 'target/activity-archive'), kit = resolve(dir, 'archive');
  mkdirSync(resolve(kit, 'out'), {recursive:true});
  const env = {...process.env, DEVELOPER_DIR:'/Applications/Xcode.app/Contents/Developer'};
  async function run(argv, name, extra = {}) {
    const log = Bun.file(resolve(dir, `${name}.log`)), start = performance.now();
    const p = Bun.spawn(argv, {cwd:root, env:{...env, ...extra}, stdout:log, stderr:log});
    const code = await p.exited;
    if (code) throw new Error(`${name} exited ${code}: ${await log.text()}`);
    return {ms:performance.now() - start, output:await log.text()};
  }
  const simctl = async (args, name) => (await run(['xcrun','simctl',...args], name)).output.trim();
  const previousSim = process.env.EXACT_SIM;
  let sim = env.EXACT_SIM, owned = false, server, session;
  const start = performance.now(), evidence = [], failures = [];
  try {
    if (!sim) {
      sim = await simctl(['create', `exact-activity-archive-${process.pid}`, 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro', 'com.apple.CoreSimulator.SimRuntime.iOS-27-0'], 'create');
      owned = true;
      await simctl(['boot',sim], 'boot');
    }
    env.EXACT_SIM = sim; process.env.EXACT_SIM = sim;
    if (build) await run([process.execPath,'host/apple/build.mjs','--ios','--widget-gallery'], 'build');
    cpSync(resolve(root, 'host/apple/tests/archive'), resolve(kit, 'kit'), {recursive:true});
    const controllerLog = resolve(dir, 'controller.log'), control = resolve(kit, 'kit/control.mjs');
    writeFileSync(controllerLog, '');
    server = Bun.spawn([process.execPath,control,sim,'serve'], {cwd:root, env:{...env, PROBE_APP_ID:'com.exact.caltrain'}, stdout:Bun.file(controllerLog), stderr:Bun.file(resolve(dir, 'controller-error.log'))});
    let exited = false; server.exited.then(() => { exited = true; });
    const deadline = performance.now() + 90_000;
    while (!readFileSync(controllerLog,'utf8').includes('READY ') && !exited && performance.now() < deadline) await Bun.sleep(100);
    if (!readFileSync(controllerLog,'utf8').includes('READY ')) throw new Error(`SpringBoard archive controller did not start: ${readFileSync(controllerLog,'utf8')}`);
    const flow = async (steps, name, expect) => {
      await run([process.execPath,control,sim,'flow',JSON.stringify(expect ? {steps,expect} : steps),name], name);
      const result = JSON.parse(readFileSync(resolve(kit,`out/${name}.json`),'utf8'));
      if (result.error || result.extras?.error) throw new Error(`${name}: ${result.error ?? result.extras.error}`);
      return result;
    };
    const normalized = value => value.replace(/, /g,' ').replace(/\s+/g,' ').trim();
    const labels = tree => tree.nodes.flatMap(node => {
      if (node.props.semanticTag === 'time' || node.type === 'time') return [];
      const text = node.props.accessibilityLabel || (node.type === 'text' ? node.lines.map(line => line.text).join(' ') : '');
      return text ? [text] : [];
    });
    const compare = (gallery, archived, name) => {
      const expected = labels(gallery), actual = normalized(archived.tree);
      if (!expected.length) throw new Error(`${name}: no gallery labels to compare`);
      for (const label of expected) if (!actual.includes(normalized(label))) throw new Error(`${name}: archive omitted gallery label ${JSON.stringify(label)}`);
      evidence.push({name, labels:expected, seconds:archived.seconds});
    };
    for (const kind of ['departure','service-alert']) {
      try {
      session = await open({host:'ios', app:'caltrain', epoch:'2026-08-28T09:50:00Z'});
      // Clear activities left by an interrupted run through the app's own controls.
      let alive = (await session.state()).resources.activities.activities;
      for (const previous of ['departure','service-alert']) {
        if (alive.some(activity => activity.kind === previous && ['active','stale'].includes(activity.activityState))) {
          await session.tap(previous === 'departure' ? 'end-activity' : 'end-alert');
          await session.clock('data');
        }
      }
      await session.tap('change-station'); await session.clock('settle');
      await session.type('station-search','Palo'); await session.tap('station-paloalto'); await session.clock('settle');
      await session.tap(kind === 'departure' ? 'start-activity' : 'start-alert');
      await session.clock('data');
      const appState = await session.state();
      writeFileSync(resolve(dir,`${kind}-state.json`),JSON.stringify(appState,null,2));
      const started = appState.resources.activities.activities.find(activity => activity.kind === kind && ['active','stale'].includes(activity.activityState));
      if (!started) throw new Error(`${kind}: the app did not start an activity`);
      const state = JSON.parse(started.state);
      const gallery = {};
      for (const placement of ['lockScreen','compactLeading','compactTrailing','minimal','expanded']) {
        const activity = {entry:kind,placement,state:JSON.stringify(state)};
        gallery[placement] = await session.op({op:'tree',activity});
        await session.op({op:'screenshot',activity,path:resolve(dir,`${kind}-${placement}.png`)});
        writeFileSync(resolve(dir,`${kind}-${placement}.json`),JSON.stringify(gallery[placement],null,2));
      }
      const compact = await flow([{op:'home'}], `${kind}-compact`, labels(gallery[kind === 'departure' ? 'compactLeading' : 'compactTrailing'])[0]);
      compare(gallery.compactLeading,compact,`${kind}-compactLeading`);
      // The relative clock is system-owned; the service alert's static trailing label compares too.
      if (kind === 'service-alert') compare(gallery.compactTrailing,compact,`${kind}-compactTrailing`);
      const found = await flow([{op:'find',label:labels(gallery.compactLeading)[0]}],`${kind}-island`);
      const island = found.extras.frame;
      if (!island || island[1] >= 100) throw new Error(`${kind}: no compact Island accessibility element`);
      const expanded = await flow([{op:'hold',xy:[201,32],seconds:1.2}], `${kind}-expanded`, labels(gallery.expanded)[0]);
      compare(gallery.expanded,expanded,`${kind}-expanded`);
      await flow([{op:'tap',xy:[20,400]}],`${kind}-collapse`);
      const lock = await flow([{op:'swipe',xy:[15,2,15,650]}],`${kind}-lockScreen`,labels(gallery.lockScreen)[0]);
      compare(gallery.lockScreen,lock,`${kind}-lockScreen`);
      const locked = await flow([{op:'lock'},{op:'home'}],`${kind}-locked`,labels(gallery.lockScreen)[0]);
      if (!locked.tree.includes('SBCoverSheetWindow') || !locked.tree.includes('proud-lock') || !locked.tree.includes('Swipe up to unlock')) throw new Error(`${kind}: the locked read is not the Lock Screen`);
      compare(gallery.lockScreen,locked,`${kind}-locked`);
      await flow([{op:'swipe',xy:[200,850,200,200]},{op:'app'}],`${kind}-return`);
      await session.tap(kind === 'departure' ? 'end-activity' : 'end-alert');
      await session.clock('data');
      await session.close(); session = null;
      } catch (error) { failures.push(`${kind}: ${error.message}`); }
      finally {
        if (session) {
          try {
            await flow([{op:'home'},{op:'app'}],`${kind}-cleanup`);
            await session.tap(kind === 'departure' ? 'end-activity' : 'end-alert');
            await session.clock('data');
          } catch (error) { failures.push(`${kind} cleanup: ${error.message}`); }
          await session.close(); session = null;
        }
      }
    }
    const report = {milliseconds:performance.now() - start, comparisons:evidence, failures};
    writeFileSync(resolve(dir,'result.json'),JSON.stringify(report,null,2));
    if (failures.length) throw new Error(failures.join('\n'));
    console.log(`activity archive: ${evidence.length} gallery/archive comparisons in ${(report.milliseconds/1000).toFixed(1)} s`);
    return report;
  } finally {
    await session?.close();
    const state = resolve(kit,'build/control.json');
    if (server && existsSync(state)) {
      const recorded = JSON.parse(readFileSync(state,'utf8'));
      if (recorded.serverPid === server.pid) { try { process.kill(recorded.xcodebuildPid,'SIGTERM'); } catch {} }
    }
    server?.kill();
    if (owned) { try { await simctl(['shutdown',sim],'shutdown'); } finally { await simctl(['delete',sim],'delete'); } }
    if (previousSim === undefined) delete process.env.EXACT_SIM; else process.env.EXACT_SIM = previousSim;
  }
}
if (import.meta.main) await archiveActivities({build:!process.argv.includes('--no-build')});
