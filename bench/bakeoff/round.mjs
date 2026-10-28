// A bake-off round: pin exact2, build every pair, probe every app's scrolling, grade every pair, report.
//   bun bench/bakeoff/round.mjs <name> [build|feel|grade|report|tidy|all] [--specs a,b] [--parallel 6] [--rev <commit>]
// The round lives in ~/bakeoff/rounds/<name> (apps and logs are large); `report` copies its records, feel results,
// grades and summary into bench/bakeoff/results/<name>/.
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { availableParallelism, homedir, loadavg } from 'node:os';
import { resolve } from 'node:path';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const REPO = resolve(HERE, '../..');
const [name, phase = 'all', ...rest] = process.argv.slice(2);
if (!name) { console.error('usage: round.mjs <name> [build|feel|grade|report|tidy|all] [--specs a,b] [--parallel n] [--rev commit]'); process.exit(2); }
const opt = Object.fromEntries(rest.reduce((a, x, i, all) => (x.startsWith('--') ? [...a, [x.slice(2), all[i + 1]]] : a), []));
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
const specs = opt.specs ? opt.specs.split(',') : Object.keys(pairs);
const ROUND = resolve(homedir(), 'bakeoff/rounds', name);
mkdirSync(ROUND, { recursive: true });
const sims = (prefix) => spawnSync('xcrun', ['simctl', 'list', 'devices'], { encoding: 'utf8' }).stdout
  .split('\n').map((l) => l.match(new RegExp(`^\\s+${prefix}\\d* \\(([0-9A-F-]{36})\\)`))?.[1]).filter(Boolean);
const say = (s) => { const line = `${new Date().toISOString().slice(11, 19)} ${s}`; console.log(line); writeFileSync(resolve(ROUND, 'round.log'), line + '\n', { flag: 'a' }); };

/** A simulator held by this round: a lock file naming this process, so two rounds never drive one simulator (a
 * grade uninstalls every bake-off app on its simulator first). A lock whose process is gone is taken over. */
const LOCKS = resolve(homedir(), 'bakeoff/locks');
async function hold(udid) {
  mkdirSync(LOCKS, { recursive: true });
  const path = resolve(LOCKS, udid);
  for (let said = false; ; said = true) {
    try { writeFileSync(path, String(process.pid), { flag: 'wx' }); return () => rmSync(path, { force: true }); } catch {}
    const owner = Number(readFileSync(path, 'utf8'));
    let alive = true;
    try { process.kill(owner, 0); } catch { alive = false; }
    if (!alive || owner === process.pid) { rmSync(path, { force: true }); continue; }
    if (!said) say(`waiting for ${udid}, held by PID ${owner}`);
    await new Promise((r) => setTimeout(r, 10000));
  }
}

/** Run `jobs` (functions of a pool slot) at most `slots.length` at a time, each slot held while its job runs. */
async function pool(jobs, slots) {
  const free = [...slots], queue = [...jobs];
  await Promise.all(slots.map(async () => {
    while (queue.length) {
      const job = queue.shift(), slot = free.shift(), release = await hold(slot);
      try { await job(slot); } catch (e) { say(`job failed: ${e.message}`); } finally { release(); free.push(slot); }
    }
  }));
}
const run = (cmd, argv, logName) => new Promise((ok) => {
  const c = spawn(cmd, argv, { cwd: REPO, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  c.stdout.on('data', (d) => (out += d)); c.stderr.on('data', (d) => (out += d));
  c.on('exit', (code) => { if (logName) writeFileSync(resolve(ROUND, 'logs', logName), out); ok({ code, out }); });
});

/** The exact2 the builders use: a detached checkout without the bake-off's own files. */
function pinCheckout() {
  // A round keeps the commit it was first pinned to: a phase run again later builds against the same exact2.
  const recorded = !opt.rev && existsSync(resolve(ROUND, 'pin.txt')) ? readFileSync(resolve(ROUND, 'pin.txt'), 'utf8').split('\n')[0].trim() : null;
  const rev = recorded || spawnSync('git', ['-C', REPO, 'rev-parse', '--short=10', opt.rev ?? 'HEAD'], { encoding: 'utf8' }).stdout.trim();
  const pin = resolve(homedir(), 'bakeoff/pins', rev);
  if (!existsSync(pin)) {
    spawnSync('git', ['-C', REPO, 'worktree', 'add', '--detach', pin, rev], { stdio: 'inherit' });
    spawnSync('bun', ['install', '--frozen-lockfile'], { cwd: pin, stdio: 'ignore' });
  }
  rmSync(resolve(pin, 'bench/bakeoff'), { recursive: true, force: true });
  writeFileSync(resolve(ROUND, 'pin.txt'), `${rev}\n${pin}\n`);
  return pin;
}

async function build() {
  const pin = pinCheckout();
  mkdirSync(resolve(ROUND, 'logs'), { recursive: true });
  say(`build: ${specs.length} pairs on ${readFileSync(resolve(ROUND, 'pin.txt'), 'utf8').split('\n')[0]}`);
  // Both arms of a spec start together, so they build under the same load.
  const jobs = specs.flatMap((spec) => ['exact', pairs[spec].stack]
    .filter((arm) => opt.redo || !existsSync(resolve(ROUND, 'records', `${spec}-${arm}.json`)))
    .map((arm) => async (sim) => {
      say(`build ${spec}-${arm} on ${sim}`);
      const r = await run('bun', [resolve(HERE, 'build.mjs'), '--round', ROUND, '--spec', spec, '--arm', arm, '--sim', sim, '--pin', pin], `${spec}-${arm}.build.txt`);
      say(`built ${r.out.trim().split('\n').pop()}`);
    }));
  await pool(jobs, sims('bake-b').slice(0, Number(opt.parallel ?? 6)));
}

async function feel() {
  const probe = resolve(HERE, '../../host/apple/feel/probe.mjs');
  mkdirSync(resolve(ROUND, 'feel'), { recursive: true });
  const [sim] = sims('bake-feel');
  const release = await hold(sim);
  spawnSync('xcrun', ['simctl', 'boot', sim]); // a reboot shuts every simulator down; the probe needs it running
  for (const spec of specs) for (const arm of ['exact', pairs[spec].stack]) {
    const recPath = resolve(ROUND, 'records', `${spec}-${arm}.json`);
    if (!existsSync(recPath)) continue;
    const rec = JSON.parse(readFileSync(recPath, 'utf8'));
    if (!rec.app) continue;
    const out = resolve(ROUND, 'feel', `${spec}-${arm}`);
    if (!opt.redo && existsSync(resolve(out, 'feel.json'))) continue;
    // Timing is only comparable on a quiet machine: wait (at most 20 minutes) for the load to fall under the core
    // count, and keep the load each probe ran at beside its result.
    for (const until = Date.now() + 20 * 60000; loadavg()[0] > availableParallelism() && Date.now() < until;) await new Promise((r) => setTimeout(r, 15000));
    const load = Math.round(loadavg()[0]);
    spawnSync('xcrun', ['simctl', 'install', sim, rec.app]);
    const steps = (() => { try { return readFileSync(resolve(HERE, 'specs', spec, 'feel-steps.txt'), 'utf8').trim(); } catch { return ''; } })();
    const r = await run('bun', [probe, '--udid', sim, '--app', rec.appId, '--out', out, ...(steps ? ['--steps', steps] : [])], `${spec}-${arm}.feel.txt`);
    spawnSync('xcrun', ['simctl', 'uninstall', sim, rec.appId]);
    mkdirSync(out, { recursive: true }); // a probe that failed early made none
    writeFileSync(resolve(out, 'load.json'), JSON.stringify({ load, cores: availableParallelism() }) + '\n');
    say(`feel ${spec}-${arm}: exit ${r.code} (load ${load})`);
  }
  release();
}

async function grade() {
  const jobs = specs.filter((spec) => opt.redo || !existsSync(resolve(ROUND, 'grades', spec, 'grade.json'))).map((spec) => async (sim) => {
    say(`grade ${spec} on ${sim}`);
    const r = await run('bun', [resolve(HERE, 'grade.mjs'), '--round', ROUND, '--spec', spec, '--sim', sim], `${spec}.grade.txt`);
    say(`graded ${r.out.trim().split('\n').pop()}`);
  });
  await pool(jobs, sims('bake-g').slice(0, Number(opt.graders ?? 2)));
}

function report() {
  const r = spawnSync('bun', [resolve(HERE, 'report.mjs'), ROUND], { encoding: 'utf8' });
  process.stdout.write(r.stdout + r.stderr);
  const dest = resolve(HERE, 'results', name);
  mkdirSync(dest, { recursive: true });
  for (const d of ['records', 'feel', 'grades']) {
    if (!existsSync(resolve(ROUND, d))) continue;
    cpSync(resolve(ROUND, d), resolve(dest, d), { recursive: true, filter: (p) => !/\/(shots|log\.jsonl|prompt\.md|trace\.jsonl)$/.test(p) && !p.endsWith('.png') && !p.endsWith('.mov') });
  }
  for (const f of ['summary.md', 'pin.txt', 'round.log']) if (existsSync(resolve(ROUND, f))) cpSync(resolve(ROUND, f), resolve(dest, f));
  // The builders' notes on what was hard: the experience half of the loop.
  mkdirSync(resolve(dest, 'notes'), { recursive: true });
  for (const d of existsSync(resolve(ROUND, 'apps')) ? readdirSync(resolve(ROUND, 'apps')) : []) {
    const n = resolve(ROUND, 'apps', d, 'NOTES.md');
    if (existsSync(n)) cpSync(n, resolve(dest, 'notes', `${d}.md`));
  }
}

/** A graded round's Exact build trees (several GB an app; the round's `built/` keeps each app) and every pin but the
 * round's own: nine rounds of them filled the disk. Sources, logs, records, probes and grades stay. */
function tidy() {
  for (const d of existsSync(resolve(ROUND, 'apps')) ? readdirSync(resolve(ROUND, 'apps')) : []) rmSync(resolve(ROUND, 'apps', d, 'target'), { recursive: true, force: true });
  const own = (() => { try { return readFileSync(resolve(ROUND, 'pin.txt'), 'utf8').split('\n')[0]; } catch { return null; } })();
  const pins = resolve(homedir(), 'bakeoff/pins');
  for (const rev of existsSync(pins) ? readdirSync(pins) : []) {
    if (rev === own) continue;
    if (spawnSync('git', ['-C', REPO, 'worktree', 'remove', '--force', resolve(pins, rev)]).status !== 0) rmSync(resolve(pins, rev), { recursive: true, force: true });
  }
  spawnSync('git', ['-C', REPO, 'worktree', 'prune']);
  say('tidied: build trees and older pins removed');
}

if (phase === 'build' || phase === 'all') await build();
if (phase === 'feel' || phase === 'all') await feel();
if (phase === 'grade' || phase === 'all') await grade();
if (phase === 'report' || phase === 'all') report();
if (phase === 'tidy' || phase === 'all') tidy();
say(`${phase} done`);
