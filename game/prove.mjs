#!/usr/bin/env bun
// Orchestrate the game's existing proof; every drive still uses the eight operations.
import {spawn} from 'node:child_process';
import {existsSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {equal} from './proof.mjs';

const [name, ...args] = process.argv.slice(2);
const option = (flag, fallback) => args.includes(flag) ? args[args.indexOf(flag) + 1] : fallback;
const hosts = option('--hosts', 'linux,web').split(','), repeat = Number(option('--repeat', '1'));
if (!/^[a-z][a-z0-9-]*$/.test(name ?? '') || !Number.isSafeInteger(repeat) || repeat < 1
    || !hosts.length || new Set(hosts).size !== hosts.length || hosts.some(h => !['web','linux'].includes(h))) {
  throw new Error('Usage: bun game/prove.mjs <game> --hosts web,linux --repeat 2 --compare-saves');
}
const app = resolve(import.meta.dir, 'games', name), script = resolve(app, 'proof.mjs');
if (!existsSync(script)) throw new Error(`No proof for ${name}`);
const root = resolve(app, 'artifacts/prove');
mkdirSync(root, {recursive:true});
const run = async (host, index, build = false) => {
  const out = resolve(root, `${host}-${build ? 'build' : index}`);
  mkdirSync(out, {recursive:true});
  const child = spawn(process.execPath, [script, host, ...(build ? ['--build-only'] : [])], {
    env:{...process.env, EXACT_PROOF_OUT:out, EXACT_PROOF_COMPARE:build ? '0' : '1'},
    stdio:['ignore','pipe','pipe'],
  });
  let log = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {log += bytes;});
  const code = await new Promise((ok, reject) => {child.on('exit', ok); child.on('error', reject);});
  writeFileSync(resolve(out, 'run.log'), log);
  if (code !== 0) throw new Error(`${host} ${build ? 'build' : index} failed: ${out}/run.log\n${log.slice(-2500)}`);
  return {...JSON.parse(readFileSync(resolve(out, 'summary.json'), 'utf8')), repeat:index};
};
// Bakes share Cargo and asset output. Complete those serially, then run the
// independent host processes in parallel with separate receipts/save paths.
for (const host of hosts) await run(host, 0, true);
const groups = await Promise.allSettled(hosts.map(async host => {
  const rows = [];
  for (let index = 1; index <= repeat; index++) rows.push(await run(host, index));
  return rows;
}));
const failures = groups.filter(r => r.status === 'rejected');
const rows = groups.flatMap(r => r.status === 'fulfilled' ? r.value : []);
const hashes = row => row.worlds.map(({session, tick, hash}) => ({session, tick, hash}));
const baseline = rows[0];
let failed = failures.length > 0;
console.log('| Host | Run | Seconds | World hashes | Save bytes |');
console.log('|---|---:|---:|---|---|');
for (const row of rows) {
  const hashOK = row.worlds.length > 0 && equal(hashes(row), hashes(baseline));
  const saveOK = row.saves.length > 0 && equal(row.saves, baseline.saves);
  failed ||= !hashOK || (args.includes('--compare-saves') && !saveOK);
  console.log(`| ${row.host} | ${row.repeat} | ${row.seconds.toFixed(3)} | ${hashOK ? 'equal' : 'FAIL'} | ${args.includes('--compare-saves') ? (saveOK ? 'identical' : 'FAIL') : 'not requested'} |`);
}
for (const failure of failures) console.error(failure.reason);
writeFileSync(resolve(root, 'summary.json'), JSON.stringify({passed:!failed, rows}, null, 2)+'\n');
process.exitCode = failed ? 1 : 0;
