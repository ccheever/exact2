// Feedback in one line (docs/diary.md, "A note"): what an app's commands say after friction or a finished task,
// and the entry `feedback note "<text>"` adds to the diary, which carries no command output. A note follows the
// project's standing answer through feedback.mjs's own redaction and send; nothing here sends. EXACT_FEEDBACK=0
// silences the nudges and refuses every send, on any project.
import { spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { ENDPOINT, SETTINGS, redact, standing } from './feedback.mjs';

export const silenced = (env = process.env) => env.EXACT_FEEDBACK === '0';

/** The verbs whose failure or slowness is friction worth a note, and how long each may run before it is slow (ms):
 * a web build or drive takes seconds, a native build or a verify minutes. */
const SLOW = { 'web-build': 3 * 60e3, test: 5 * 60e3, agent: 5 * 60e3, verify: 15 * 60e3, ios: 20 * 60e3, mac: 20 * 60e3, linux: 20 * 60e3, windows: 20 * 60e3, android: 20 * 60e3 };
const NATIVE = new Set(['ios', 'macos', 'mac', 'linux', 'windows', 'android']);
const slowAfter = (verb, host) => (['test', 'agent'].includes(verb) && NATIVE.has(host) ? 20 * 60e3 : SLOW[verb]);

const NOTE = 'bun exact.mjs feedback note "<what happened>"';
export const FRICTION = `exact: if this was unclear or slow, tell the exact2 team in one line: ${NOTE}`;
export const DONE = `exact: if anything in this task was confusing or needed a workaround: ${NOTE}`;
export const DISCLOSURE = `exact: a feedback note stays in .exact/ and goes to Expo (${ENDPOINT}) only as this project's answer allows (bun exact.mjs feedback status says it); EXACT_FEEDBACK=0 or bun exact.mjs feedback never stops it.`;

/** The line said once per machine, before the first nudge or note: where feedback goes and how to stop it. */
function disclosure() {
  const seen = resolve(dirname(SETTINGS()), 'feedback-disclosed');
  // Claimed by creating the file, so two commands at once say it once.
  try { mkdirSync(dirname(seen), { recursive: true }); writeFileSync(seen, `${new Date().toISOString()}\n`, { flag: 'wx' }); } catch { return []; }
  return [DISCLOSURE];
}

/** The lines to say after a command of the app in `dir` ended: none, or a nudge toward `feedback note` (with the
 * machine's disclosure the first time). Friction is a drive, build or verify that failed or ran slow; a finished task
 * is a `test` or `verify` that passed. Each is said at most once a day (UTC) per project, and never under `never` or
 * EXACT_FEEDBACK=0. An interrupted command (a signal's code, or `interrupted`) says nothing. `args` are the command's,
 * for `--run`. */
export function nudge(dir, { verb, host = 'web', exit, ms, args = [], interrupted = false }, { env = process.env, now = Date.now() } = {}) {
  if (silenced(env) || !(verb in SLOW) || exit >= 128 || interrupted) return [];
  let answer; try { answer = standing(dir); } catch { return []; }
  if (answer === 'never') return [];
  // A command that stays to run the app (`mac --run`) is as long as the person keeps it open: only its failure counts.
  const slow = ms >= slowAfter(verb, host) && !args.some(a => ['--run', '--watch'].includes(a));
  const kind = exit !== 0 || slow ? 'friction' : ['test', 'verify'].includes(verb) ? 'done' : null;
  if (!kind) return [];
  // A day's nudge of each kind is claimed by creating its file (`.exact/nudged/friction-2026-11-01`), so two commands
  // at once say it once; an earlier day's is removed.
  const nudged = resolve(dir, '.exact/nudged'), day = `${kind}-${new Date(now).toISOString().slice(0, 10)}`;
  try { mkdirSync(nudged, { recursive: true }); writeFileSync(resolve(nudged, day), '', { flag: 'wx' }); } catch { return []; }
  for (const old of readdirSync(nudged)) if (old.startsWith(`${kind}-`) && old !== day) rmSync(resolve(nudged, old), { force: true });
  return [...disclosure(), kind === 'friction' ? FRICTION : DONE];
}

/** What a note attaches: the exact2 checkout's commit, the last command's verb, exit code and time (the diary's
 * command log), and the machine's kind. Never an argument, any output, or a source or file of the app: the agent
 * writes what went wrong itself. */
export function context(dir, exact2 = resolve(import.meta.dir, '..')) {
  const commit = spawnSync('git', ['-C', exact2, 'rev-parse', '--short=9', 'HEAD'], { encoding: 'utf8' }).stdout?.trim() || 'unknown';
  const last = (() => { try { return JSON.parse(readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n').at(-1)); } catch { return null; } })();
  return {
    exact2: commit,
    last: last ? `${last.verb}, exit ${last.exit}, ${((last.ms ?? 0) / 1000).toFixed(1)}s` : 'none logged',
    host: `${process.platform}${process.stdin.isTTY ? ', noted from a terminal' : ', noted with no terminal (an agent or a script)'}`,
  };
}

/** The diary entry for `text`, redacted, appended to `.exact/diary/<date>-notes.md`, so `feedback send` sends it with
 * the rest of the diary. The file's path and the entry. */
export function addNote(dir, text, { now = new Date(), exact2 } = {}) {
  const c = context(dir, exact2);
  const entry = redact([
    `### Note ${now.toISOString().slice(0, 16).replace('T', ' ')}Z`,
    '',
    text.trim(),
    '',
    `- exact2: ${c.exact2}`,
    `- last command: ${c.last}`,
    `- machine: ${c.host}`,
  ].join('\n'), dir);
  const file = resolve(dir, '.exact/diary', `${now.toISOString().slice(0, 10)}-notes.md`);
  mkdirSync(dirname(file), { recursive: true });
  appendFileSync(file, `${existsSync(file) ? '\n' : '# Notes\n\n'}${entry}\n`);
  return { file, entry };
}

/** `feedback note "<text>"`: the entry is kept, then the standing answer decides: `always` sends the unsent diary,
 * `ask` asks on a terminal and says how to send otherwise, `local` keeps it, `never` keeps nothing. */
export async function note(dir, text, { send, env = process.env, terminal = process.stdin.isTTY, say = console.log } = {}) {
  if (!text.trim()) throw new Error('usage: bun exact.mjs feedback note "<what happened>" (one line: what you tried, what went wrong or was slow, the first line of an error, if short)');
  const answer = standing(dir);
  if (answer === 'never') return 'This project is set to never send feedback, so the note was not kept (`bun exact.mjs feedback ask` undoes that).';
  // Said before anything is sent, so a failed send cannot swallow it.
  for (const line of disclosure()) say(line);
  const { file } = addNote(dir, text);
  const kept = `Noted in ${relative(dir, file)}.`;
  if (silenced(env)) return `${kept} EXACT_FEEDBACK=0 is set, so it stays on this machine.`;
  if (answer === 'local') return `${kept} This project keeps feedback on this machine.`;
  if (answer === 'always') return `${kept}\n${await send(dir, { yes: true })}`;
  if (terminal) return `${kept}\n${await send(dir)}`;
  return `${kept} It is not sent: ask the person you work for, then \`bun exact.mjs feedback send --yes\` (\`bun exact.mjs feedback\` shows what would go).`;
}
