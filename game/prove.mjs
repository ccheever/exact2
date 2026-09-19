#!/usr/bin/env bun
// Orchestrate the game's existing proof; every drive still uses the eight operations.
import {spawn, spawnSync} from 'node:child_process';
import {existsSync, mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {basename, resolve} from 'node:path';
import {equal, agreePins, webUnavailable, comparePlacement} from './proof.mjs';

const [destination, ...args] = process.argv.slice(2);
const local = destination?.includes('/');
const name = local ? basename(resolve(destination)) : destination;
const option = (flag, fallback) => args.includes(flag) ? args[args.indexOf(flag) + 1] : fallback;
const repin = args.includes('--repin');
const hosts = option('--hosts', 'linux,web').split(','), repeat = Number(option('--repeat', '1'));
if (!/^[a-z][a-z0-9-]*$/.test(name ?? '') || !Number.isSafeInteger(repeat) || repeat < 1
    || !hosts.length || new Set(hosts).size !== hosts.length || hosts.some(h => !['web','linux','macos','ios'].includes(h))) {
  throw new Error('Usage: bun game/prove.mjs <name|path> --hosts web,linux,ios --repeat 2 --compare-saves');
}
const app = local ? resolve(destination) : resolve(import.meta.dir, 'games', name), script = resolve(app, 'proof.mjs');
const argument = local ? `'${app.replaceAll("'", "'\\''")}'` : name;
if (!existsSync(script)) throw new Error(`No proof for ${name}`);
const root = resolve(app, 'artifacts/prove');
mkdirSync(root, {recursive:true});
const run = async (host, index, build = false, mode = '0') => {
  const out = resolve(root, `${host}-${mode}-${build ? 'build' : index}`);
  mkdirSync(out, {recursive:true});
  rmSync(resolve(out, 'summary.json'), {force:true});
  rmSync(resolve(out, `placement-${host}.json`), {force:true});
  const child = spawn(process.execPath, [script, host, ...(build ? ['--build-only'] : [])], {
    env:{...process.env, EXACT_PROOF_OUT:out, EXACT_PROOF_COMPARE:build || repin ? '0' : '1', EXACT_PROOF_REPIN:repin ? '1' : '0', EXACT_GAME_PARANOID:mode},
    stdio:['ignore','pipe','pipe'],
  });
  let log = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {log += bytes;});
  const code = await new Promise((ok, reject) => {child.on('exit', ok); child.on('error', reject);});
  writeFileSync(resolve(out, 'run.log'), log);
  const summaryPath = resolve(out, 'summary.json');
  const summary = existsSync(summaryPath) ? {...JSON.parse(readFileSync(summaryPath, 'utf8')), repeat:index} : null;
  if (summary && name === 'placement-fixture' && !build && !summary.failures?.length) {
    const tuples = resolve(out, `placement-${host}.json`);
    if (existsSync(tuples)) summary.placement = JSON.parse(readFileSync(tuples,'utf8'));
  }
  if (args.includes('--report') && (!build || code !== 0) && summary) for (const hint of summary.facilities ?? []) console.log(`REPORT ${host} ${mode}: ${hint}`);
  if (code !== 0) throw Object.assign(new Error(`${host} mode ${mode} ${build ? 'build' : index} failed: ${out}/run.log\n${log.slice(-2500)}`), {summary, webUnavailable:host === 'web' && webUnavailable(log)});
  return summary;
};
if (repin) {
  const pinFile = resolve(app, 'pins.json'), before = JSON.parse(readFileSync(pinFile, 'utf8'));
  const rows = [], errors = [], exercised = [];
  if (!hosts.includes('linux')) throw new Error('repin requires the linux host; use --hosts linux,web');
  // A single web dist is mode-specific: bake and run each mode serially.
  for (const host of hosts) {
    let unavailable = false;
    try {
      for (const mode of ['0', '1', 'fresh-game']) {
        try { rows.push(await run(host, 1, false, mode)); }
        catch (error) {
          if (mode === '0' && error.webUnavailable) { unavailable = true; console.log('WEB unavailable: configured Chrome could not launch (ENOENT); repin will refuse the missing requested host. Set CHROME and rerun, or explicitly use --hosts linux.'); break; }
          errors.push(error);
        }
      }
    } finally {
      // Leave ordinary mode's receipt/product, including after a refusal.
      if (host === 'web' && !unavailable) try { await run(host, 0, true); } catch (error) { errors.push(error); }
    }
    if (!unavailable) exercised.push(host);
  }
  for (const error of errors) console.error(error.message);
  if (errors.length) throw new Error('repin refused: mode/host proof failed; pins.json unchanged; inspect artifacts/prove/*/run.log and rerun the named proof with --paranoid');
  const candidate = agreePins(rows, before, hosts);
  const revision = spawnSync('git', ['rev-parse', 'HEAD'], {cwd:app, encoding:'utf8'});
  if (revision.status !== 0) throw new Error('repin refused: cannot identify commit; pins.json unchanged');
  const command = `bun game/prove.mjs ${argument} --repin --hosts ${exercised.join(',')}`;
  const after = {...candidate, generated:command, at:revision.stdout.trim()};
  for (const section of ['ticks', 'saves']) for (const [key, value] of Object.entries(after[section]))
    console.log(`${section} ${key}: ${before[section]?.[key] ?? '(new)'} → ${value}`);
  if (!exercised.includes('web')) console.log('WEB not exercised; pins record linux only, no web agreement claimed.');
  writeFileSync(pinFile, JSON.stringify(after, null, 2)+'\n');
} else {
// Bakes share Cargo and asset output. Complete those serially, then run the
// independent host processes in parallel with separate receipts/save paths.
const buildFailures = new Map();
for (const host of hosts) {
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
  console.log(`| ${row.host} | ${row.repeat} | ${row.seconds.toFixed(3)} | ${hashOK ? 'equal' : 'FAIL'} | ${args.includes('--compare-saves') ? (saveOK ? 'identical' : 'FAIL') : 'not requested'} | ${row.status ?? 'UNVERIFIED'} |`);
}
for (const failure of failures) console.error(failure.reason);
if (name === 'placement-fixture' && hosts.includes('linux') && hosts.includes('web')) {
  try {
    for (let index=1; index<=repeat; index++) comparePlacement(
      rows.find(r=>r.host==='linux' && r.repeat===index)?.placement,
      rows.find(r=>r.host==='web' && r.repeat===index)?.placement);
    console.log('PLACEMENT web/Linux captured tuples agree within 0.5 px');
  } catch(error) { console.error(error.message); failed=true; }
}
const status = failed ? 'FAIL' : rows.length && rows.every(row => row.status === 'PASS') ? 'PASS' : 'UNVERIFIED';
console.log(`PROOF ${status} ${name}`);
if (status === 'UNVERIFIED') console.log(`No complete tick/save baseline was checked. Generate it with bun game/prove.mjs ${argument} --repin`);
writeFileSync(resolve(root, 'summary.json'), JSON.stringify({status, rows}, null, 2)+'\n');
process.exitCode = failed ? 1 : 0;

}
