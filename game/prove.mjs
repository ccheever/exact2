#!/usr/bin/env bun
// Orchestrate the game's existing proof; every drive still uses the eight operations.
import {snapshot, validateSnapshot, validateProof, reusableLeg, digest, writeJSON, inventory, assertInventory} from './proof-snapshot.mjs';
import {startStatus} from './status.mjs';
import {spawn, spawnSync} from 'node:child_process';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync} from 'node:fs';
import {basename, isAbsolute, relative, resolve} from 'node:path';
import {equal, agreePins, nativeProofHost, pinInputs, pinSourceInputs, pinRevision, webUnavailable, paranoidRuns, proofCommand, lookAcrossRuns, lookOnlyFailure} from './proof.mjs';
import {gameDefaults, lintGame, prepareGame} from './app/shells.mjs';

const [destination, ...args] = process.argv.slice(2);
const local = destination === '.' || (destination && isAbsolute(destination)) || destination?.includes('/') || destination?.includes('\\');
const name = local ? basename(resolve(destination)) : destination;
const option = (flag, fallback) => args.includes(flag) ? args[args.indexOf(flag) + 1] : fallback;
const device = args.includes('--device'), phone = option('--phone');
if (args.includes('--phone') && (!device || !phone || phone.startsWith('--'))) throw new Error('--phone requires --device and a device name or identifier');
let repin = args.includes('--repin');
const repeat = Number(option('--repeat', '1'));
if (!/^[a-z][a-z0-9-]*$/.test(name ?? '') || !Number.isSafeInteger(repeat) || repeat < 1) {
  throw new Error('Usage: bun game/prove.mjs <name|path> --hosts web,linux,windows,ios --repeat 2 --compare-saves');
}
const sourceApp = local ? resolve(destination) : resolve(import.meta.dir, 'games', name);
let app = sourceApp;
const script = resolve(app, 'proof.mjs');
const continuation = option('--continue');
const continuedRoot = continuation ? resolve(existsSync(continuation) ? continuation : resolve(app, 'artifacts/prove', continuation)) : null;
const saved = continuedRoot ? JSON.parse(readFileSync(resolve(continuedRoot, 'legs.json'), 'utf8')) : null;
if (saved && repin && !saved.repin) throw new Error('continue refused: repin changed');
if (saved) repin = saved.repin;
if (!existsSync(script)) throw new Error(`No proof for ${name}`);
const progress = startStatus(app, 'prove');
const pinFile = resolve(app, 'pins.json');
const previous = saved?.before ?? JSON.parse(readFileSync(pinFile, 'utf8'));
// A first baseline is accepted only after the same all-mode/host agreement as repin.
const firstPins = !Object.keys(previous.ticks ?? {}).length && !Object.keys(previous.saves ?? {}).length;
if (firstPins && repin && !saved) throw new Error('omit `--repin` for the first baseline');
if (firstPins && !repin) {
  console.log('No pins yet; filling the first baseline after every requested host and mode agrees.');
  repin = true;
}
const defaultNative = process.platform === 'win32' ? 'windows' : 'linux';
const hosts = option('--hosts', saved?.hosts?.join(',') ?? (repin || args.includes('--compare-saves') ? `${defaultNative},web` : defaultNative)).split(',');
if (!hosts.length || new Set(hosts).size !== hosts.length || hosts.some(h => !['web','linux','windows','macos','ios'].includes(h))) {
  throw new Error('Use --hosts linux,windows,web,macos,ios to select distinct proof hosts');
}
const native = nativeProofHost(hosts);
if (firstPins && (!native || !hosts.includes('web'))) throw new Error('first baseline requires a native host (linux or windows) and web');
if (device && !hosts.includes('ios')) throw new Error('--device requires ios in --hosts');
const artifacts = resolve(app, 'artifacts/prove');
mkdirSync(artifacts, {recursive:true});
const root = continuedRoot ?? mkdtempSync(resolve(artifacts, 'run-'));
const config = {hosts, repeat, repin, compareSaves:args.includes('--compare-saves'), device, phone:phone ?? null, env:Object.fromEntries(['RUSTUP_TOOLCHAIN','RUSTFLAGS','CARGO_TARGET_DIR','CARGO_BUILD_BUILD_DIR','EXACT_UPDATE_TRUST','EXACT_GAME_PARANOID_COMPARE'].map(key => [key,process.env[key] ?? null]))};
if (saved && JSON.stringify(saved.config) !== JSON.stringify(config)) throw new Error('continue refused: requested host/mode/profile matrix or device changed');
if (saved) validateSnapshot(saved.snapshot, resolve(import.meta.dir, '..'), sourceApp);
const record = saved ?? {version:1, before:previous, repin, hosts, config, snapshot:snapshot(resolve(import.meta.dir, '..'), sourceApp, root), legs:{}};
app = record.snapshot.src;
const checkSDK = () => {
  const sdkOnly = values => Object.fromEntries(Object.entries(values).filter(([name]) => name.startsWith('sdk/')));
  assertInventory(sdkOnly(record.snapshot.inputs), sdkOnly(inventory(resolve(import.meta.dir, '..'), sourceApp)));
  validateProof(record.snapshot, resolve(import.meta.dir, '..'), sourceApp);
};
checkSDK();
writeJSON(resolve(root, 'legs.json'), record);
console.log(`ARTIFACTS ${root}`);
const children = new Set();
let interrupted;
const signals = ['SIGINT','SIGTERM'].map(signal => [signal, async () => {
  if (interrupted) return;
  interrupted = signal;
  await Promise.all([...children].map(child => new Promise(ok => { child.once('exit',ok); child.kill(signal); })));
  for (const [name, listener] of signals) process.off(name,listener);
  process.kill(process.pid, signal);
}]);
for (const [signal, listener] of signals) process.on(signal,listener);
const buildTarget = resolve(process.env.CARGO_TARGET_DIR ?? resolve(sourceApp,'target'));
const buildEnvironment = {CARGO_TARGET_DIR:buildTarget, CARGO_BUILD_BUILD_DIR:resolve(process.env.CARGO_BUILD_BUILD_DIR ?? buildTarget)};
const run = async (host, index, build = false, mode = '0', profile = 'gpu-dev') => {
  progress.set({phase:`${host} ${mode} ${profile} ${build ? 'build' : 'proof'}`});
  checkSDK();
  const key = `${host}-${mode}-${profile}-${build ? 'build' : index}`;
  const reuse = !build && reusableLeg(record.legs[key], root, record.snapshot.digest);
  if (reuse) { console.log(`REUSE ${key}`); return {...reuse, repeat:index, profile}; }
  const destination = host === 'ios' && device ? 'ios-device' : host;
  const out = resolve(root, `${destination}-${mode}-${build ? 'build' : index}${profile === 'release' ? '-release' : ''}`);
  mkdirSync(out, {recursive:true});
  record.legs[key] = {host, mode, profile, digest:record.snapshot.digest, state:'running', startedAt:Date.now(), pins:null, verdict:null};
  writeJSON(resolve(root, 'legs.json'), record);
  // Resolve an external entrypoint in its own directory, without Cargo metadata.
  const child = spawn(process.execPath, ['./proof.mjs', host, ...(host === 'ios' && device ? ['--device', ...(phone ? ['--phone', phone] : [])] : []), ...(build ? ['--build-only'] : [])], {
    cwd:app,
    env:{...process.env, EXACT_APP_DIR:app, EXACT_PROOF_NAME:name, ...buildEnvironment, EXACT_PROOF_BUILD_OUT:resolve(sourceApp,'artifacts/build'), EXACT_WEB_DIST:resolve(root,'dist'), EXACT_PROOF_OUT:out, EXACT_PROOF_COMPARE:build || repin ? '0' : '1', EXACT_PROOF_REPIN:repin ? '1' : '0', EXACT_GAME_PARANOID:mode, EXACT_GAME_PROOF_PROFILE:profile},
    stdio:['ignore','pipe','pipe'],
  });
  let log = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {
    log += bytes;
    // Cold compilation must remain observable to callers' inactivity deadlines.
    if (stream === child.stderr) process.stderr.write(bytes);
  });
  children.add(child);
  const code = await new Promise((ok, reject) => {child.on('exit', (code, signal) => ok(code ?? 128)); child.on('error', reject);});
  children.delete(child);
  if (interrupted) await new Promise(() => {});
  checkSDK();
  writeFileSync(resolve(out, 'run.log'), log);
  const summaryPath = resolve(out, 'summary.json');
  const summary = existsSync(summaryPath) ? {...JSON.parse(readFileSync(summaryPath, 'utf8')), repeat:index, profile} : null;
  record.legs[key] = {...record.legs[key], durationMs:Date.now()-record.legs[key].startedAt, state:[0,3].includes(code) && summary ? 'complete' : 'failed', exitCode:code, verdict:summary?.status ?? 'FAIL', pins:summary?.pins, summaryPath:relative(root, summaryPath), summaryDigest:summary ? digest(readFileSync(summaryPath)) : null};
  writeJSON(resolve(root, 'legs.json'), record);
  if (args.includes('--report') && (!build || code !== 0) && summary) for (const hint of summary.facilities ?? []) console.log(`REPORT ${host} ${mode}: ${hint}`);
  if (![0,3].includes(code)) throw Object.assign(new Error(`${host} mode ${mode} ${build ? 'build' : index} failed: ${out}/run.log\n${log.slice(-2500)}`), {summary, webUnavailable:host === 'web' && webUnavailable(log)});
  return summary;
};
if (repin) {
  // A baseline records only a game that holds the determinism contract.
  const game = gameDefaults(app)?.game;
  if (game) {
    prepareGame(app, game);
    lintGame(app, game, {env:{...process.env, ...buildEnvironment}});
  }
  const pinFile = resolve(sourceApp, 'pins.json'), before = JSON.parse(readFileSync(pinFile, 'utf8'));
  const rows = [], errors = [], exercised = [];
  if (!native) throw new Error('repin requires a native host; use --hosts linux,web or --hosts windows,web');
  // A single web dist is mode-specific: bake and run each mode serially.
  for (const host of hosts) {
    let unavailable = false;
    await paranoidRuns(async mode => {
      if (unavailable) return 0;
      try { rows.push(await run(host, 1, false, mode)); return 0; }
      catch (error) {
        // A look that failed is the look's: the pins record the simulation, which passed.
        if (lookOnlyFailure(error.summary)) { rows.push(error.summary); return 0; }
        if (mode === '0' && error.webUnavailable) { unavailable = true; console.log('WEB unavailable: repin refuses the missing requested host; set CHROME and rerun.'); }
        else errors.push(error);
        return 1;
      }
    }, async () => {
      if (!unavailable) try { await run(host, 0, true); } catch (error) { errors.push(error); return 1; }
      return 0;
    }, host);
    if (!unavailable) exercised.push(host);
  }
  try { rows.push(await run(native, 1, false, '0', 'release')); }
  catch (error) { if (lookOnlyFailure(error.summary)) rows.push(error.summary); else errors.push(error); }
  for (const error of errors) console.error(error.message);
  if (errors.length) throw new Error(`repin refused: mode/host proof failed; pins.json unchanged; inspect ${root}/*/run.log and rerun the named proof with --paranoid`);
  checkSDK();
  const candidate = agreePins(rows, before, hosts, app);
  const command = record.command ??= proofCommand(import.meta.path, local ? sourceApp : name, ...(args.includes('--repin') ? ['--repin'] : []), '--hosts', exercised.join(','), ...(device ? ['--device'] : []), ...(phone ? ['--phone', phone] : []));
  const inputs = pinInputs(rows), sourceInputs = pinSourceInputs(rows);
  const after = {...candidate, inputs, sourceInputs, game:previous.game ?? name, generated:command, at:record.at ??= pinRevision(sourceApp, sourceInputs), ...(option('--reason', '') ? {reason:option('--reason', '')} : {})};
  for (const section of ['ticks', 'saves']) for (const [key, value] of Object.entries(after[section]))
    console.log(`${section} ${key}: ${before[section]?.[key] ?? '(new)'} → ${value}`);
  if (!exercised.includes('web')) console.log(`WEB not exercised; pins record ${exercised.join(',')} only, no web agreement claimed.`);
  writeJSON(resolve(root, 'legs.json'), record);
  writeJSON(pinFile, after);
  console.log(`PROOF PASS ${name}: baseline published after host/mode agreement`);
} else {
// Each proof checks its own build receipt. Only multiple hosts need a separate
// serial bake phase before their independent processes can run in parallel.
const buildFailures = new Map();
for (const host of hosts.length > 1 ? hosts : []) {
  try { await run(host, 0, true); } catch(error) { buildFailures.set(host,error); }
}
const groups = await Promise.allSettled(hosts.map(async host => {
  if (buildFailures.has(host)) throw buildFailures.get(host);
  const rows = [];
  for (let index = 1; index <= repeat; index++) {
    try { rows.push(await run(host, index)); }
    catch (error) { error.rows = [...rows, ...(error.summary ? [error.summary] : [])]; throw error; }
  }
  return rows;
}));
const failures = groups.filter(r => r.status === 'rejected');
const rows = groups.flatMap(r => r.status === 'fulfilled' ? r.value : r.reason.rows ?? (r.reason.summary ? [r.reason.summary] : []));
const hashes = row => row.worlds.map(({session, tick, hash}) => ({session, tick, hash}));
const baseline = rows[0];
// A run that failed by its look alone is the look's failure, below, not the simulation's.
let failed = failures.some(failure => !lookOnlyFailure(failure.reason.summary));
console.log('| Host | Run | Seconds | World hashes | Save bytes | Simulation | Look |');
console.log('|---|---:|---:|---|---|---|---|');
for (const row of rows) {
  const hashOK = row.worlds.length > 0 && equal(hashes(row), hashes(baseline));
  const saveOK = row.saves.length > 0 && equal(row.saves, baseline.saves);
  failed ||= !hashOK || (config.compareSaves && !saveOK);
  console.log(`| ${row.device ? 'ios-device' : row.host} | ${row.repeat} | ${row.seconds.toFixed(3)} | ${hashOK ? 'equal' : 'FAIL'} | ${config.compareSaves ? (saveOK ? 'identical' : 'FAIL') : 'not requested'} | ${row.simulation ?? row.status ?? 'UNVERIFIED'} | ${row.look ? `${row.look.status}, ${row.look.detail}` : 'not taken'} |`);
}
// The look result (LLP 1127.000 D5): a tell no game.look.silence names fails the proof; it passes only when every
// requested host's every run saw every world it showed, whole. A run with no summary is a gap.
const looks = rows.map(row => row.look);
for (const look of looks.filter(Boolean)) for (const picture of [...look.pages ?? [], ...look.pictures ?? []]) console.log(`LOOK ${picture}`);
const expected = hosts.flatMap(host => Array.from({length:repeat}, (_, i) => ({host, repeat:i + 1})));
const {status:lookStatus, gaps:lookGaps} = lookAcrossRuns(rows, expected);
for (const gap of lookGaps) console.log(`look: unverified, ${gap}`);
for (const failure of failures) console.error(failure.reason);
if (hosts.length > 1) {
  const entry = await import(resolve(app, 'proof.mjs'));
  if (entry.compare) try { await entry.compare(rows, {root, repeat}); }
  catch (error) { console.error(error.message); failed = true; }
}
const status = failed || lookStatus === 'FAIL' ? 'FAIL' : rows.length && rows.every(row => row.status === 'PASS') ? 'PASS' : 'UNVERIFIED';
console.log(`simulation: ${failed ? 'FAIL' : rows.length && rows.every(row => (row.simulation ?? row.status) === 'PASS') ? 'PASS' : 'UNVERIFIED'} · look: ${lookStatus}${looks.some(Boolean) ? '' : ', no look taken'}`);
console.log(`PROOF ${status} ${name}`);
if (status === 'UNVERIFIED') console.log(`No complete tick/save baseline was checked. Generate it with ${proofCommand(import.meta.path, local ? sourceApp : name, '--repin')}`);
writeFileSync(resolve(root, 'summary.json'), JSON.stringify({status, look:lookStatus, lookGaps, rows}, null, 2)+'\n');
process.exitCode = status === 'PASS' ? 0 : status === 'FAIL' ? 1 : 3;

}
