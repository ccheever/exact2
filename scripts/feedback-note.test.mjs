import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { redact, send, setStanding } from './feedback.mjs';
import { DISCLOSURE, DONE, FRICTION, addNote, note, nudge } from './feedback-note.mjs';

function app() {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-feedback-note-'));
  const dir = resolve(parent, 'field-log');
  mkdirSync(resolve(dir, '.exact'), { recursive: true });
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Field Log', app: { id: 'com.acme.fieldlog' } }));
  writeFileSync(resolve(dir, 'app.contract'), 'view SecretLedgerScreen { text "top-secret-source" }\n');
  process.env.EXACT_CONFIG_DIR = resolve(parent, 'config');
  return { parent, dir };
}
const env = {};

test('a failed or slow drive, build or verify nudges once a day, a passing test or verify once too, and nothing else does', () => {
  const { parent, dir } = app();
  try {
    const now = Date.parse('2026-11-01T12:00:00Z');
    // The first nudge on a machine says where feedback goes and how to stop it, once.
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 1, ms: 2000 }, { env, now }), [DISCLOSURE, FRICTION]);
    assert.deepEqual(nudge(dir, { verb: 'ios', exit: 1, ms: 2000 }, { env, now: now + 1000 }), [], 'friction once a day');
    assert.deepEqual(nudge(dir, { verb: 'verify', exit: 0, ms: 2000 }, { env, now: now + 2000 }), [DONE], 'a finished task has its own');
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 0, ms: 2000 }, { env, now: now + 3000 }), [], 'and once a day too');
    assert.deepEqual(nudge(dir, { verb: 'mac', exit: 1, ms: 2000 }, { env, now: now + 86400e3 }), [FRICTION], 'a day later, again');
    rmSync(resolve(dir, '.exact/nudged'), { recursive: true });
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 1, ms: 2000, interrupted: true }, { env, now }), [], 'nor one a signal reached');
    assert.deepEqual(nudge(dir, { verb: 'agent', exit: 0, ms: 2000 }, { env, now }), [], 'a passing drive is not the end of a task');
    assert.deepEqual(nudge(dir, { verb: 'web', exit: 1, ms: 2000 }, { env, now }), [], 'the dev loop is not one of them');
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 130, ms: 2000 }, { env, now }), [], 'an interrupted command says nothing');
    assert.deepEqual(nudge(dir, { verb: 'mac', exit: 0, ms: 3600e3, args: ['--run'] }, { env, now }), [], 'a running app is not slow');
    assert.deepEqual(nudge(dir, { verb: 'web-build', exit: 0, ms: 4 * 60e3 }, { env, now }), [FRICTION], 'a slow build is friction');
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('never and EXACT_FEEDBACK=0 silence every nudge', () => {
  const { parent, dir } = app();
  try {
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 1, ms: 10 }, { env: { EXACT_FEEDBACK: '0' } }), []);
    setStanding(dir, 'never');
    assert.deepEqual(nudge(dir, { verb: 'test', exit: 1, ms: 10 }, { env }), []);
    assert.deepEqual(nudge(dir, { verb: 'verify', exit: 0, ms: 10 }, { env }), []);
    assert.ok(!existsSync(resolve(dir, '.exact/nudged')), 'and nothing is counted');
    assert.ok(!existsSync(resolve(process.env.EXACT_CONFIG_DIR, 'feedback-disclosed')));
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('a note attaches exact2\'s commit, the last command and the machine, redacted, and no output, source or file', () => {
  const { parent, dir } = app();
  try {
    writeFileSync(resolve(dir, '.exact/commands.jsonl'), `${JSON.stringify({ at: 't', exact2: 'abc', verb: 'test ios', exit: 1, ms: 12345 })}\n`);
    const { file, entry } = addNote(dir, 'line-clamp was blank on Field Log until I rebuilt', { now: new Date('2026-11-01T12:34:56Z') });
    assert.equal(file, resolve(dir, '.exact/diary/2026-11-01-notes.md'));
    const commit = spawnSync('git', ['-C', resolve(import.meta.dir, '..'), 'rev-parse', '--short=9', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
    assert.match(entry, new RegExp(`^### Note 2026-11-01 12:34Z\\n\\nline-clamp was blank on <app> until I rebuilt\\n\\n- exact2: ${commit}\\n- last command: test ios, exit 1, 12\\.3s\\n- machine: ${process.platform}, noted [^\\n]*$`));
    // The app's code never goes: no file of the app.
    for (const text of [entry, readFileSync(file, 'utf8')]) assert.doesNotMatch(text, /SecretLedgerScreen|top-secret-source/);
    // A second note the same day joins the same file.
    addNote(dir, 'second', { now: new Date('2026-11-01T13:00:00Z') });
    assert.equal(readFileSync(file, 'utf8').match(/### Note/g).length, 2);
    // The diary's redaction blanks a secret setting with any prefix.
    assert.equal(redact('DB_PASSWORD=hunter2 and EXACT_TOKEN: abc123', dir), 'DB_PASSWORD=[redacted] and EXACT_TOKEN: [redacted]');
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('a note follows the standing answer: always sends, ask says how, local keeps, never refuses, EXACT_FEEDBACK=0 keeps it here', async () => {
  const { parent, dir } = app();
  try {
    const sent = [];
    const fake = async (_, options) => (sent.push(options), 'Sent.');
    assert.match(await note(dir, 'slow ios build', { send: fake, env, terminal: false, say: () => {} }), /Noted in \.exact\/diary\/[\d-]+-notes\.md\. It is not sent: ask the person you work for, then `bun exact\.mjs feedback send --yes`/);
    assert.equal(sent.length, 0, 'ask never sends from an agent');
    await note(dir, 'on a terminal', { send: fake, env, terminal: true });
    assert.deepEqual(sent, [undefined], 'ask on a terminal asks through send');
    setStanding(dir, 'always');
    assert.match(await note(dir, 'docs silent on X', { send: fake, env, terminal: false }), /\nSent\.$/);
    assert.deepEqual(sent.at(-1), { yes: true });
    assert.match(await note(dir, 'opted out', { send: fake, env: { EXACT_FEEDBACK: '0' }, terminal: false }), /EXACT_FEEDBACK=0 is set, so it stays on this machine\.$/);
    assert.equal(sent.length, 2);
    setStanding(dir, 'local');
    assert.match(await note(dir, 'kept', { send: fake, env, terminal: false }), /keeps feedback on this machine/);
    const kept = readdirSync(resolve(dir, '.exact/diary'));
    setStanding(dir, 'never');
    assert.match(await note(dir, 'refused', { send: fake, env, terminal: false }), /set to never send feedback, so the note was not kept \(`bun exact\.mjs feedback ask` undoes that\)/);
    assert.deepEqual(readdirSync(resolve(dir, '.exact/diary')), kept);
    assert.doesNotMatch(readFileSync(resolve(dir, '.exact/diary', kept[0]), 'utf8'), /refused/);
    await assert.rejects(note(dir, '  ', { send: fake, env }), /usage: bun exact\.mjs feedback note/);
    // EXACT_FEEDBACK=0 refuses every send, whatever the project answered.
    setStanding(dir, 'always');
    const posts = [];
    assert.match(await send(dir, { yes: true, env: { EXACT_FEEDBACK: '0' }, fetch: async (...a) => (posts.push(a), { ok: true, json: async () => ({ id: 'r' }) }) }), /EXACT_FEEDBACK=0 is set, so nothing is sent/);
    assert.equal(posts.length, 0);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('a second note the same day is archived beside the first when sent, and a failed send still discloses first', async () => {
  const { parent, dir } = app();
  try {
    setStanding(dir, 'always');
    let ok = false;
    const said = [];
    const fetch = async () => (ok ? { ok: true, json: async () => ({ id: 'r' }) } : { ok: false, status: 503 });
    const post = (d, options) => send(d, { ...options, fetch });
    await assert.rejects(note(dir, 'first', { send: post, env, terminal: false, say: line => said.push(line) }), /503/);
    assert.deepEqual(said, [DISCLOSURE], 'said before the send failed');
    ok = true;
    await note(dir, 'second', { send: post, env, terminal: false, say: line => said.push(line) });
    await note(dir, 'third', { send: post, env, terminal: false, say: line => said.push(line) });
    assert.deepEqual(said, [DISCLOSURE], 'once');
    const sent = readdirSync(resolve(dir, '.exact/diary/sent'));
    assert.equal(sent.length, 1);
    assert.match(readFileSync(resolve(dir, '.exact/diary/sent', sent[0]), 'utf8'), /first[^]*second[^]*third/);
    // A note written while a send waits on the network is not archived with what went.
    let release;
    const slow = (d, options) => send(d, { ...options, fetch: () => new Promise(done => { release = () => done({ ok: true, json: async () => ({ id: 'r2' }) }); }) });
    const sending = note(dir, 'fourth', { send: slow, env, terminal: false, say: () => {} });
    while (!release) await new Promise(done => setTimeout(done, 5));
    addNote(dir, 'fifth, during the send');
    release();
    await sending;
    assert.doesNotMatch(readFileSync(resolve(dir, '.exact/diary/sent', sent[0]), 'utf8'), /fifth/);
    assert.match(readFileSync(resolve(dir, '.exact/diary', readdirSync(resolve(dir, '.exact/diary')).find(f => f.endsWith('.md'))), 'utf8'), /fifth, during the send/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('an app\'s command nudges at the end of stderr on failure and success, and a note carries none of its output', () => {
  const { parent, dir } = app();
  try {
    // A checkout whose ios build fails and whose verify passes, with the real feedback scripts.
    const sdk = resolve(parent, 'sdk'), root = resolve(import.meta.dir, '..');
    for (const file of ['feedback-note', 'feedback', 'help']) { mkdirSync(resolve(sdk, 'scripts'), { recursive: true }); symlinkSync(resolve(root, `scripts/${file}.mjs`), resolve(sdk, `scripts/${file}.mjs`)); }
    mkdirSync(resolve(sdk, 'host/apple'), { recursive: true });
    writeFileSync(resolve(sdk, 'host/apple/build.mjs'), 'console.error("ios: the signing team is unset\\n  4 | top-secret-source"); process.exit(1);');
    writeFileSync(resolve(sdk, 'scripts/verify.mjs'), 'console.log("verified");');
    const driver = resolve(parent, 'driver.mjs');
    writeFileSync(driver, `import { run } from ${JSON.stringify(resolve(root, 'scripts/app-cli.mjs'))};\nprocess.exit(await run({ dir: ${JSON.stringify(dir)}, name: 'field-log', exact2: ${JSON.stringify(sdk)}, args: process.argv.slice(2) }));\n`);
    const drive = (args, more = {}) => spawnSync(process.execPath, [driver, ...args], { encoding: 'utf8', env: { ...process.env, FORCE_COLOR: '0', NO_COLOR: '1', ...more }, stdio: ['ignore', 'pipe', 'pipe'] });
    const failed = drive(['ios']);
    assert.equal(failed.status, 1);
    assert.match(failed.stderr, /ios: the signing team is unset\n[^]*\nexact: a feedback note stays in \.exact\/[^\n]*\nexact: if this was unclear or slow, tell the exact2 team in one line: bun exact\.mjs feedback note "<what happened>"\n$/);
    assert.doesNotMatch(drive(['ios']).stderr, /feedback note/, 'once a day');
    assert.match(drive(['verify']).stderr, /^exact: if anything in this task was confusing or needed a workaround: bun exact\.mjs feedback note "<what happened>"\n$/);
    assert.doesNotMatch(drive(['verify']).stderr, /feedback note/, 'once a day too');
    rmSync(resolve(dir, '.exact/nudged'), { recursive: true });
    assert.doesNotMatch(drive(['ios'], { EXACT_FEEDBACK: '0' }).stderr, /feedback/, 'EXACT_FEEDBACK=0 says nothing');
    drive(['ios']);
    const noted = spawnSync(process.execPath, [resolve(root, 'scripts/feedback.mjs'), 'note', 'ios build needed a signing team; the docs never said'], { encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir }, stdio: ['ignore', 'pipe', 'pipe'] });
    assert.equal(noted.status, 0, noted.stderr);
    const diary = readFileSync(resolve(dir, '.exact/diary', readdirSync(resolve(dir, '.exact/diary'))[0]), 'utf8');
    assert.match(diary, /ios build needed a signing team[^]*- last command: ios, exit 1, [\d.]+s\n- machine: [^\n]*\n$/);
    assert.doesNotMatch(diary, /signing team is unset|top-secret-source/, 'no output of the command');
    // A drive refused before it starts (an unknown flag, a --backend with no origin) is logged and nudged too.
    rmSync(resolve(dir, '.exact/nudged'), { recursive: true });
    const refused = drive(['agent', 'web', '--no-such-flag']);
    assert.equal(refused.status, 2);
    assert.match(refused.stderr, /unknown flag --no-such-flag[^]*\nexact: if this was unclear or slow/);
    assert.match(readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8'), /"verb":"agent web","exit":2/);
    assert.equal(drive(['test', '--backend']).status, 2);
    assert.match(readFileSync(resolve(dir, '.exact/commands.jsonl'), 'utf8').trim().split('\n').at(-1), /"verb":"test web","exit":2/);
    // The child's stderr reaches the caller as it said it, and its exit code is kept.
    writeFileSync(resolve(sdk, 'host/apple/build.mjs'), 'for (let i = 0; i < 2000; i++) console.error(`line ${i}`); process.exit(3);');
    const said = drive(['ios']);
    assert.equal(said.status, 3);
    assert.equal(said.stderr.split('\n').filter(line => line.startsWith('line ')).join('\n'), Array.from({ length: 2000 }, (_, i) => `line ${i}`).join('\n'));
    // A child a signal ended says nothing, and its signal's code is kept.
    rmSync(resolve(dir, '.exact/nudged'), { recursive: true });
    writeFileSync(resolve(sdk, 'host/apple/build.mjs'), 'process.kill(process.pid, "SIGTERM");');
    const signalled = drive(['ios']);
    assert.equal(signalled.status, 143);
    assert.doesNotMatch(signalled.stderr, /feedback note/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});
