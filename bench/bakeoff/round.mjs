// A bake-off round: pin exact2, build every pair, probe every app's scrolling, grade every pair, report.
//   bun bench/bakeoff/round.mjs <name> [build|feel|grade|report|all] [--specs a,b] [--parallel 6] [--rev <commit>]
// The round lives in ~/bakeoff/rounds/<name> (apps and logs are large); `report` copies its records, feel results,
// grades and summary into bench/bakeoff/results/<name>/.
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

const HERE = resolve(new URL('.', import.meta.url).pathname);
const REPO = resolve(HERE, '../..');
const [name, phase = 'all', ...rest] = process.argv.slice(2);
if (!name) { console.error('usage: round.mjs <name> [build|feel|grade|report|all] [--specs a,b] [--parallel n] [--rev commit]'); process.exit(2); }
const opt = Object.fromEntries(rest.reduce((a, x, i, all) => (x.startsWith('--') ? [...a, [x.slice(2), all[i + 1]]] : a), []));
const pairs = JSON.parse(readFileSync(resolve(HERE, 'pairs.json'), 'utf8'));
const specs = opt.specs ? opt.specs.split(',') : Object.keys(pairs);
const ROUND = resolve(homedir(), 'bakeoff/rounds', name);
mkdirSync(ROUND, { recursive: true });
const sims = (prefix) => spawnSync('xcrun', ['simctl', 'list', 'devices'], { encoding: 'utf8' }).stdout
  .split('\n').map((l) => l.match(new RegExp(`^\\s+${prefix}\\d* \\(([0-9A-F-]{36})\\)`))?.[1]).filter(Boolean);
const say = (s) => { const line = `${new Date().toISOString().slice(11, 19)} ${s}`; console.log(line); writeFileSync(resolve(ROUND, 'round.log'), line + '\n', { flag: 'a' }); };

/** Run `jobs` (functions of a pool slot) at most `slots.length` at a time. */
async function pool(jobs, slots) {
  const free = [...slots], queue = [...jobs];
  await Promise.all(slots.map(async () => {
    while (queue.length) {
      const job = queue.shift(), slot = free.shift();
      try { await job(slot); } catch (e) { say(`job failed: ${e.message}`); } finally { free.push(slot); }
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
  const rev = spawnSync('git', ['-C', REPO, 'rev-parse', '--short=10', opt.rev ?? 'HEAD'], { encoding: 'utf8' }).stdout.trim();
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
  const probe = resolve(HERE, 'feel/probe.mjs');
  if (!existsSync(probe)) { say('feel: no probe yet, skipped'); return; }
  mkdirSync(resolve(ROUND, 'feel'), { recursive: true });
  const [sim] = sims('bake-feel');
  for (const spec of specs) for (const arm of ['exact', pairs[spec].stack]) {
    const recPath = resolve(ROUND, 'records', `${spec}-${arm}.json`);
    if (!existsSync(recPath)) continue;
    const rec = JSON.parse(readFileSync(recPath, 'utf8'));
    if (!rec.app) continue;
    const out = resolve(ROUND, 'feel', `${spec}-${arm}`);
    if (!opt.redo && existsSync(resolve(out, 'feel.json'))) continue;
    spawnSync('xcrun', ['simctl', 'install', sim, rec.app]);
    const steps = (() => { try { return readFileSync(resolve(HERE, 'specs', spec, 'feel-steps.txt'), 'utf8').trim(); } catch { return ''; } })();
    const r = await run('bun', [probe, '--udid', sim, '--app', rec.appId, '--out', out, ...(steps ? ['--steps', steps] : [])], `${spec}-${arm}.feel.txt`);
    spawnSync('xcrun', ['simctl', 'uninstall', sim, rec.appId]);
    say(`feel ${spec}-${arm}: exit ${r.code}`);
  }
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
    cpSync(resolve(ROUND, d), resolve(dest, d), { recursive: true, filter: (p) => !/\/(shots|log\.jsonl|prompt\.md)$/.test(p) && !p.endsWith('.png') && !p.endsWith('.mov') });
  }
  for (const f of ['summary.md', 'pin.txt', 'round.log']) if (existsSync(resolve(ROUND, f))) cpSync(resolve(ROUND, f), resolve(dest, f));
  // The builders' notes on what was hard: the experience half of the loop.
  mkdirSync(resolve(dest, 'notes'), { recursive: true });
  for (const d of existsSync(resolve(ROUND, 'apps')) ? readdirSync(resolve(ROUND, 'apps')) : []) {
    const n = resolve(ROUND, 'apps', d, 'NOTES.md');
    if (existsSync(n)) cpSync(n, resolve(dest, 'notes', `${d}.md`));
  }
}

if (phase === 'build' || phase === 'all') await build();
if (phase === 'feel' || phase === 'all') await feel();
if (phase === 'grade' || phase === 'all') await grade();
if (phase === 'report' || phase === 'all') report();
say(`${phase} done`);
