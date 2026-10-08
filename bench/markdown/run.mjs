#!/usr/bin/env bun
// The Markdown reader against itself and other readers on macOS: scrolling
// through the window server, and launch to first content (README.md).
//
//   bun bench/markdown/run.mjs scroll [--quick] [--reps 3] [--scenarios a,b]
//   bun bench/markdown/run.mjs launch [--reps 5] [--doc readme|corpus]
//   bun bench/markdown/run.mjs summary <runs dir>
//   bun bench/markdown/run.mjs docs     write the documents and print where
//
// Either run measures this checkout's Markdown app (built first; --no-exact
// skips it), --base <checkout> (that checkout's, labelled base), every
// --app label=<executable or .app>, and --legend <path> (or EXACT_BENCH_LEGEND).
// Apps alternate trial by trial. --record appends the medians to
// bench/markdown/history.jsonl.
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { writeDocs } from './docs.mjs';

const HERE = dirname(new URL(import.meta.url).pathname);
const ROOT = resolve(HERE, '../..');
const WORK = resolve(ROOT, 'target/bench/markdown');
const HISTORY = resolve(HERE, 'history.jsonl');
const LOCK = '/tmp/exact-bench-screen.lock';

const trackpad = (speed, duration) => ['--mode', 'trackpad', '--speed', String(speed), '--duration', String(duration)];
const resize = ['--mode', 'resize', '--duration', '4', '--widths', '900,640'];
const jump = ['--mode', 'jump', '--duration', '5', '--jumps', '0.5,0.1,0.9,0.3,0.7'];
const tiny = ['--mode', 'trackpad', '--speed', '900', '--duration', '2', '--settle', '1'];
// name → [scrollbench arguments, document]; LLP 1044 §3.2's rows first.
const SCENARIOS = {
  slow: [trackpad(900, 6), 'corpus'],
  trackpad: [trackpad(3600, 6), 'corpus'],
  wheel: [['--mode', 'wheel', '--speed', '3600', '--duration', '6'], 'corpus'],
  reverse: [[...trackpad(3600, 6), '--direction', 'alternate'], 'corpus'],
  fast: [trackpad(12000, 4), 'corpus'],
  resize: [resize, 'corpus'],
  jump: [jump, 'corpus'],
};
for (const d of ['dense-inline-1m', 'dense-plain-1m', 'paragraph-1m', 'paragraph-3m', 'code-1m', 'table-1m', 'many-blocks-1m', 'scripts-1m', 'long-tokens-1m', 'emoji-1m', 'zalgo-1m', 'wide-32x2000', 'wide-256x200']) {
  SCENARIOS[`p-${d}`] = [trackpad(3600, 4), d];
  SCENARIOS[`f-${d}`] = [trackpad(12000, 4), d];
  SCENARIOS[`j-${d}`] = [jump, d];
  SCENARIOS[`r-${d}`] = [resize, d];
}
for (const d of ['malformed-bracket-64k', 'malformed-autolink-64k', 'dense-bracket-64k', 'dense-autolink-64k', 'wide-16x10', 'wide-64x10', 'wide-256x10']) SCENARIOS[`o-${d}`] = [tiny, d];
const DEFAULT = ['slow', 'trackpad', 'wheel', 'reverse', 'fast', 'resize', 'jump', 'p-dense-inline-1m', 'p-dense-plain-1m',
  'p-paragraph-1m', 'r-paragraph-1m', 'p-code-1m', 'r-code-1m', 'p-table-1m', 'p-many-blocks-1m', 'p-scripts-1m', 'p-long-tokens-1m',
  'j-dense-inline-1m', 'j-paragraph-1m', 'j-table-1m'];
const QUICK = ['trackpad', 'fast', 'resize', 'jump', 'p-dense-inline-1m', 'r-paragraph-1m'];

const argv = process.argv.slice(2);
const command = argv[0];
const flag = name => argv.includes(name);
const option = (name, fallback) => argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback;
const options = name => argv.flatMap((a, i) => a === name ? [argv[i + 1]] : []);

const median = xs => { const s = xs.filter(x => x != null).sort((a, b) => a - b); return s.length ? (s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2) : null; };
const round = x => x == null ? null : Math.round(x * 10) / 10;
const sh = (cmd, args) => execFileSync(cmd, args, { encoding: 'utf8' }).trim();

/** A probe compiled once per source digest. */
function probe(name) {
  const source = resolve(HERE, `${name}.swift`);
  const digest = createHash('sha256').update(readFileSync(source)).digest('hex').slice(0, 16);
  const bin = resolve(WORK, 'bin', `${name}-${digest}`);
  if (!existsSync(bin)) {
    mkdirSync(dirname(bin), { recursive: true });
    const r = spawnSync('swiftc', ['-O', source, '-o', bin], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error(`swiftc ${name}.swift failed`);
  }
  return bin;
}

/** The apps under test: label → { exec, env }. */
async function apps() {
  const list = [];
  if (!flag('--no-exact')) {
    const r = spawnSync('bun', ['host/apple/build.mjs', 'markdown-apple'], { cwd: ROOT, stdio: 'inherit' });
    if (r.status !== 0) throw new Error('bun host/apple/build.mjs markdown-apple failed');
    const { resolveApp } = await import('../../scripts/app.mjs');
    const { appleArtifacts } = await import('../../host/apple/build.mjs');
    const paths = appleArtifacts(resolveApp('markdown'));
    list.push({ label: 'exact', exec: paths.binary, env: `EXACT_ASSETS=${paths.capture}`, what: 'this checkout' });
  }
  // --base <checkout>: that checkout's Markdown app, built there, as base.
  const base = option('--base');
  if (base) {
    const r = spawnSync('bun', ['host/apple/build.mjs', 'markdown-apple'], { cwd: resolve(base), stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8' });
    process.stdout.write(r.stdout ?? '');
    const built = r.status === 0 && r.stdout.match(/^host\/apple: (\S+\/standalone\/\S+) \(/m);
    if (!built) throw new Error(`could not build the Markdown app in ${base}`);
    const exec = resolve(base, built[1]);
    list.push({ label: 'base', exec, env: `EXACT_ASSETS=${resolve(dirname(exec), '../capture')}`, what: `checkout at ${sh('git', ['-C', resolve(base), 'rev-parse', '--short=10', 'HEAD'])}` });
  }
  const legend = option('--legend', process.env.EXACT_BENCH_LEGEND);
  const named = [...options('--app'), ...(legend ? [`legend=${legend}`] : [])];
  for (const spec of named) {
    const at = spec.indexOf('=');
    if (at < 1) throw new Error(`--app takes label=<path>, not ${spec}`);
    let exec = resolve(spec.slice(at + 1));
    if (exec.endsWith('.app')) exec = resolve(exec, 'Contents/MacOS', sh('plutil', ['-extract', 'CFBundleExecutable', 'raw', resolve(exec, 'Contents/Info.plist')]));
    if (!existsSync(exec)) throw new Error(`no executable at ${exec}`);
    // An Exact development build reads its assets from the capture beside its products.
    const capture = resolve(dirname(exec), '../capture');
    list.push({ label: spec.slice(0, at), exec, env: existsSync(capture) ? `EXACT_ASSETS=${capture}` : '', what: basename(exec) });
  }
  if (new Set(list.map(a => a.label)).size !== list.length) throw new Error('two apps share a label');
  if (!list.length) throw new Error('nothing to measure');
  return list;
}

function hidIdle() {
  const m = sh('ioreg', ['-c', 'IOHIDSystem', '-d', '4']).match(/"HIDIdleTime" = (\d+)/);
  return m ? Number(m[1]) / 1e9 : Infinity;
}
const loadAverage = () => Number(sh('sysctl', ['-n', 'vm.loadavg']).split(/\s+/)[1]);

// One screen, one trial at a time, across every checkout on the machine.
function lock() {
  for (let said = false; ; Bun.sleepSync(2000)) {
    try { mkdirSync(LOCK); writeFileSync(`${LOCK}/pid`, String(process.pid)); return; } catch {}
    let owner = 0;
    try { owner = Number(readFileSync(`${LOCK}/pid`, 'utf8')); } catch {}
    let alive = false;
    try { process.kill(owner, 0); alive = true; } catch {}
    if (owner && !alive) { rmSync(LOCK, { recursive: true, force: true }); continue; }
    if (!said) { console.log(`waiting for ${LOCK} (pid ${owner})`); said = true; }
  }
}
const unlock = () => { try { if (Number(readFileSync(`${LOCK}/pid`, 'utf8')) === process.pid) rmSync(LOCK, { recursive: true, force: true }); } catch {} };
process.on('exit', unlock);
process.on('SIGINT', () => process.exit(130));

function trial(bin, app, args, out) {
  const need = Number(option('--idle', '3'));
  for (let said = false; hidIdle() < need; Bun.sleepSync(1000)) if (!said) { console.log(`waiting for ${need} s without keyboard or mouse input`); said = true; }
  const load = loadAverage();
  lock();
  let r;
  try { r = spawnSync(bin, ['--exec', app.exec, ...(app.env ? ['--env', app.env] : []), '--out', out, ...args], { encoding: 'utf8', timeout: 240_000 }); }
  finally { unlock(); }
  let d;
  try { d = JSON.parse(readFileSync(out, 'utf8')); } catch { d = { error: `no output (${r.status}): ${(r.stderr || '').slice(-300)}` }; }
  d.load_before = load;
  writeFileSync(out, JSON.stringify(d, null, 1));
  return d;
}

function scrollSummary(dir) {
  const rows = {};
  for (const f of readdirSync(dir).filter(f => f.endsWith('.json')).sort()) {
    const [scen, app] = f.split('.');
    const j = JSON.parse(readFileSync(resolve(dir, f), 'utf8'));
    ((rows[scen] ??= {})[app] ??= []).push(j.error || j.human_interference ? null : j);
  }
  const table = {};
  console.log(`${'scenario'.padEnd(22)} ${'app'.padEnd(8)} ${'n'.padStart(2)} ${'fps'.padStart(6)} ${'hitch/s'.padStart(8)} ${'drop'.padStart(5)} ${'p99gap'.padStart(7)} ${'maxgap'.padStart(7)} ${'blank'.padStart(6)} ${'travel'.padStart(8)}`);
  for (const [scen, byApp] of Object.entries(rows)) for (const [app, all] of Object.entries(byApp)) {
    const js = all.filter(Boolean), m = k => round(median(js.map(k)));
    const head = `${scen.padEnd(22)} ${app.padEnd(8)} ${String(js.length).padStart(2)}`;
    if (!js.length) { console.log(`${head}  every trial failed or was touched (${all.length})`); continue; }
    if (SCENARIOS[scen]?.[0].includes('jump')) {
      const land = js.flatMap(j => j.jump_first_last_count_inkless ?? []), content = land.map(x => x[1]).filter(x => x >= 0);
      const row = { n: js.length, jumps: land.length, first_ms: round(median(land.map(x => x[0]))), content_ms: round(median(content)),
        content_max_ms: content.length ? round(Math.max(...content)) : null, unfinished: land.filter(x => x[1] < 0).length, blank_frames: land.reduce((s, x) => s + x[3], 0) };
      (table[scen] ??= {})[app] = row;
      console.log(`${head}  ${row.jumps} jumps: first change ${row.first_ms} ms; content p50/max ${row.content_ms}/${row.content_max_ms} ms; unfinished ${row.unfinished}; blank frames ${row.blank_frames}`);
      continue;
    }
    const row = { n: js.length, fps: m(j => j.presented_fps), hitch_ms_per_s: m(j => j.hitch_ms_per_s), dropped: m(j => j.dropped_frames),
      p99_gap_ms: m(j => j.gap_ms_p50_p95_p99_max[2]), max_gap_ms: m(j => j.gap_ms_p50_p95_p99_max[3]), blank_over_250: m(j => j.frames_inkless_over_250), travel_px: m(j => j.total_shift_px) };
    (table[scen] ??= {})[app] = row;
    console.log(`${head} ${String(row.fps).padStart(6)} ${String(row.hitch_ms_per_s).padStart(8)} ${String(row.dropped).padStart(5)} ${String(row.p99_gap_ms).padStart(7)} ${String(row.max_gap_ms).padStart(7)} ${String(row.blank_over_250).padStart(6)} ${String(row.travel_px).padStart(8)}`);
  }
  return table;
}

function launchSummary(dir) {
  const byApp = {};
  for (const f of readdirSync(dir).filter(f => f.startsWith('launch.') && f.endsWith('.json')).sort()) {
    const j = JSON.parse(readFileSync(resolve(dir, f), 'utf8'));
    (byApp[f.split('.')[1]] ??= []).push(j);
  }
  const table = {};
  for (const [app, js] of Object.entries(byApp)) {
    const ok = k => js.map(j => j[k]).filter(x => x > 0);
    table[app] = { n: js.length, window_ms: round(median(ok('first_window_ms'))), content_ms: round(median(ok('first_content_ms'))), content_runs: ok('first_content_ms').map(Math.round) };
    console.log(`${app.padEnd(8)} window ${table[app].window_ms} ms, content ${table[app].content_ms} ms (median of ${table[app].content_runs.length}/${js.length}: ${table[app].content_runs.join(', ')})`);
  }
  return table;
}

function record(kind, dir, table, extra) {
  const git = (...a) => sh('git', ['-C', ROOT, ...a]);
  const fps = (() => { try { return JSON.parse(readFileSync(resolve(dir, readdirSync(dir).find(f => f.endsWith('.json'))), 'utf8')).refresh_ms; } catch { return undefined; } })();
  const line = { kind, date: new Date().toISOString(), commit: git('rev-parse', '--short=10', 'HEAD'), dirty: git('status', '--porcelain', '--untracked-files=no') !== '',
    machine: sh('sysctl', ['-n', 'hw.model']), chip: sh('sysctl', ['-n', 'machdep.cpu.brand_string']), macos: sh('sw_vers', ['-productVersion']),
    refresh_ms: fps === undefined ? undefined : round(fps), load: round(loadAverage()), ...extra, results: table };
  appendFileSync(HISTORY, JSON.stringify(line) + '\n');
  console.log(`recorded in ${HISTORY.replace(ROOT + '/', '')}`);
}

function runDir(kind) {
  const dir = option('--out', resolve(WORK, 'runs', `${new Date().toISOString().replace(/[:.]/g, '-')}-${kind}`));
  mkdirSync(dir, { recursive: true });
  return dir;
}

if (command === 'docs') {
  console.log(writeDocs(ROOT, resolve(WORK, 'docs')));
} else if (command === 'summary') {
  const dir = resolve(argv[1] ?? '');
  if (!existsSync(dir)) throw new Error('summary takes a runs directory');
  readdirSync(dir).some(f => f.startsWith('launch.')) ? launchSummary(dir) : scrollSummary(dir);
} else if (command === 'scroll') {
  const scenarios = option('--scenarios', (flag('--quick') ? QUICK : DEFAULT).join(',')).split(',');
  for (const s of scenarios) if (!SCENARIOS[s]) throw new Error(`no scenario ${s}; known: ${Object.keys(SCENARIOS).join(' ')}`);
  const reps = Number(option('--reps', flag('--quick') ? '1' : '3'));
  const docs = writeDocs(ROOT, resolve(WORK, 'docs')), bin = probe('scrollbench'), list = await apps(), dir = runDir('scroll');
  console.log(`${scenarios.length} scenarios × ${reps} × ${list.map(a => a.label).join(', ')} → ${dir}\nThe probe moves the pointer and brings each app forward; hands off the keyboard and mouse.`);
  for (const scen of scenarios) for (let rep = 0; rep < reps; rep++) {
    const [args, doc] = SCENARIOS[scen];
    for (const app of rep % 2 ? [...list].reverse() : list) {
      const d = trial(bin, app, ['--settle', '4', '--footprint', '0', ...args, '--', resolve(docs, `${doc}.md`)], resolve(dir, `${scen}.${app.label}.${rep}.json`));
      const land = d.jump_first_last_count_inkless ?? [];
      console.log(`${scen} ${app.label} ${rep}: ${d.error ?? (d.human_interference ? 'touched, discarded' : land.length
        ? `content after a jump ${land.map(x => Math.round(x[1])).join(', ')} ms`
        : `${round(d.presented_fps)} fps, ${round(d.hitch_ms_per_s)} hitch ms/s`)}`);
    }
  }
  console.log();
  const table = scrollSummary(dir);
  if (flag('--record')) record('scroll', dir, table, { reps, apps: Object.fromEntries(list.map(a => [a.label, a.what])) });
} else if (command === 'launch') {
  const doc = option('--doc', 'readme'), reps = Number(option('--reps', '5'));
  const docs = writeDocs(ROOT, resolve(WORK, 'docs')), bin = probe('launchbench'), list = await apps(), dir = runDir('launch');
  console.log(`launch with ${doc}.md × ${reps} × ${list.map(a => a.label).join(', ')} → ${dir}`);
  // The first launch after a build reads a cold page cache; one per app is discarded.
  const region = option('--region') ? ['--region', option('--region')] : [];
  for (const app of list) trial(bin, app, [...region, '--', resolve(docs, `${doc}.md`)], resolve(dir, `warm.${app.label}.json`));
  for (let rep = 0; rep < reps; rep++) for (const app of rep % 2 ? [...list].reverse() : list) {
    const d = trial(bin, app, [...region, '--', resolve(docs, `${doc}.md`)], resolve(dir, `launch.${app.label}.${rep}.json`));
    console.log(`${app.label} ${rep}: ${d.error ?? `window ${round(d.first_window_ms)} ms, content ${round(d.first_content_ms)} ms`}`);
  }
  console.log();
  const table = launchSummary(dir);
  if (flag('--record')) record('launch', dir, table, { doc, reps, apps: Object.fromEntries(list.map(a => [a.label, a.what])) });
} else {
  console.log(readFileSync(new URL(import.meta.url), 'utf8').split('\n').slice(1, 14).map(l => l.replace(/^\/\/ ?/, '')).join('\n'));
  process.exit(command ? 2 : 0);
}
