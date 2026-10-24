// @exact/language-model's conformance suite on Apple (LLP 1133.016 §10, §13
// stage 2): the fixture app (`test/fixture`) made with `exact new` outside
// the repo, depending on this package by `file:`, built for macOS or an iOS
// simulator and driven by `exact.mjs agent`.
//
// By default the Apple implementation is bound as in production (`--bind
// language-model`) and talks to Foundation Models on this machine. What the
// suite checks depends on what the machine has, which it prints first:
// with Apple Intelligence on, a create, a constrained prompt's typed value,
// a stream to `complete`, a tool's round trip through the app's handler and
// an abort; otherwise the availability's reason and a create that fails.
// Never a reply's wording.
//
// `--agent` drives the fixture under the agent instead: the Swift agent
// model and §10's drives 7 and 8 on the device's runtime (`agentDrives`).
//
// usage: bun packages/language-model/test/apple.mjs [--agent] [macos|ios] [--sim <udid>]
import { rmSync } from 'node:fs';
import { dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { agentDrives, checker, makeFixture } from './fixture.mjs';

const args = process.argv.slice(2), agent = args.includes('--agent');
const host = args.includes('ios') ? 'ios' : 'macos';
const sim = args.includes('--sim') ? args[args.indexOf('--sim') + 1] : null;
const { check, failed } = checker();
const { app } = makeFixture();
const flags = [...(sim ? ['--sim', sim] : []), '--size', '600x900'];

const drive = (ops, bind = false) => {
  const r = spawnSync('bun', ['exact.mjs', 'agent', host, ...flags, ...(bind ? ['--bind', 'language-model'] : []), ...ops], { cwd: app, encoding: 'utf8', maxBuffer: 64 << 20 });
  return { ok: r.status === 0, out: `${r.stdout}\n${r.stderr}` };
};
const shown = (out, id) => [...out.matchAll(new RegExp(`\\[${id}\\] "((?:[^"\\\\]|\\\\.)*)"`, 'g'))].map(m => m[1]).at(-1) ?? '';

if (agent) {
  for (const [name, ops, want] of agentDrives()) {
    const { ok, out } = drive(ops);
    const missing = want.filter(w => !out.includes(w));
    check(`${host} agent`, name, ok && !missing.length, `missing ${JSON.stringify(missing)}\n${out.split('\n').slice(-30).join('\n')}`);
  }
} else {
  const first = drive(['clock +500 real', 'clock data', 'tree availability', 'tree tooled'], true);
  const availability = shown(first.out, 'availability');
  console.log(`${host}: availability "${availability}", with tools "${shown(first.out, 'tooled')}"`);
  check(host, 'availability is one of the interface\'s', first.ok && /^(available -|unavailable (device-not-eligible|not-enabled|model-not-ready|os|unsupported-options)|downloadable -|downloading -) /.test(availability), first.out.slice(-2000));
  if (!availability.startsWith('available')) {
    const { ok, out } = drive(['tap create', 'clock data', 'tree created-error'], true);
    check(host, 'a create while unavailable fails', ok && /\S/.test(shown(out, 'created-error')), out.slice(-2000));
  } else {
    const wait = (n) => Array.from({ length: n }, () => ['clock +1000 real', 'clock data']).flat();
    const { ok, out } = drive(['tap ask', 'clock data', ...wait(20), 'tree asked', 'tap create', 'clock data', 'tree created',
      'type draft Use the findNotes tool to look up my notes about lifetimes, then tell me their titles.', 'tap stream', 'clock data', ...wait(30),
      'tree reply-state', 'tree reply', 'tree tool', 'logs'], true);
    // Data code's session has no tools: its constraint is a schema object, its value JSON text.
    const asked = shown(out, 'asked').replace(/\\"/g, '"');
    let title = null;
    try { title = JSON.parse(asked).title; } catch { /* not JSON */ }
    check(host, 'a constrained prompt answers a value of its schema', typeof title === 'string' && title !== '', asked);
    check(host, 'a create answers the session, its window and tools', ok && /^session-\d+ \d+ findNotes false$/.test(shown(out, 'created')), shown(out, 'created') + out.slice(-1500));
    const state = shown(out, 'reply-state'), tool = shown(out, 'tool'), calls = Number(state.split(' ')[1] ?? 0);
    check(host, 'a stream runs to complete', /^complete \d+$/.test(state) && shown(out, 'reply') !== '', `${state} "${shown(out, 'reply')}"`);
    // Whether the model calls the tool is the model's; a call it makes is the app's handler's to answer.
    console.log(`${host}: the model made ${calls} tool calls${calls ? `; the last: ${tool}` : ''}`);
    check(host, 'a tool the model calls runs the app\'s handler', !calls || tool.startsWith('findNotes done'), `${state} tool "${tool}"`);
    const stop = drive(['tap create', 'clock data', 'tap stream', 'clock data', 'clock +300 real', 'tap stop', 'clock data', 'tree reply-state'], true);
    check(host, 'abort ends the response aborted', /^(aborted|complete) /.test(shown(stop.out, 'reply-state')), shown(stop.out, 'reply-state') + stop.out.slice(-1500));
  }
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
