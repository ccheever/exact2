#!/usr/bin/env bun
// A native drive's build, before the drive (LLP 1116.002 D4). The driver
// refuses a build its own record shows is missing or stale, and never builds
// (LLP 1012.001.000 D1: a cold build in `agent macos tree` would be minutes
// unexplained). An app's `exact.mjs` runs this first for `test` and `agent` on
// ios and macos, as it runs web-build before a web drive: a fresh build costs
// the check and says nothing; a missing or stale one is built by the command
// `bun exact.mjs ios|mac` runs, said in one stderr line. The build's output
// goes to the app's `.exact/<host>-build.log`, and to stderr, summarised, only
// when it fails. stdout stays the drive's (`--json`).
//
//   bun scripts/agent-build.mjs <ios|macos> --app <name> [--sim <udid|name>]

import { spawn } from 'node:child_process';
import { createWriteStream, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { appleArtifacts, appleRunBinary } from '../host/apple/build.mjs';
import { receiptChanges, refuseStale } from './agent-launch.mjs';
import { claimLive, resolveApp } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const label = host => host === 'ios' ? 'iOS' : 'macOS';

/** What a drive of `app` on `host` (`ios`, else macOS) launches, by its own
 * record: `missing`, or the inputs `changed` since its receipt ([] when fresh,
 * or when a macOS binary has no receipt). `sample` is the sample host's. */
export function nativeBuild(host, app, { sample = false } = {}) {
  const ios = host === 'ios';
  const built = ios ? appleArtifacts(app, { destination: 'ios-simulator', host: sample }).bundle : appleRunBinary(app, sample);
  if (!existsSync(built)) return { built, missing: true, changed: [] };
  const receipt = ios ? resolve(built, 'receipt.json')
    : [resolve(built, '..', 'receipt.json'), resolve(appleArtifacts(app, { host: sample }).bundle, 'Contents/Resources/receipt.json')].find(existsSync);
  return { built, receipt, changed: receipt ? receiptChanges(receipt, app) : [] };
}

/** Why a drive must build first, or null when it need not: no build yet, or
 * one older than its sources (up to three named, from the app's directory). */
export function buildReason(state, app) {
  if (state.missing) return 'there is no build yet';
  if (!state.changed.length) return null;
  const near = path => { const at = resolve(ROOT, path); return app?.dir && at.startsWith(`${app.dir}/`) ? relative(app.dir, at) : path; };
  const more = state.changed.length > 3 ? ` and ${state.changed.length - 3} more` : '';
  return `the build is older than its sources (${state.changed.slice(0, 3).map(near).join(', ')}${more} changed)`;
}

/** The command that builds `app` for a drive (`ios` or `mac`): an `exact new`
 * app's own `exact.mjs` verb, else exact2's build. An `exact.mjs` written
 * before test and agent built first is told how to get that. */
export function rebuildCommand(app, verb) {
  const own = app.dir && !resolve(app.dir).startsWith(`${ROOT}/`) && existsSync(resolve(app.dir, 'exact.mjs'));
  if (!own) return `bun host/apple/build.mjs${verb === 'ios' ? ' --ios' : ''} ${app.crate('apple')}`;
  const command = `(cd '${String(app.dir).replaceAll("'", "'\\''")}' && bun exact.mjs ${verb})`;
  return readFileSync(resolve(app.dir, 'exact.mjs'), 'utf8').includes('agent-build.mjs') ? command
    : `${command} (after bun exact.mjs update, test and agent build it themselves)`;
}

/** The driver's refusal (LLP 1012.001.000 D1) on what `nativeBuild` found. */
export function refuseBuild(host, state, app, { sample = false } = {}) {
  const ios = host === 'ios';
  const command = sample ? `bun host/apple/build.mjs${ios ? ' --ios' : ''} ${app.crate('apple')} --host` : rebuildCommand(app, ios ? 'ios' : 'mac');
  if (state.missing) throw new Error(`run ${command} first`);
  refuseStale(ios ? 'ios' : sample ? 'sample host' : 'macos', state.receipt, state.changed, command);
}

/** host/apple/build.mjs's arguments for the build `bun exact.mjs ios|mac` runs. */
export const buildArgs = (host, app, sim) => [...(host === 'ios' ? ['--ios'] : []), app.crate('apple'), ...(host === 'ios' && sim ? ['--sim', sim] : [])];

/** What a failed build said, short: from its first error, at most 30 lines (else
 * its last 20), and its last line; then that it failed and where its log is. */
export function failureSummary(output, { host, code, log }) {
  const lines = output.split('\n').filter(line => line.trim());
  const first = lines.findIndex(line => /^error\b|\berror(\[E\d+\])?:/.test(line));
  const shown = first < 0 ? lines.slice(-20) : lines.slice(first, first + 30);
  if (lines.length && !shown.includes(lines.at(-1))) shown.push('…', lines.at(-1));
  return [...shown, `the ${label(host)} build failed (exit ${code}), so nothing was driven; its whole log: ${log}`].join('\n');
}

/** host/apple/build.mjs with `args`, its output into `log`; its exit code and output. */
function appleBuild(args, log) {
  mkdirSync(resolve(log, '..'), { recursive: true });
  const file = createWriteStream(log), chunks = [];
  const child = spawn(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), ...args], { stdio: ['ignore', 'pipe', 'pipe'] });
  const pass = signal => child.kill(signal), signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  for (const signal of signals) process.on(signal, pass);
  for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => { file.write(chunk); chunks.push(chunk); });
  return new Promise(done => {
    child.on('error', error => chunks.push(Buffer.from(String(error.message))));
    child.on('close', (code, signal) => {
      for (const s of signals) process.off(s, pass);
      file.end(() => done({ code: code ?? (signal ? 128 : 1), output: Buffer.concat(chunks).toString('utf8') }));
    });
  });
}

/** A build of this app already running (one started in the background) ends
 * before the check, so the drive neither races it nor is refused as busy. */
async function waitForBuild(app, say) {
  const lock = appleArtifacts(app).lock;
  for (let told = false; ; await new Promise(r => setTimeout(r, 250))) {
    let held; try { held = readFileSync(lock, 'utf8'); } catch { return; }
    if (!claimLive(held)) return;
    if (!told) { told = true; say(`waiting for the Apple build of ${app.name} already running`); }
  }
}

/** Builds first when the drive would refuse (`nativeBuild`): one line on `say`,
 * the build's output in the app's `.exact/`, its summary when it fails; the
 * exit code. A fresh build runs nothing and says nothing. */
export async function buildFirst(host, app, { sim, say = line => console.error(line), build = appleBuild, wait = waitForBuild } = {}) {
  host = host === 'ios' ? 'ios' : 'macos';
  if (host === 'macos' && process.env.EXACT_MAC_BIN) return 0; // the caller's own binary: the driver says it did not check
  await wait(app, say);
  const reason = buildReason(nativeBuild(host, app), app);
  if (!reason) return 0;
  const log = resolve(app.dir, '.exact', `${host}-build.log`), shown = relative(process.cwd(), log).startsWith('..') ? log : relative(process.cwd(), log);
  say(`building for ${label(host)}: ${reason}; its log: ${shown}`);
  const { code, output } = await build(buildArgs(host, app, sim), log);
  if (code) say(failureSummary(output, { host, code, log: shown }));
  return code;
}

if (import.meta.main) {
  const args = process.argv.slice(2), value = flag => args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined;
  if (!['ios', 'macos', 'mac'].includes(args[0])) {
    console.error('usage: bun scripts/agent-build.mjs <ios|macos> --app <name> [--sim <udid|name>]');
    process.exit(2);
  }
  Promise.resolve().then(() => buildFirst(args[0], resolveApp(value('--app')), { sim: value('--sim') }))
    .then(code => process.exit(code), error => { console.error(error.message); process.exit(1); });
}
