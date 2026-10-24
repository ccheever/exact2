// LLP 1133.016 §10's drives 1–6 of Agents under the agent model, on any
// host the agent drives: each drive's operations, then what its output must
// show. Drives 7 and 8 are the fixture's (`web.mjs --agent`) and the
// runner's tests.
//
// usage: bun packages/language-model/test/drives.mjs [web|macos|ios] [--sim <udid>] [1 2 …]
//   web builds Agents first; macos and ios drive the build `host/apple/build.mjs` made.
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { checker, root } from './fixture.mjs';

const args = process.argv.slice(2);
const host = ['web', 'macos', 'ios'].includes(args[0]) ? args.shift() : 'web';
const sim = args[0] === '--sim' ? (args.shift(), args.shift()) : null;
const only = args.map(Number);

const LONG = 'Tell me about ownership, borrowing and lifetimes in Rust, with an example of each and how they relate to one another in practice, and what the borrow checker does when two mutable borrows overlap in one function body, and why.';
const avail = v => `prefer extension language-model.languageModelAvailability=${JSON.stringify(v)}`;
const script = v => `prefer extension language-model.responses=${JSON.stringify(v)}`;
// A new conversation on the device, its composer ready.
const open = ['tap new', 'clock +1000', 'clock data', 'tap model-device', 'tap new-start', 'clock data', 'clock +1000'];
const send = text => [`type composer ${text}`, 'tap send', 'clock data'];

const drives = {
  1: {
    name: 'availability: not enabled, downloadable, downloading, available',
    ops: [avail({ availability: 'unavailable', reason: 'not-enabled' }), 'tap new', 'clock +1000', 'clock data', 'tree model-device',
      avail({ availability: 'downloadable' }), 'clock data', 'tap model-device', 'clock data', 'tap download', 'clock settle', 'tree model-device',
      avail({ availability: 'downloading', progress: 0.5 }), 'clock settle', 'tree model-device',
      avail({ availability: 'available' }), 'clock data', 'clock +1000', 'clock data', 'tree composer-caption'],
    want: ['Turn on Apple Intelligence in Settings to use this model.', 'Downloading… 0%', 'Downloading… 50%', '[composer-caption] "On this device"'],
  },
  2: {
    name: 'a reply streams on the session clock, and a reload restores the conversation',
    ops: [script([{ text: 'Lifetimes say how long a borrow may live.' }, { value: { title: 'Rust lifetimes' } }]), ...open, ...send('What are lifetimes?'),
      'clock +130', 'tree msg-m3', 'clock +1000', 'clock data', 'tree msg-m3', 'tree title-c1',
      ...(host === 'web' ? ['reload', 'clock data', 'clock +1000', 'clock data', 'tree msg-m3'] : [])],
    want: ['"Lifetimes say how long "', '"Lifetimes say how long a borrow may live."', '[title-c1] "Rust lifetimes"'],
  },
  3: {
    name: 'stop keeps the text so far, and the response reads aborted',
    ops: [script([{ text: 'A held reply that waits for stop', hold: true }]), ...open, ...send('Hold on.'), 'clock +2000', 'tap stop', 'clock data', 'clock +200', 'clock data', 'tree msg-m3', 'state extensions'],
    want: ['"A held reply that waits for "', '"state":"aborted"'],
  },
  4: {
    name: 'the tool finds an earlier conversation',
    ops: [script([{ text: 'Lifetimes say how long a borrow may live.' }, { value: { title: 'Rust lifetimes' } },
      { toolCalls: [{ name: 'findConversations', arguments: { query: 'lifetimes' } }], text: 'Last week you asked Claude Opus 5.5 about lifetimes.' }]),
    ...open, ...send('What are lifetimes?'), 'clock +2000', 'clock data', ...send('What did I ask about lifetimes?'), 'clock +2000', 'clock data', 'tree turn-m5', 'logs'],
    want: ['Searched your conversations for “lifetimes”', 'Last week you asked Claude Opus 5.5 about lifetimes.', 'findConversations for response-', 'tool:model:response-'],
  },
  5: {
    name: 'a 256-token window evicts, and a reload’s create trims the history',
    ops: ['prefer extension language-model.contextWindow=256', 'prefer extension language-model.rate=0', ...open,
      ...send(LONG), 'clock +300', 'clock data', ...send(`${LONG} Again.`), 'clock +300', 'clock data', ...send(`${LONG} Once more.`), 'clock +300', 'clock data',
      'tree turn-m7', 'state extensions',
      ...(host === 'web' ? ['reload', 'prefer extension language-model.contextWindow=256', 'prefer extension language-model.rate=0', 'clock data', 'clock +1000', 'clock data', ...send('And one more.'), 'clock +300', 'clock data', 'tree turn-m9', 'logs'] : [])],
    want: ['[forgot-m7] "Earlier messages no longer fit in the on-device model’s memory"', '"overflowed":true', ...(host === 'web' ? ['[forgot-m9]', 'earlier turns evicted'] : [])],
  },
  6: {
    name: 'a title by structured output',
    ops: [script([{ text: 'Sure.' }, { value: { title: 'Rust lifetimes' } }]), ...open, ...send('Name this.'), 'clock +1000', 'clock data', 'tree title-c1'],
    want: ['[title-c1] "Rust lifetimes"'],
  },
};

if (host === 'web') {
  const b = spawnSync('bun', ['host/web/build.mjs', 'agents-web'], { cwd: root, encoding: 'utf8' });
  if (b.status !== 0) { console.log(b.stdout, b.stderr); process.exit(1); }
}
const { check, failed } = checker();
for (const [n, d] of Object.entries(drives)) {
  if (only.length && !only.includes(Number(n))) continue;
  const flags = ['--app', 'agents', '--size', '1100x720', ...(host === 'web' ? ['--storage', `lm-drive-${n}-${process.pid}`] : []), ...(sim ? ['--sim', sim] : [])];
  const r = spawnSync('bun', [resolve(root, 'scripts/agent.mjs'), host, ...flags, ...d.ops], { cwd: root, encoding: 'utf8', maxBuffer: 64 << 20 });
  const out = `${r.stdout}\n${r.stderr}`;
  const missing = d.want.filter(w => !out.includes(w));
  check(host, `drive ${n}: ${d.name}`, r.status === 0 && !missing.length, `${r.status !== 0 ? `exit ${r.status}; ` : ''}missing ${JSON.stringify(missing)}\n${out.split('\n').slice(-40).join('\n')}`);
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
