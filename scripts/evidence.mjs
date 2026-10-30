#!/usr/bin/env bun
/**
 * evidence — what has been run against an app's current sources (LLP 1146 D2).
 *
 * Each drive and test run (`agent.mjs`, `verify`) appends a line to `<app>/.exact/runs.jsonl`: when, on which host and
 * target, on which clock, what ran, what it found, the commit, and the build it ran on, named by the digest of that
 * build's record (the web's `dist/.exact-build.json`, an Apple build's `receipt.json`). The driver refuses a build
 * older than its sources, so a run's build was fresh when it ran. A run is current while that build is still the
 * host's build and still fresh: the same checks the driver makes (`webChanges`, `nativeBuild`) say what made it stale.
 * Linux, Windows, Android, tvOS and phone builds are not identified yet; their runs are listed as unchecked.
 *
 *   bun scripts/evidence.mjs [--app <name>] [--since <rev>] [--draft]
 *   bun scripts/evidence.mjs note <host> "<what was seen>" [--app <name>] [--device "<model, OS>"]
 */
import { createHash } from 'node:crypto';
import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { resolveApp, webDist as defaultWebDist } from './app.mjs';
import { nativeBuild, receiptPath } from './agent-build.mjs';
import { webChanges } from './agent-launch.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const git = (...args) => spawnSync('git', args, { cwd: ROOT, encoding: 'utf8' }).stdout?.trim() ?? '';
const digest = (path) => createHash('sha256').update(readFileSync(path)).digest('hex').slice(0, 16);
const runsOf = (app) => resolve(app.dir, '.exact', 'runs.jsonl');

/** The record of the build a run on `host` uses: its path, or null where builds are not identified. */
function buildRecord(app, host, { webDist, device } = {}) {
  if (device) return null;
  if (host === 'web') { const marker = resolve(webDist ?? defaultWebDist(), '.exact-build.json'); return existsSync(marker) ? marker : null; }
  if (host === 'macos' || host === 'ios') return receiptPath(host, app) ?? null;
  return null;
}

/** Append one run to the app's record. Never throws: a run's result does not depend on its record. */
export function recordRun(appName, { host, what, result, timing = 'agent', touch, device, sim, target, webDist, kind = 'run' }) {
  try {
    const app = resolveApp(appName), record = buildRecord(app, host, { webDist, device: device || !!target });
    const line = { at: new Date().toISOString(), kind, host, target: target ?? (device ? 'device' : host === 'ios' ? sim ?? process.env.EXACT_SIM ?? 'simulator' : host === 'web' ? process.env.EXACT_WEB_BROWSER ?? 'chrome' : host),
      timing, ...(touch && touch !== 'agent' ? { touch } : {}), what, result, commit: git('rev-parse', '--short=10', 'HEAD'), build: record ? digest(record) : null };
    mkdirSync(resolve(app.dir, '.exact'), { recursive: true });
    appendFileSync(runsOf(app), JSON.stringify(line) + '\n');
  } catch { /* the record is advice */ }
}

/** Every recorded run of `app`, oldest first. */
export function runs(app) {
  if (!existsSync(runsOf(app))) return [];
  return readFileSync(runsOf(app), 'utf8').split('\n').filter(Boolean).flatMap((l) => { try { return [JSON.parse(l)]; } catch { return []; } });
}

/** The host's build now: its id, and what changed since it was built ([] when fresh); null where unidentified. */
function buildNow(app, host) {
  if (host === 'web') {
    const dist = defaultWebDist(), marker = resolve(dist, '.exact-build.json');
    return existsSync(marker) ? { id: digest(marker), changed: webChanges(dist, app).all } : null;
  }
  if (host === 'macos' || host === 'ios') {
    const state = nativeBuild(host, app);
    return state.missing || !state.receipt ? null : { id: digest(state.receipt), changed: state.changed };
  }
  return null;
}

/** Each run with its standing: `current`, `stale` (and why), or `unchecked`. */
export function standing(app, all = runs(app), now = (host) => buildNow(app, host)) {
  const builds = new Map();
  return all.map((r) => {
    if (!r.build) return { ...r, standing: 'unchecked', why: ['web', 'macos', 'ios'].includes(r.host) ? 'a device build is not identified yet' : `a ${r.host} build is not identified yet` };
    if (!builds.has(r.host)) builds.set(r.host, now(r.host));
    const b = builds.get(r.host);
    if (!b) return { ...r, standing: 'stale', why: `no ${r.host} build is there now` };
    if (b.id !== r.build) return { ...r, standing: 'stale', why: 'a later build replaced the one it ran on' };
    if (b.changed.length) return { ...r, standing: 'stale', why: `${b.changed.slice(0, 3).join(', ')}${b.changed.length > 3 ? ` and ${b.changed.length - 3} more` : ''} changed after` };
    return { ...r, standing: 'current' };
  });
}

/** The hosts whose code a set of changed paths touches: shared layers touch every host. */
export function hostsTouched(paths, app) {
  const all = ['web', 'macos', 'ios', 'tvos', 'linux', 'windows', 'android'], out = new Set();
  const own = app && resolve(app.dir).startsWith(ROOT + '/') ? resolve(app.dir).slice(ROOT.length + 1) + '/' : null;
  for (const p of paths) {
    if (/^(runner|kernel|contract|plan|js|motion|num)\//.test(p) || (own && p.startsWith(own) && !/\/(apple|ios|macos|linux|windows|android|web)\//.test(p.slice(own.length - 1)))) all.forEach((h) => out.add(h));
    else if (/^host\/apple\//.test(p) || (own && /^(apple|ios|macos)\//.test(p.slice(own.length)))) ['macos', 'ios', 'tvos'].forEach((h) => out.add(h));
    else if (/^host\/web(-js)?\//.test(p) || (own && p.startsWith(`${own}web/`))) out.add('web');
    else if (/^host\/linux\//.test(p)) ['linux', 'windows', 'android'].forEach((h) => out.add(h));
    else if (/^host\/windows\//.test(p)) out.add('windows');
    else if (/^host\/android\//.test(p)) out.add('android');
  }
  return all.filter((h) => out.has(h));
}

const said = (r) => {
  const t = r.result;
  const found = r.kind === 'note' ? r.what : typeof t === 'string' ? `${r.what}: ${t}` : t?.tests ? `${r.what}: ${t.passed} passed, ${t.failed} failed${t.skipped ? `, ${t.skipped} skipped` : ''}` : `${r.what}: ${t?.ok ? 'ok' : `failed${t?.error ? ` (${t.error})` : ''}`}`;
  return `${r.host}${r.target && r.target !== r.host ? `, ${r.target}` : ''}: ${found}${r.kind === 'note' ? ' (seen by hand)' : ` (${r.timing} clock${r.touch ? `, ${r.touch} touch` : ''})`}`;
};

if (import.meta.main) {
  const argv = process.argv.slice(2), opt = (name) => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const app = resolveApp(opt('--app'));
  if (argv[0] === 'note') {
    const [, host, what] = argv;
    if (!host || !what) throw new Error('evidence note <host> "<what was seen>" [--device "<model, OS>"]');
    // Seen on a named device, the note stands for that device, whose build is not identified; else for the host's build now.
    recordRun(app.name, { host, what, result: null, kind: 'note', target: opt('--device'), timing: 'real' });
    console.log(`noted on ${host}: ${what}`);
    process.exit(0);
  }
  const since = opt('--since') ?? 'origin/main';
  const changed = [...new Set([...git('diff', '--name-only', `${since}...HEAD`).split('\n'), ...git('status', '--porcelain').split('\n').map((l) => l.slice(3))].filter(Boolean))];
  const all = standing(app);
  // The latest run of each kind of thing on each host and target is what stands; older ones repeat it.
  const latest = [...new Map(all.map((r) => [`${r.host}\n${r.target}\n${r.kind}\n${r.what}`, r])).values()];
  const lines = [];
  for (const r of latest) lines.push(`${said(r)}${r.standing === 'current' ? ', current' : `; ${r.standing}${r.why ? ` (${r.why})` : ''}`}`);
  const covered = new Set(latest.filter((r) => r.standing === 'current').map((r) => r.host));
  const owed = hostsTouched(changed, app).filter((h) => !covered.has(h));
  if (argv.includes('--draft')) {
    for (const l of lines.filter((l) => l.endsWith(', current'))) console.log(`- ${l.slice(0, -', current'.length)}`);
    for (const h of owed) console.log(`- ${h}: not run against these sources`);
  } else {
    console.log(`${app.name}: ${latest.length ? `${latest.length} run${latest.length === 1 ? '' : 's'} recorded` : 'no run recorded'} (${runsOf(app)})`);
    for (const l of lines) console.log(`  ${l}`);
    if (owed.length) console.log(`changed since ${since}, and no current run: ${owed.join(', ')}`);
  }
}
