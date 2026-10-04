#!/usr/bin/env bun
// The authoring diary's sending half (docs/diary.md). An app's agent keeps
// diaries in `.exact/diary/`; its `exact.mjs` appends each command to
// `.exact/commands.jsonl`. This assembles what has not been sent yet, redacts
// it, and sends it only when asked.
//
//   feedback [preview]      print exactly what `send` would send
//   feedback send [--yes]   send it (asks first on a terminal unless --yes)
//   feedback delete <id>    delete a sent diary by its receipt
//   feedback always|never|ask|local   this project's standing answer
//   feedback status         that answer, and what is unsent
//
// The standing answer is per user and per project (~/.config/exact/feedback.json,
// keyed by the app's real path), so cloning an app never carries consent along. A
// `*` key there answers `local` or `never` (and nothing else) for every project
// without its own; a study that hands an agent a private EXACT_CONFIG_DIR sets
// `{"*": "local"}` before the app exists.
// `local` keeps the diary and never asks or sends. With EXACT_DIARY=detailed set,
// `status` also prints the detailed diary's extra instructions (DETAILED below), so
// only a study's agents carry them in context.
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, resolve } from 'node:path';
import { createInterface } from 'node:readline/promises';

export const ENDPOINT = process.env.EXACT_FEEDBACK_URL ?? 'https://exact-diaries.ccheever-0f6.workers.dev';
const SETTINGS = () => resolve(process.env.EXACT_CONFIG_DIR ?? resolve(homedir(), '.config/exact'), 'feedback.json');

const readJson = (path, fallback) => { try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return fallback; } };

export const ANSWERS = ['always', 'never', 'ask', 'local'];
// A `*` entry can only withhold: it answers `local` or `never`, never a consent to send.
const wildcard = all => (['local', 'never'].includes(all['*']) ? all['*'] : undefined);
export function standing(dir) { const all = readJson(SETTINGS(), {}); return all[realpathSync(dir)] ?? wildcard(all) ?? 'ask'; }
export function setStanding(dir, answer) {
  const all = readJson(SETTINGS(), {});
  // `ask` under a `*` entry is kept explicitly, so `feedback ask` undoes `local` or `never` from either.
  if (answer === 'ask' && !wildcard(all)) delete all[realpathSync(dir)]; else all[realpathSync(dir)] = answer;
  mkdirSync(dirname(SETTINGS()), { recursive: true });
  writeFileSync(SETTINGS(), JSON.stringify(all, null, 2) + '\n');
}

/** What identifies this app, longest first so a path is replaced before a name inside it. */
function identifiers(dir) {
  const manifest = readJson(resolve(dir, 'app.json'), {});
  const name = basename(dir);
  const words = [name, name.replaceAll('-', '_'), manifest.name, manifest.short_name, manifest.id, manifest.app?.id, manifest.app?.name]
    .filter(w => typeof w === 'string' && w.length >= 3);
  return [...new Set([realpathSync(dir), resolve(dir), ...words])].sort((a, b) => b.length - a.length);
}

const SECRETS = [
  /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g,
  /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g,
  /\b(?:sk|pk|rk)[-_](?:live|test|ant|proj)?[-_]?[A-Za-z0-9_-]{16,}/g,
  /\b(?:ghp|gho|ghu|ghs|github_pat)_[A-Za-z0-9_]{20,}/g,
  /\bxox[abposr]-[A-Za-z0-9-]{10,}/g,
  /https:\/\/hooks\.slack\.com\/\S+/g,
  /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g,
  /\b((?:api[_-]?key|secret|token|password|passwd|auth)\w*\s*[:=]\s*)["']?[^\s"']{8,}/gi,
];

/** The home directory, the app's directory and its names become placeholders;
 * anything shaped like a credential is blanked. A net under the diary's own rules. */
export function redact(text, dir) {
  for (const id of identifiers(dir)) {
    const pattern = id.includes('/') ? escape(id) : `(?<![A-Za-z0-9])${escape(id)}(?![A-Za-z0-9])`;
    text = text.replace(new RegExp(pattern, 'gi'), '<app>');
  }
  // However the app's directory was spelled (a symlinked /var, a relative path), what led to it goes too.
  text = text.replace(/(?:~|\.{0,2})(?:\/[^\s/'"`<>]+)*\/<app>/g, '<app>').replaceAll(homedir(), '~');
  for (const secret of SECRETS) text = text.replace(secret, (match, key) => typeof key === 'string' ? `${key}[redacted]` : '[redacted]');
  return text;
}
const escape = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Unsent diaries and command-log lines, and the text that would go. */
export function pending(dir) {
  const diaryDir = resolve(dir, '.exact/diary');
  const diaries = existsSync(diaryDir) ? readdirSync(diaryDir).filter(f => f.endsWith('.md')).sort() : [];
  const state = readJson(resolve(dir, '.exact/sent.json'), { commands: 0 });
  const log = existsSync(resolve(dir, '.exact/commands.jsonl')) ? readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').split('\n').filter(Boolean) : [];
  const commands = log.slice(state.commands).map(safeParse).filter(Boolean);
  const parts = diaries.map(f => `# ${f}\n\n${readFileSync(resolve(diaryDir, f), 'utf8').trim()}\n`);
  if (commands.length) parts.push(`# commands\n\n${commands.map(c => `${c.at} ${c.exact2 ?? '?'} ${c.verb} exit=${c.exit} ${(c.ms / 1000).toFixed(1)}s`).join('\n')}\n`);
  const text = parts.length ? redact(`platform: ${process.platform} ${process.arch}\n\n${parts.join('\n')}`, dir) : '';
  return { diaries, commands, logLength: log.length, text };
}
const safeParse = line => { try { return JSON.parse(line); } catch { return null; } };

async function confirm(question) {
  if (!process.stdin.isTTY) return false;
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try { return /^y(es)?$/i.test((await rl.question(question)).trim()); } finally { rl.close(); }
}

export async function send(dir, { yes = false, fetch: post = fetch } = {}) {
  if (standing(dir) === 'never') return 'This project is set to never send feedback (`feedback ask` undoes that).';
  if (standing(dir) === 'local') return 'This project keeps its diary on this machine and never sends it (`feedback ask` undoes that).';
  const { diaries, commands, logLength, text } = pending(dir);
  if (!diaries.length && !commands.length) return 'Nothing unsent.';
  if (!yes) {
    console.log(`${text}\n---\nThis goes to ${ENDPOINT}.`);
    if (!(await confirm('Send it? [y/N] '))) return 'Not sent.';
  }
  const exact2 = commands.findLast(c => c.exact2)?.exact2 ?? null;
  const response = await post(`${ENDPOINT}/diary`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ version: 1, exact2, text }) });
  if (!response.ok) throw new Error(`the feedback endpoint answered ${response.status}; nothing was marked sent`);
  const { id } = await response.json();
  const sentDir = resolve(dir, '.exact/diary/sent');
  mkdirSync(sentDir, { recursive: true });
  for (const f of diaries) renameSync(resolve(dir, '.exact/diary', f), resolve(sentDir, f));
  writeFileSync(resolve(dir, '.exact/sent.json'), JSON.stringify({ commands: logLength, receipts: [...(readJson(resolve(dir, '.exact/sent.json'), {}).receipts ?? []), id] }) + '\n');
  return `Sent ${diaries.length} ${diaries.length === 1 ? 'diary' : 'diaries'} and ${commands.length} logged commands. Receipt: ${id}\nTo delete it: bun exact.mjs feedback delete ${id}`;
}

/** The detailed diary (EXACT_DIARY=detailed): what a study of authoring needs beyond docs/diary.md. */
export const DETAILED = `This session keeps the detailed diary. In the same diary file, also keep:

- A timeline: a line at each step, stamped with the time from \`date '+%F %T'\`. Run it each time rather than estimating.
- For every error: the command, the first lines of its output, what you believed was wrong, each attempt, and what worked.
- Every doc you read: what you were looking for, and whether it was there.
- Every guess you made where the docs were silent. Later, mark each one right or wrong.
- Every workaround left in the app.
- At the end, a self-assessment for each platform: what works, how you checked it, and what you are unsure of.`;

/** The standing answer and what is unsent; the detailed diary's instructions when a study asks for them. */
export function status(dir, env = process.env) {
  const { diaries, commands } = pending(dir);
  const answer = standing(dir);
  const line = answer === 'local' ? `local: ${diaries.length} unsent diaries, kept on this machine; ${commands.length} logged commands`
    : `${answer}: ${diaries.length} unsent diaries, ${commands.length} unsent logged commands`;
  return env.EXACT_DIARY === 'detailed' && answer !== 'never' ? `${line}\n\n${DETAILED}` : line;
}

async function main([verb = 'preview', ...rest]) {
  const dir = process.env.EXACT_APP_DIR;
  if (!dir) throw new Error('run this through an app\'s exact.mjs (bun exact.mjs feedback …)');
  if (verb === 'preview') { const { text } = pending(dir); return text || 'Nothing unsent.'; }
  if (verb === 'send') return send(dir, { yes: rest.includes('--yes') });
  if (verb === 'status') return status(dir);
  if (ANSWERS.includes(verb)) { setStanding(dir, verb); return `This project's feedback answer is now: ${verb}.`; }
  if (verb === 'delete' && rest[0]) {
    const response = await fetch(`${ENDPOINT}/diary/${encodeURIComponent(rest[0])}`, { method: 'DELETE' });
    if (!response.ok) throw new Error(`the feedback endpoint answered ${response.status}`);
    return `Deleted ${rest[0]}.`;
  }
  throw new Error('usage: bun exact.mjs feedback [preview|send [--yes]|delete <id>|always|never|ask|local|status]');
}

if (import.meta.main) {
  try { console.log(await main(process.argv.slice(2))); }
  catch (e) { console.error(`feedback: ${e.message}`); process.exit(1); }
}
