#!/usr/bin/env bun
// `bun exact.mjs verify [web|ios]` (LLP 1116.004 D7): one command a builder can stop on, where test, a
// drive, check and a screenshot were three to five turns. It builds when the build is stale (web: the dist's
// record against its sources, as the driver checks it; iOS: agent-build.mjs `buildFirst`, the plan fast path
// included), runs app.test.contract (agent-test.mjs `runTests`), then launches the app on a fresh scratch
// store at a phone width and a desktop width (iOS: the simulator's own size, with UIKit's bars) and runs
// `check` light and dark at each, keeping one capture per width and scheme in .exact/verify/ for milestones.
// It prints one short report ending "verify: clean" or "verify: N problems" (`--json`: one object). Exit 1
// when the build, a test or a launch fails; check's findings are counted in the last line but never fail it,
// since they read the screen by heuristics and some only advise. The web's tests and its two launches run
// at once; a simulator takes one drive at a time.
//
//   bun scripts/verify.mjs [web|ios] --app <name> [--sim <udid|name>] [--json]
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { guardFlags } from './help.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
/** The web's two widths: a phone's, and a desktop window's. */
export const WEB_SIZES = [[390, 844], [1280, 900]];
const SHOWN = 6, WARNINGS = 4; // the findings and build warnings shown; the count says the rest
const secs = (ms) => `${(ms / 1000).toFixed(1)} s`;
const firstLine = (text) => String(text ?? '').split('\n').find((l) => l.trim())?.trim() ?? '';
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** Builds the web app when its dist is missing or older than its sources, as the driver would refuse it:
 * null when fresh, else `{ ms, code, warnings, output }`. */
async function webBuild(app) {
  const [{ webDist }, { builtAppMatches, jsTargetBuild }, { webChanges }] = await Promise.all([import('./app.mjs'), import('../host/web/serve.mjs'), import('./agent-launch.mjs')]);
  const dist = webDist();
  if (await builtAppMatches(dist, app) && jsTargetBuild(dist) && !webChanges(dist, app).all.length) return null;
  const started = Date.now();
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.name], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20, env: { ...process.env, EXACT_APP_DIR: app.dir } });
  const output = `${r.stdout ?? ''}${r.stderr ?? ''}`;
  return { ms: Date.now() - started, code: r.status ?? 1, output, warnings: output.split('\n').filter((l) => /^warning: /.test(l)) };
}

/** The iOS build a drive needs, or its plan alone (agent-build.mjs): null when fresh, else what it said. */
async function iosBuild(app, sim) {
  const { buildFirst } = await import('./agent-build.mjs');
  const said = [], started = Date.now();
  // A native build (or the wait for one) is said as it starts too: it is minutes, and the report comes after it.
  const code = await buildFirst('ios', app, { sim, say: (line) => { said.push(line); if (/^(building|waiting) for/.test(line)) console.error(line); } });
  return said.length || code ? { ms: Date.now() - started, code, said, warnings: [] } : null;
}

/** The authored tests: `{ passed, failed, results }`, `{ none }` without a file, or `{ error }` when they do not compile. */
async function tests(host, app) {
  const file = resolve(app.dir, 'app.test.contract');
  if (!existsSync(file)) return { none: true };
  const { runTests } = await import('./agent-test.mjs');
  try {
    const r = await runTests({ host, file, app: app.name });
    return { passed: r.passed, failed: r.failed, results: r.results.map(({ name, failures }) => ({ name, failures })) };
  } catch (e) { return { error: e.message }; }
}

/** One launch at `size` (null: the device's own) on a fresh scratch store: the app's data landed and its
 * transitions ended, then `check both`, its captures kept in `shots`. A run per scheme, `{ size, scheme, asked,
 * shot, findings, lines, hidden }`: `lines` are check's own text for it, with a line for a resource that failed. */
async function checkAt(host, app, size, store, shots) {
  const [{ open }, { checkOp }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-check.mjs'), import('./agent-launch.mjs')]);
  const fresh = { EXACT_AGENT_STORAGE_FRESH: '1' };
  const s = await open({ host, app: app.name, storage: store, env: fresh, ...(size ? { size } : {}), ...(host === 'ios' ? { chrome: 'platform' } : {}) });
  try {
    await s.clock('data');
    await s.clock('settle');
    const failed = Object.entries((await s.state().catch(() => ({}))).failed ?? {}).map(([name, why]) => `  failed   resource ${name}: ${why}; the screen shows its placeholder or last value`);
    const shot = (vp, scheme) => resolve(shots, `${host}-${vp.w}x${vp.h}-${scheme}.png`);
    const r = await checkOp(s, ['both'], { shots: shot });
    return groups(r.text).map((g, i) => {
      const run = r.check[i] ?? {}, vp = run.viewport ?? { w: size?.[0], h: size?.[1] };
      return { size: [vp.w, vp.h], scheme: run.scheme, asked: run.asked, shot: shot(vp, run.asked ?? run.scheme), findings: run.findings ?? [], lines: [...failed, ...g.lines], hidden: g.hidden };
    });
  } finally {
    await s.close();
    if (host === 'web') await removeBrowserProfile(driveStore(app.id, store).profile).catch(() => {});
  }
}

/** check's text by scheme: each header's finding lines, and how many its "… and N more" left out. */
export function groups(text) {
  const out = [];
  for (const line of String(text ?? '').split('\n')) {
    if (/^check /.test(line)) { out.push({ head: line, lines: [], hidden: 0 }); continue; }
    const g = out.at(-1), more = line.match(/^\s+… and (\d+) more/);
    if (!g || !line.trim()) continue;
    if (more) g.hidden += Number(more[1]);
    else g.lines.push(line);
  }
  return out;
}

const BOX = /\s*\[-?\d+,-?\d+ \d+×\d+\]/g;
/** The findings of every launch and scheme, each once: a line found at several (its box aside, which a width
 * moves) is shown once, tagged with where it was unless it was everywhere. */
export function distinctFindings(runs) {
  const sizes = new Set(runs.map((r) => r.size.join('x'))), schemes = new Set(runs.map((r) => r.asked ?? r.scheme)), seen = new Map();
  for (const r of runs) for (const line of r.lines) {
    const key = line.replace(BOX, '').replace(/\d+-wide/g, '');
    if (!seen.has(key)) seen.set(key, { line, where: [] });
    seen.get(key).where.push([r.size.join('x'), r.asked ?? r.scheme]);
  }
  return [...seen.values()].map(({ line, where }) => {
    const ws = [...new Set(where.map((w) => w[0]))], ss = [...new Set(where.map((w) => w[1]))];
    const tag = where.length === runs.length ? '' : ws.length === sizes.size ? ss.join(', ') : ss.length === schemes.size ? ws.join(', ') : where.map((w) => w.join(' ')).join(', ');
    return tag ? line.replace(/^(\s*)/, `$1(${tag} only) `) : line;
  });
}

/** The report's lines and its problem count, from what each stage gave. */
export function report({ host, built, tested, checks, shots, ms, cwd = process.cwd() }) {
  const out = [];
  let failedTests = 0, findings = 0, broken = 0;
  if (built) {
    const said = built.said?.length ? built.said.join('\n') : `built ${host}`;
    out.push(`${said} (${secs(built.ms)}${built.warnings.length ? `, ${plural(built.warnings.length, 'warning')}` : ''})`);
    for (const w of built.warnings.slice(0, WARNINGS)) out.push(`  ${w}`);
    if (built.warnings.length > WARNINGS) out.push(`  … and ${built.warnings.length - WARNINGS} more (bun exact.mjs web-build prints them all)`);
    findings += built.warnings.length;
  }
  if (tested?.none) out.push('tests: none (no app.test.contract)');
  else if (tested?.error) {
    broken++;
    out.push(`tests: did not compile`, ...tested.error.split('\n').filter((l) => l.trim()).slice(0, 6).map((l) => `  ${l}`));
  } else if (tested) {
    failedTests = tested.failed;
    out.push(`tests: ${tested.passed} passed, ${tested.failed} failed`);
    for (const t of tested.results) for (const f of t.failures) out.push(`  FAIL "${t.name}": ${firstLine(f).replace(`${t.name}: `, '')}`);
  }
  // Every launch and scheme in one group: each finding once, said where unless it was everywhere.
  for (const e of checks.filter((c) => c.error)) { broken++; out.push(`check ${e.label}: could not drive: ${firstLine(e.error)}`); }
  const runs = checks.filter((c) => !c.error);
  if (runs.length) {
    const shown = distinctFindings(runs), count = shown.length + Math.max(0, ...runs.map((r) => r.hidden));
    const kept = runs.some((r) => r.asked && r.asked !== r.scheme) ? '; the app keeps its own scheme' : '';
    out.push(`check ${[...new Set(runs.map((r) => r.size.join('x')))].join(', ')} · ${[...new Set(runs.map((r) => r.asked ?? r.scheme))].join(', ')}${kept}: ${count ? plural(count, 'finding') : 'no findings'}`);
    out.push(...shown.slice(0, SHOWN));
    if (count > SHOWN) out.push(`  … and ${count - Math.min(SHOWN, shown.length)} more (verify --json lists them all)`);
    findings += count;
  }
  if (shots.length) {
    const dir = relative(cwd, resolve(shots[0], '..')) || '.';
    out.push(`screenshots (${dir.startsWith('..') ? resolve(shots[0], '..') : dir}): ${shots.map((p) => relative(resolve(p, '..'), p)).join(' ')}`);
  }
  const problems = failedTests + findings + broken;
  const why = [failedTests && plural(failedTests, 'failed test'), broken && plural(broken, 'failure', 'failures') + ' to compile or drive', findings && plural(findings, 'finding')].filter(Boolean);
  out.push(problems ? `verify: ${plural(problems, 'problem')} (${why.join(', ')}${failedTests || broken ? '' : '; the tests passed'}) in ${secs(ms)}` : `verify: clean in ${secs(ms)}`);
  return { lines: out, problems, exit: failedTests || broken ? 1 : 0 };
}

async function main(argv) {
  const args = guardFlags('verify', argv);
  const value = (flag) => args.indexOf(flag) >= 0 ? args[args.indexOf(flag) + 1] : undefined;
  const json = args.includes('--json'), sim = value('--sim');
  const flagged = new Set([args.indexOf('--app') + 1, args.indexOf('--sim') + 1].filter((i) => i > 0));
  const host = args.find((a, i) => !a.startsWith('-') && !flagged.has(i)) ?? 'web';
  if (!['web', 'ios'].includes(host)) { console.error(`verify: web or ios, not ${host} (test ${host} runs the tests there)`); return 2; }
  if (sim) process.env.EXACT_SIM = sim; // as the driver names it (devices.mjs `simulator`)
  const started = Date.now();
  const { resolveApp } = await import('./app.mjs');
  const app = resolveApp(value('--app'));
  const say = (lines) => console.log(lines.join('\n'));

  const built = host === 'web' ? await webBuild(app) : await iosBuild(app, sim);
  if (built?.code) {
    const tail = host === 'web' ? built.output.split('\n').filter((l) => l.trim()).slice(-20) : built.said;
    if (json) console.log(JSON.stringify({ host, built: { ms: built.ms, code: built.code, said: tail }, problems: 1, exit: built.code }));
    else say([...tail, `verify: the ${host} build failed (exit ${built.code}), so nothing ran`]);
    return built.code;
  }

  const shots = resolve(app.dir, '.exact', 'verify');
  mkdirSync(shots, { recursive: true });
  for (const f of readdirSync(shots)) if (f.startsWith(`${host}-`) && f.endsWith('.png')) rmSync(resolve(shots, f), { force: true });
  // The web's scratch stores are this run's own, removed after it (and any a killed run left, as a test's are).
  const tag = `r${process.pid}-${Math.random().toString(36).slice(2, 8)}`;
  if (host === 'web') {
    const [{ driveStore }, { sweepTestStores }] = await Promise.all([import('./agent-launch.mjs'), import('./agent-test.mjs')]);
    sweepTestStores(driveStore(app.id, 'verify').base, 'verify');
  }
  const launch = (size, n) => checkAt(host, app, size, host === 'web' ? `verify.${tag}.t${n}` : 'verify', shots)
    .catch((e) => [{ size: null, label: size ? size.join('x') : host, error: e.message }]);
  let tested, checks;
  if (host === 'web') [tested, ...checks] = await Promise.all([tests(host, app), ...WEB_SIZES.map(launch)]);
  else { tested = await tests(host, app); checks = [await launch(null, 0)]; }
  checks = checks.flat();
  const kept = checks.filter((c) => c.shot && existsSync(c.shot)).map((c) => c.shot);
  const r = report({ host, built, tested, checks, shots: kept, ms: Date.now() - started });
  if (json) console.log(JSON.stringify({ host, built: built && { ms: built.ms, said: built.said ?? [], warnings: built.warnings }, tests: tested, checks, screenshots: kept, problems: r.problems, exit: r.exit }));
  else say(r.lines);
  return r.exit;
}

if (import.meta.main) main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { console.error(e.message); process.exit(1); });
