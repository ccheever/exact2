#!/usr/bin/env bun
/**
 * known — which of a local run's failures also fail on origin/main (LLP 1146 D1).
 *
 * The async lane attaches what it found at each commit it checks to that commit as a git note
 * (`refs/notes/async`, tier 2's `refs/notes/async2`; `publish` below), pushed to origin. This runs a
 * test command, prints its output as it comes, names its failures as the lane does (async.mjs
 * `failures`), and sorts each against the nearest first-parent ancestor of HEAD on origin/main whose
 * note ran a check of the same kind to the end:
 *
 *   known      it failed there too, with how often it failed in the last ten such notes
 *   new        that check ran there and this test did not fail
 *   unknown    no note there ran such a check to the end, so nothing is known of it
 *
 * The exit status is the command's: a known failure is still a failure.
 *
 *   bun scripts/known.mjs [--no-fetch] -- <command> [args…]
 *     e.g. bun scripts/known.mjs -- cargo test -p exact-runner --lib
 *          bun scripts/known.mjs -- bun host/apple/build.mjs --test --ios
 *   bun scripts/known.mjs [--no-fetch] --log <file> --kind cargo|xctest|bun
 */
import { spawn, spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { failures } from './async.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
export const NOTE_REFS = ['refs/notes/async', 'refs/notes/async2'];
/** How many of a check's latest notes a failure's rate is counted over. */
const RATE_OVER = 10;

const git = (args, cwd = ROOT) => spawnSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });

/** A failure without its check's name: `ios: ExactKitTests.A b failed` and `local: ExactKitTests.A b failed` are one test. */
export const identity = (failure) => failure.slice(failure.indexOf(': ') + 2);

/** The kind of test a check's command runs, which a local command of the same kind is compared with. */
export function kindOf(command) {
  if (/\bbuild\.mjs\b.*--test\b/.test(command)) return 'xctest';
  if (/\bcargo\b.*\btest\b/.test(command)) return 'cargo';
  if (/\bbun\b.*\btest\b/.test(command)) return 'bun';
  return null;
}

/** A lane's note for one commit: what each check ran and whether it ran to the end, and every failure. A check that
 *  stopped early (did not compile, hung, or failed with nothing a pattern names) says nothing of the tests it held. */
export function noteOf(result, tier) {
  const checks = {};
  for (const [name, c] of Object.entries(result.checks)) {
    const own = result.failures.filter((f) => f.startsWith(`${name}: `));
    const stopped = own.some((f) => /: exit \d+ \(see log\)$| does not compile$| hung( past .*)?$/.test(f));
    checks[name] = c.skipped ? { command: c.command, skipped: c.skipped } : { command: c.command, status: c.status, complete: !stopped };
  }
  return { sha: result.sha, tier, at: new Date().toISOString(), checks, failures: result.failures };
}

/** Attach `note` to its commit and push the lane's ref to origin. The lane is the ref's only writer. A failed push is
 *  said and retried with the next commit's. */
export function publish(note, cwd = ROOT) {
  const ref = NOTE_REFS[note.tier === 2 ? 1 : 0];
  const add = git(['notes', `--ref=${ref}`, 'add', '-f', '-m', JSON.stringify(note), note.sha], cwd);
  if (add.status !== 0) return `git notes add: ${add.stderr.trim()}`;
  const push = git(['push', '-q', 'origin', ref], cwd);
  return push.status === 0 ? null : `git push ${ref}: ${push.stderr.trim()}`;
}

/** The lanes' notes on the first parents of origin/main's merge base with HEAD, newest first: `[{ sha, notes }]`. */
export function mainNotes({ fetch = true, depth = 400, cwd = ROOT } = {}) {
  if (fetch) git(['fetch', '-q', 'origin', '+refs/notes/async*:refs/notes/async*'], cwd);
  const base = git(['merge-base', 'HEAD', 'origin/main'], cwd).stdout.trim();
  if (!base) return [];
  const log = git(['log', '--first-parent', `-n${depth}`, ...NOTE_REFS.map((r) => `--notes=${r}`), '--format=%H%n%N%x1e', base], cwd).stdout;
  const out = [];
  for (const entry of log.split('\x1e')) {
    const [sha, ...lines] = entry.trim().split('\n');
    const notes = lines.flatMap((l) => { try { const n = JSON.parse(l); return n && n.checks ? [n] : []; } catch { return []; } });
    if (sha && notes.length) out.push({ sha, notes });
  }
  return out;
}

/** Each local failure sorted against `history` (`mainNotes`) for checks of `kind`. */
export function classify(local, kind, history) {
  const covering = (n) => Object.values(n.checks).some((c) => !c.skipped && c.complete && kindOf(c.command) === kind);
  const seen = history.flatMap(({ sha, notes }) => notes.filter(covering).map((n) => ({ sha, note: n })));
  const at = seen[0] ?? null, recent = seen.slice(0, RATE_OVER);
  const rows = local.map((failure) => {
    const id = identity(failure);
    if (!at) return { failure, verdict: 'unknown' };
    const failedIn = recent.filter(({ note }) => note.failures.some((f) => identity(f) === id)).length;
    return at.note.failures.some((f) => identity(f) === id)
      ? { failure, verdict: 'known', rate: `${failedIn} of its last ${recent.length} lane runs` }
      : { failure, verdict: 'new', ...(failedIn ? { rate: `${failedIn} of the last ${recent.length} lane runs` } : {}) };
  });
  return { at: at && { sha: at.sha, when: at.note.at, tier: at.note.tier }, rows };
}

/** One line for the run, then each failure that is not known. */
export function report({ at, rows }) {
  if (!rows.length) return [];
  const count = (v) => rows.filter((r) => r.verdict === v).length;
  if (!at) return [`${rows.length} failed: unknown on origin/main (no lane note there ran this kind of check to the end; \`git fetch origin 'refs/notes/async*:refs/notes/async*'\` fetches them)`];
  const lines = [`${rows.length} failed: ${count('known')} known on origin/main at ${at.sha.slice(0, 10)} (lane${at.tier === 2 ? ' tier 2' : ''}, ${at.when?.slice(0, 16).replace('T', ' ') ?? 'undated'}), ${count('new')} new${count('unknown') ? `, ${count('unknown')} unknown` : ''}`];
  for (const r of rows) lines.push(`  ${r.verdict.padEnd(5)} ${identity(r.failure)}${r.rate ? ` (failed ${r.rate})` : ''}`);
  return lines;
}

if (import.meta.main) {
  // Its own flags lead; the command is what follows them, after a `--` or not (Bun keeps no `--` for a script).
  const argv = process.argv.slice(2), own = { '--no-fetch': 0, '--kind': 1, '--log': 1 }, flags = {};
  let i = 0;
  while (i < argv.length && argv[i] in own) { flags[argv[i]] = own[argv[i]] ? argv[i + 1] : true; i += 1 + own[argv[i]]; }
  if (argv[i] === '--') i++;
  const fetch = !flags['--no-fetch'], opt = (name) => flags[name];
  let log, status, kind;
  if (i < argv.length) {
    const [command, ...args] = argv.slice(i);
    kind = opt('--kind') ?? kindOf([command, ...args].join(' '));
    if (!kind) throw new Error(`known: ${command} ${args.join(' ')} is no kind the lane runs (cargo test, bun test, build.mjs --test); pass --kind`);
    ({ log, status } = await new Promise((done) => {
      const child = spawn(command, args, { stdio: ['inherit', 'pipe', 'pipe'] });
      let out = '';
      child.stdout.on('data', (d) => { out += d; process.stdout.write(d); });
      child.stderr.on('data', (d) => { out += d; process.stderr.write(d); });
      child.on('close', (code) => done({ log: out, status: code ?? 1 }));
    }));
  } else if (opt('--log')) {
    log = readFileSync(opt('--log'), 'utf8'); status = 1; kind = opt('--kind');
    if (!['cargo', 'xctest', 'bun'].includes(kind)) throw new Error('known --log <file>: name its --kind (cargo, xctest or bun)');
  } else {
    console.log('Usage: bun scripts/known.mjs [--no-fetch] -- <command> [args…] | --log <file> --kind cargo|xctest|bun');
    process.exit(2);
  }
  const local = status === 0 ? [] : failures('local', log, status);
  for (const line of report(classify(local, kind, mainNotes({ fetch })))) console.error(line);
  process.exit(status);
}
