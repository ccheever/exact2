import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pending, redact, send, setStanding, standing } from './feedback.mjs';
import { summary } from './feedback-worker.js';

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

test('the endpoint tells Slack counts from the diary, not its content', () => {
  assert.deepEqual(summary('# a.md\n\n### Rough\n- one\n- two\n\n### Lean in\n- nice\n\n### Checkpoints\n- exact new: smooth\n- iOS build: rough (see Rough)\n\n# commands\n\nt abc web exit=0 1.0s\nt abc ios exit=1 9.0s\n'),
    { diaries: 1, rough: 2, lean: 1, smooth: 1, roughSteps: 1, commands: 2, failed: 1 });
});
