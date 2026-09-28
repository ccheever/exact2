#!/usr/bin/env bun
// Record the motion parity fixture from a real browser and check the engine
// against it. The cases and the check are Rust (host/web/src/parity.rs);
// this script serves parity.html + cases.json, renders it in headless Chrome,
// writes host/web/tests/fixtures/browser-motion.txt, and runs the check.
// Not a blocking check (it needs Chrome); the fixture it writes is held by
// `cargo test -p exact-web` (tests/parity.rs) with no browser at all.
// --presence drives the corpus's Contract fixture on web, macOS and Linux.
// --hosts selects hosts; --record-presence <host> deliberately replaces the
// shared recording. It is an observation, not a ruling that that host is right.
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, mkdtempSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const here = resolve(new URL('.', import.meta.url).pathname);
const root = resolve(here, '../..');
const run = (args) => spawnSync('cargo', ['run', '-q', '--release', '-p', 'exact-web', '--bin', 'parity', '--', ...args], { cwd: root, encoding: 'utf8' });

// LLP 1063: these are recordings of one script, not a browser oracle. Boxes
// use CSS pixels/points; 0.1 tolerates native layout rounding, opacity 0.005
// tolerates serialization. Missing ghosts and non-finite numbers always fail.
export function comparePresence(reference, actual) {
  const errors = [];
  const expected = reference.samples ?? [], got = actual.samples ?? [];
  if (!expected.length || got.length !== expected.length) errors.push(`sample count ${got.length}, expected ${expected.length || 'a nonempty timeline'}`);
  for (const id of ['card', 'sibling']) if (!expected[0]?.nodes?.[id]) errors.push(`reference: ${id} has no initial observation`);
  if (JSON.stringify(reference.steps) !== JSON.stringify(actual.steps)) errors.push('the recorded operations differ');
  for (const [i, a] of expected.entries()) {
    const b = got[i];
    if (!b || a.name !== b.name || a.clock !== b.clock) { errors.push(`${a.name}: missing sample or clock differs`); continue; }
    for (const id of ['card', 'sibling']) {
      const x = a.nodes?.[id], y = b.nodes?.[id], at = `${a.name} ${id}`;
      if (x === undefined || y === undefined) { errors.push(`${at}: no observation`); continue; }
      if (x === null || y === null) { if (x !== y) errors.push(`${at}: ${y === null ? 'absent' : 'present'}, expected ${x === null ? 'absent' : 'present'}`); continue; }
      if (x.exiting !== y.exiting) errors.push(`${at}: exiting ${y.exiting}, expected ${x.exiting}`);
      for (const k of ['x', 'y', 'w', 'h', 'opacity']) {
        if (![x[k], y[k]].every(Number.isFinite) || Math.abs(x[k] - y[k]) > (k === 'opacity' ? .005 : .1)) errors.push(`${at}: ${k} ${y[k]}, expected ${x[k]}`);
      }
    }
  }
  return errors;
}

async function presenceParity() {
  const { open } = await import('../../scripts/agent.mjs');
  const args = process.argv.slice(2), option = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
  const hosts = (option('--hosts') ?? 'web,macos,linux').split(',');
  if (hosts.some(host => !['web', 'macos', 'linux', 'ios'].includes(host))) throw new Error('--hosts takes web,macos,linux,ios');
  const record = option('--record-presence');
  if (record && !hosts.includes(record)) throw new Error('--record-presence must name a host in --hosts');
  const dir = resolve(root, 'target/presence-parity');
  mkdirSync(dir, { recursive: true });
  const plan = resolve(dir, 'presence.plan'), c = run(['presence', plan]);
  if (c.status !== 0) throw new Error(c.stderr);
  const steps = JSON.parse(c.stdout), records = {}, failures = [];
  for (const host of hosts) {
    let s;
    try {
      s = await open({ host, plan, size: [420, 600] });
      const tree = await s.tree(), ids = Object.fromEntries(['card', 'sibling'].map(name => [name, tree.nodes.find(n => n.props.testId === name)?.id]));
      if (Object.values(ids).some(id => id == null)) throw new Error('the presence fixture did not mount');
      const samples = [], raw = [];
      for (const step of steps) {
        if (step.advance != null) await s.clock(`+${step.advance}`);
        if (step.tap) await s.tap(step.tap);
        if (step.resize) {
          const resized = await s.op({ op: 'tap', resize: step.resize });
          if (JSON.stringify(resized.resized) !== JSON.stringify(step.resize)) throw new Error(`window did not resize: ${JSON.stringify(resized)}`);
        }
        const layout = await s.layout(), state = await s.state();
        if (!Array.isArray(state.presence)) throw new Error('state.presence is unavailable; rebuild the host');
        // AppKit's titlebar/toolbar consume viewport height. The window's
        // requested size was verified above; compare boxes relative to root.
        if (step.resize && Math.abs(layout.viewport.w - step.resize[0]) > 1) throw new Error(`viewport did not resize: ${JSON.stringify(layout.viewport)}`);
        const origin = layout.nodes.find(n => n.testId === 'root');
        if (!origin) throw new Error('the fixture root has no viewport box');
        const nodes = Object.fromEntries(Object.entries(ids).map(([name, id]) => {
          const matches = state.presence.filter(n => n.id === id);
          if (matches.length > 1) throw new Error(`${name}: duplicate presence observation`);
          const n = matches[0];
          return [name, n ? { x: n.x - origin.x, y: n.y - origin.y, w: n.w, h: n.h, opacity: n.opacity, exiting: n.exiting } : null];
        }));
        if (!samples.length && Object.values(nodes).some(n => !n)) throw new Error('the initial moving nodes have no presence observation');
        samples.push({ name: step.name, clock: state.clock, nodes });
        raw.push({ name: step.name, layout, state });
      }
      records[host] = { host, steps, samples };
      writeFileSync(resolve(dir, `${host}.json`), JSON.stringify({ ...records[host], raw, logs: await s.logs() }, null, 2) + '\n');
      console.log(`presence ${host}: recorded ${samples.length} samples`);
    } catch (error) { failures.push(`${host}: ${error.message}`); }
    finally { await s?.close(); }
  }
  const fixture = resolve(here, 'tests/fixtures/presence-motion.json');
  if (record) {
    if (!records[record]) throw new Error(`no complete ${record} recording; reference left unchanged\n${failures.join('\n')}`);
    writeFileSync(fixture, JSON.stringify(records[record], null, 2) + '\n');
    console.log(`presence: recorded ${record} as the shared timeline`);
  }
  const reference = JSON.parse(readFileSync(fixture, 'utf8'));
  for (const [host, recording] of Object.entries(records)) {
    const errors = comparePresence(reference, recording);
    failures.push(...errors.map(error => `${host}: ${error}`));
    console.log(`presence ${host}: ${errors.length ? `${errors.length} disagreements` : `all ${recording.samples.length} samples match`} (box 0.1 pt, opacity 0.005)`);
  }
  writeFileSync(resolve(dir, 'comparison.json'), JSON.stringify({ reference: reference.host, failures }, null, 2) + '\n');
  for (const error of failures) console.error(`  ${error}`);
  console.log(`presence parity: ${failures.length ? `${failures.length} failure(s)` : 'ok'}; recordings in target/presence-parity`);
  process.exitCode = failures.length ? 1 : 0;
}

async function browserParity() {
  const cases = run(['cases']);
  if (cases.status !== 0) { console.error(cases.stderr); process.exit(1); }
  const html = readFileSync(resolve(here, 'parity.html'));
  const server = createServer((req, res) => {
    if (req.url.startsWith('/cases.json')) { res.writeHead(200, { 'content-type': 'application/json' }); res.end(cases.stdout); return; }
    res.writeHead(200, { 'content-type': 'text/html' }); res.end(html);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  // A page target over the DevTools pipe, not `--dump-dom`: a drag timeline's
  // oracle is a scroll timeline, which moves only when the page renders
  // frames, and a dumped page renders none.
  const { Cdp } = await import('../../scripts/agent.mjs');
  const render = async () => {
    const profile = mkdtempSync(resolve(tmpdir(), 'exact-parity-'));
    const child = spawn(chrome, ['--headless=new', '--disable-gpu', `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run', '--no-default-browser-check', '--remote-debugging-pipe', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    try {
      const cdp = new Cdp(child.stdio[3], child.stdio[4]);
      child.on('exit', () => cdp.fail('Chrome closed'));
      const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
      const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
      await cdp.send('Page.navigate', { url: `http://127.0.0.1:${port}/` }, sessionId);
      const done = await cdp.send('Runtime.evaluate', {
        expression: `new Promise((ok) => { const t = setInterval(() => { const o = document.getElementById('out'); if (o?.dataset.done) { clearInterval(t); ok(o.textContent); } }, 50); })`,
        awaitPromise: true, returnByValue: true,
      }, sessionId, 60000);
      return done.result?.value ?? '';
    } catch (error) {
      return `error ${error.message}`;
    } finally {
      if (child.exitCode === null) { const exit = new Promise((r) => child.once('exit', r)); child.kill(); await exit; }
      rmSync(profile, { recursive: true, force: true });
    }
  };
  let text = await render();
  if (!text || /^error /m.test(text)) text = await render();
  server.close();
  if (!text || /^error /m.test(text)) { console.error('the page did not finish:\n' + text.slice(0, 500)); process.exit(1); }
  const lines = text.trim().split('\n');
  const browser = lines.find((l) => l.startsWith('browser '))?.slice(8) ?? 'unknown browser';
  const header = run(['header', `${new Date().toISOString().slice(0, 10)}, ${browser}`]).stdout;
  const fixture = resolve(here, 'tests/fixtures/browser-motion.txt');
  writeFileSync(fixture, header + lines.filter((l) => l.startsWith('sample ')).join('\n') + '\n');
  console.log(`recorded ${lines.filter((l) => l.startsWith('sample ')).length} samples from ${browser} → host/web/tests/fixtures/browser-motion.txt`);
  const check = run(['check', fixture]);
  process.stdout.write(check.stdout);
  process.exit(check.status ?? 1);

}

if (import.meta.main) {
  if (process.argv.includes("--presence")) await presenceParity();
  else await browserParity();
}
