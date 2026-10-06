import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pending, redact, send, setStanding, standing, status } from './feedback.mjs';
import { needs, summary } from './feedback-worker.js';

function app() {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-feedback-'));
  const dir = resolve(parent, 'field-log');
  mkdirSync(resolve(dir, '.exact/diary'), { recursive: true });
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Field Log', id: 'com.acme.fieldlog', app: { id: 'com.acme.fieldlog' } }));
  process.env.EXACT_CONFIG_DIR = resolve(parent, 'config');
  return { parent, dir };
}

test('the app, its paths and anything shaped like a credential are replaced before sending', () => {
  const { parent, dir } = app();
  try {
    const text = redact(`Field Log (com.acme.fieldlog) failed in ${realpathSync(dir)}/web and ${homedir()}/x; field-log; field-logger; `
      + 'token: abcdef123456 ghp_abcdefghijklmnopqrstuvwxyz AKIAABCDEFGHIJKLMNOP https://hooks.slack.com/services/T/B/x', dir);
    assert.equal(text, '<app> (<app>) failed in <app>/web and ~/x; <app>; field-logger; token: [redacted] [redacted] [redacted] [redacted]');
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('send posts what is unsent once, marks it sent, and never sends for a project set to never', async () => {
  const { parent, dir } = app();
  try {
    writeFileSync(resolve(dir, '.exact/diary/2026-10-04-a.md'), '### Rough\n- a slow build\n');
    writeFileSync(resolve(dir, '.exact/commands.jsonl'), `${JSON.stringify({ at: 't', exact2: 'abc', verb: 'web', exit: 0, ms: 1200 })}\n`);
    const posts = [];
    const fetch = async (url, init) => (posts.push([url, JSON.parse(init.body)]), { ok: true, json: async () => ({ id: 'receipt-1' }) });
    setStanding(dir, 'never');
    assert.match(await send(dir, { yes: true, fetch }), /never/);
    assert.equal(posts.length, 0);
    setStanding(dir, 'ask');
    assert.equal(standing(dir), 'ask');
    assert.match(await send(dir, { yes: true, fetch }), /Receipt: receipt-1/);
    assert.equal(posts.length, 1);
    assert.equal(posts[0][1].exact2, 'abc');
    assert.match(posts[0][1].text, /a slow build[^]*abc web exit=0 1\.2s/);
    assert.ok(existsSync(resolve(dir, '.exact/diary/sent/2026-10-04-a.md')));
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, '.exact/sent.json'), 'utf8')).receipts, ['receipt-1']);
    assert.equal(pending(dir).text, '');
    assert.equal(await send(dir, { yes: true, fetch }), 'Nothing unsent.');
    // A failed post marks nothing sent.
    writeFileSync(resolve(dir, '.exact/diary/2026-10-05-b.md'), 'b');
    await assert.rejects(send(dir, { yes: true, fetch: async () => ({ ok: false, status: 500 }) }), /500; nothing was marked sent/);
    assert.ok(existsSync(resolve(dir, '.exact/diary/2026-10-05-b.md')));
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('local keeps the diary and never sends; a * entry answers for projects without their own', async () => {
  const { parent, dir } = app();
  try {
    writeFileSync(resolve(dir, '.exact/diary/2026-10-04-a.md'), '### Rough\n- a slow build\n');
    mkdirSync(process.env.EXACT_CONFIG_DIR, { recursive: true });
    writeFileSync(resolve(process.env.EXACT_CONFIG_DIR, 'feedback.json'), JSON.stringify({ '*': 'local' }));
    assert.equal(standing(dir), 'local');
    const posts = [];
    const fetch = async (url, init) => (posts.push(url), { ok: true, json: async () => ({ id: 'r' }) });
    assert.match(await send(dir, { yes: true, fetch }), /never sends/);
    assert.equal(posts.length, 0);
    assert.ok(existsSync(resolve(dir, '.exact/diary/2026-10-04-a.md')));
    // The project's own answer wins over *, and `ask` undoes local even under *.
    setStanding(dir, 'always');
    assert.equal(standing(dir), 'always');
    setStanding(dir, 'ask');
    assert.equal(standing(dir), 'ask');
    // A * entry never consents to sending.
    writeFileSync(resolve(process.env.EXACT_CONFIG_DIR, 'feedback.json'), JSON.stringify({ '*': 'always' }));
    assert.equal(standing(dir), 'ask');
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('status prints the detailed diary only when EXACT_DIARY=detailed, and never under never', () => {
  const { parent, dir } = app();
  try {
    // Each answer says what it asks (the chess diary: `ask` read as undefined).
    assert.equal(status(dir, {}), 'ask (keep the diary; ask once before sending): 0 unsent diaries, 0 unsent logged commands');
    assert.match(status(dir, { EXACT_DIARY: 'detailed' }), /^ask \(keep the diary; ask once before sending\): 0 unsent[^]*detailed diary[^]*date '\+%F %T'[^]*self-assessment/);
    // docs/diary.md reads `never` in this output as the opt-out; the extra instructions must not say it.
    assert.doesNotMatch(status(dir, { EXACT_DIARY: 'detailed' }), /never/);
    setStanding(dir, 'never');
    assert.equal(status(dir, { EXACT_DIARY: 'detailed' }), 'never (keep no diary; never ask): 0 unsent diaries, 0 unsent logged commands');
    setStanding(dir, 'local');
    assert.doesNotMatch(status(dir, {}), /never/);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

test('the endpoint tells Slack counts from the diary, not its content', () => {
  assert.deepEqual(summary('# a.md\n\n### Rough\n- one\n- two\n\n### Lean in\n- nice\n\n### Checkpoints\n- exact new: smooth\n- iOS build: rough (see Rough)\n\n# commands\n\nt abc web exit=0 1.0s\nt abc ios exit=1 9.0s\n'),
    { diaries: 1, rough: 2, lean: 1, smooth: 1, roughSteps: 1, commands: 2, failed: 1, needs: [] });
});

test('Needed lines are counted per capability across diaries; anything else in the section is not', () => {
  const a = '# a.md\n\n### Needed\n- Camera: by hand (~40 min; a Swift bridge)\n- notifications: missing (skipped)\n- haptics: provided\n- scan receipts with the camera\n\n### Checkpoints\n- camera: by hand\n';
  const b = '# b.md\n\n### Needed\n- camera: by hand\n- share sheet: provided, rough (see Rough)\n';
  assert.deepEqual(summary(a).needs, [['camera', 'by hand'], ['notifications', 'missing'], ['haptics', 'provided']]);
  assert.deepEqual(needs([a, b, '# c.md\n\n### Needed\nnone\n']), [
    { name: 'camera', provided: 0, 'by hand': 2, missing: 0 },
    { name: 'notifications', provided: 0, 'by hand': 0, missing: 1 },
    { name: 'haptics', provided: 1, 'by hand': 0, missing: 0 },
    { name: 'share sheet', provided: 1, 'by hand': 0, missing: 0 },
  ]);
});
