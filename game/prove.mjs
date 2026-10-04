#!/usr/bin/env bun
// Orchestrate the game's existing proof; every drive still uses the eight operations.
import {spawn, spawnSync} from 'node:child_process';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync} from 'node:fs';
import {basename, isAbsolute, resolve} from 'node:path';
import {equal, agreePins, nativeProofHost, pinInputs, pinRevision, webUnavailable, paranoidRuns, proofCommand} from './proof.mjs';
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
const app = local ? resolve(destination) : resolve(import.meta.dir, 'games', name), script = resolve(app, 'proof.mjs');
if (!existsSync(script)) throw new Error(`No proof for ${name}`);
const pinFile = resolve(app, 'pins.json');
const previous = JSON.parse(readFileSync(pinFile, 'utf8'));
// A first baseline is accepted only after the same all-mode/host agreement as repin.
const firstPins = !Object.keys(previous.ticks ?? {}).length && !Object.keys(previous.saves ?? {}).length;
if (firstPins && repin) throw new Error('omit `--repin` for the first baseline');
if (firstPins && !repin) {
  console.log('No pins yet; filling the first baseline after every requested host and mode agrees.');
  repin = true;
}
const defaultNative = process.platform === 'win32' ? 'windows' : 'linux';
const hosts = option('--hosts', repin || args.includes('--compare-saves') ? `${defaultNative},web` : defaultNative).split(',');
if (!hosts.length || new Set(hosts).size !== hosts.length || hosts.some(h => !['web','linux','windows','macos','ios'].includes(h))) {
  throw new Error('Use --hosts linux,windows,web,macos,ios to select distinct proof hosts');
}
const native = nativeProofHost(hosts);
if (firstPins && (!native || !hosts.includes('web'))) throw new Error('first baseline requires a native host (linux or windows) and web');
if (device && !hosts.includes('ios')) throw new Error('--device requires ios in --hosts');
const artifacts = resolve(app, 'artifacts/prove');
mkdirSync(artifacts, {recursive:true});
const root = mkdtempSync(resolve(artifacts, 'run-'));
console.log(`ARTIFACTS ${root}`);
const run = async (host, index, build = false, mode = '0', profile = 'gpu-dev') => {
  const destination = host === 'ios' && device ? 'ios-device' : host;
  const out = resolve(root, `${destination}-${mode}-${build ? 'build' : index}${profile === 'release' ? '-release' : ''}`);
  mkdirSync(out, {recursive:true});
  // Resolve an external entrypoint in its own directory, without Cargo metadata.
  const child = spawn(process.execPath, ['./proof.mjs', host, ...(host === 'ios' && device ? ['--device', ...(phone ? ['--phone', phone] : [])] : []), ...(build ? ['--build-only'] : [])], {
    cwd:app,
    env:{...process.env, EXACT_WEB_DIST:resolve(root,'dist'), EXACT_PROOF_OUT:out, EXACT_PROOF_COMPARE:build || repin ? '0' : '1', EXACT_PROOF_REPIN:repin ? '1' : '0', EXACT_GAME_PARANOID:mode, EXACT_GAME_PROOF_PROFILE:profile},
    stdio:['ignore','pipe','pipe'],
  });
  let log = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {
    log += bytes;
    // Cold compilation must remain observable to callers' inactivity deadlines.
    if (stream === child.stderr) process.stderr.write(bytes);
  });
  const code = await new Promise((ok, reject) => {child.on('exit', ok); child.on('error', reject);});
  writeFileSync(resolve(out, 'run.log'), log);
  const summaryPath = resolve(out, 'summary.json');
  const summary = existsSync(summaryPath) ? {...JSON.parse(readFileSync(summaryPath, 'utf8')), repeat:index, profile} : null;
  if (args.includes('--report') && (!build || code !== 0) && summary) for (const hint of summary.facilities ?? []) console.log(`REPORT ${host} ${mode}: ${hint}`);
  if (code !== 0) throw Object.assign(new Error(`${host} mode ${mode} ${build ? 'build' : index} failed: ${out}/run.log\n${log.slice(-2500)}`), {summary, webUnavailable:host === 'web' && webUnavailable(log)});
  return summary;
};
if (repin) {
  // A baseline records only a game that holds the determinism contract.
  const game = gameDefaults(app)?.game;
  if (game) {
    prepareGame(app, game);
    lintGame(app, game, {env:{...process.env, CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR ?? resolve(app, 'target')}});
  }
  const pinFile = resolve(app, 'pins.json'), before = JSON.parse(readFileSync(pinFile, 'utf8'));
  const rows = [], errors = [], exercised = [];
  if (!native) throw new Error('repin requires a native host; use --hosts linux,web or --hosts windows,web');
  // A single web dist is mode-specific: bake and run each mode serially.
  for (const host of hosts) {
    let unavailable = false;
    await paranoidRuns(async mode => {
      if (unavailable) return 0;
      try { rows.push(await run(host, 1, false, mode)); return 0; }
      catch (error) {
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
  try { rows.push(await run(native, 1, false, '0', 'release')); } catch (error) { errors.push(error); }
  for (const error of errors) console.error(error.message);
  if (errors.length) throw new Error(`repin refused: mode/host proof failed; pins.json unchanged; inspect ${root}/*/run.log and rerun the named proof with --paranoid`);
  const candidate = agreePins(rows, before, hosts, app);
  const command = proofCommand(import.meta.path, local ? app : name, ...(args.includes('--repin') ? ['--repin'] : []), '--hosts', exercised.join(','), ...(device ? ['--device'] : []), ...(phone ? ['--phone', phone] : []));
  const inputs = pinInputs(rows);
  const after = {...candidate, inputs, game:previous.game ?? name, generated:command, at:pinRevision(app, inputs), ...(option('--reason', '') ? {reason:option('--reason', '')} : {})};
  for (const section of ['ticks', 'saves']) for (const [key, value] of Object.entries(after[section]))
    console.log(`${section} ${key}: ${before[section]?.[key] ?? '(new)'} → ${value}`);
  if (!exercised.includes('web')) console.log(`WEB not exercised; pins record ${exercised.join(',')} only, no web agreement claimed.`);
  writeFileSync(pinFile, JSON.stringify(after, null, 2)+'\n');
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
let failed = failures.length > 0;
console.log('| Host | Run | Seconds | World hashes | Save bytes | Proof |');
console.log('|---|---:|---:|---|---|---|');
for (const row of rows) {
  const hashOK = row.worlds.length > 0 && equal(hashes(row), hashes(baseline));
  const saveOK = row.saves.length > 0 && equal(row.saves, baseline.saves);
  failed ||= !hashOK || (args.includes('--compare-saves') && !saveOK);
  console.log(`| ${row.device ? 'ios-device' : row.host} | ${row.repeat} | ${row.seconds.toFixed(3)} | ${hashOK ? 'equal' : 'FAIL'} | ${args.includes('--compare-saves') ? (saveOK ? 'identical' : 'FAIL') : 'not requested'} | ${row.status ?? 'UNVERIFIED'} |`);
}
for (const failure of failures) console.error(failure.reason);
if (hosts.length > 1) {
  const entry = await import(script);
  if (entry.compare) try { await entry.compare(rows, {root, repeat}); }
  catch (error) { console.error(error.message); failed = true; }
}
const status = failed ? 'FAIL' : rows.length && rows.every(row => row.status === 'PASS') ? 'PASS' : 'UNVERIFIED';
console.log(`PROOF ${status} ${name}`);
if (status === 'UNVERIFIED') console.log(`No complete tick/save baseline was checked. Generate it with ${proofCommand(import.meta.path, local ? app : name, '--repin')}`);
writeFileSync(resolve(root, 'summary.json'), JSON.stringify({status, rows}, null, 2)+'\n');
process.exitCode = status === 'PASS' ? 0 : 1;

}
