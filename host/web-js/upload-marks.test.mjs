import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// LLP 1016.000 D10 on the JS target, as the runner keeps it: an upload is asked once for its arguments. The stand-in
// counts each upload it is sent, by `n`, and holds `n=2`'s until the test lets it answer.
//  - New arguments with the same request keep the upload going (`12` asks what `2` asked, LLP 1054.000.000 D3).
//  - Arguments come back to answer as their upload ended, after another upload's answer or a failed ask between.
//  - A store write the upload read, a refresh, and a refused commit (in which the arguments answered at once) send none
//    of them again; nor does a relaunch, which reads the marks the page kept.
//  - A plain request in flight after them (`5`) is asked again by a refresh, as natively; so is an upload the grants
//    refused before it went out (`6`), which never reached the network.
test('an upload is asked once for its arguments, through new arguments, a refused commit and a relaunch', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/upload-marks-test-'));
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET, POST', 'access-control-allow-headers': '*' };
  const asked = {};
  let answerTwo = () => {};
  const plain = [];
  const peer = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, async fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { status: 204, headers: cors });
    const n = new URL(req.url).searchParams.get('n');
    if (new URL(req.url).pathname === '/plain') {
      await new Promise(answer => plain.push(answer));
      return new Response('plain', { headers: cors });
    }
    asked[n] = (asked[n] ?? 0) + 1;
    await req.arrayBuffer();
    if (n === '2') await new Promise(answer => { answerTwo = answer; });
    return new Response(`took ${n}`, { status: 201, headers: cors });
  } });
  const origin = `http://127.0.0.1:${peer.port}`;
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Upload Marks', app: { id: 'test.upload-marks', name: 'Upload Marks' }, host: { web: {} }, rust: false }));
  const actions = { one: 'pick = 1', two: 'pick = 2', twelve: 'pick = 12', three: 'pick = 3', five: 'pick = 5', six: 'pick = 6', refused: 'pick = 0\n    boom = true', renew: 'send renewed = renew()', again: 'refresh up' };
  writeFileSync(resolve(dir, 'app.contract'), `shape Done
  ok: bool

component UploadMarks
  state pick = 1
  state boom = false
  resource up = up(pick) as shape string else empty()
  resource bad = bad(boom) as shape string else empty()
  mutation renewed as shape Done
${Object.entries(actions).map(([name, body]) => `  action ${name}\n    ${body}\n`).join('')}  view
    column
      text up testId="up"
      text bad testId="bad"
${Object.keys(actions).map(name => `      button press=${name} testId="${name}"\n        text "${name}"\n`).join('')}`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.upload-marks';
export const grants = ${JSON.stringify(`net.fetch ${origin}\nsecret.keep marks.token`)};
type Store = { get(name: string): string | null; set(name: string, value: string): void };
let fives = 0, sixes = 0;
export function answer(name: string, args: any[], store: Store): any {
  if (name === 'up') {
    const n = args[0];
    if (n === 0) return 'idle';
    if (n === 3) throw new Error('no third');
    // (each ask its own URL: a browser holds a GET for a URL another in flight asks, its cache's lock)
    if (n === 5) { const k = ++fives; return fetch(${JSON.stringify(origin)} + '/plain?k=' + k).then(r => r.text()).then(t => t + ' ' + k); }
    // Outside the grants: refused before it goes out.
    if (n === 6) { const k = ++sixes; return fetch('https://elsewhere.example/up', { method: 'POST', body: 'x', exactUploadProgress: true,
      exactStream: (e: any) => e.type + ' ' + k } as RequestInit); }
    store.get('marks.token');
    // (12 asks what 2 asks: the same request)
    return fetch(${JSON.stringify(origin)} + '/up?n=' + (n % 10), { method: 'POST', body: 'x', exactUploadProgress: true,
      exactStream: (e: any) => e.type === 'load' ? 'load ' + e.data : e.type } as RequestInit);
  }
  if (name === 'bad') {
    if (args[0]) throw Object.assign(new Error('refused'), { refuse: true });
    return 'fine';
  }
  if (name === 'renew') return (async () => { store.set('marks.token', 'new'); return { ok: true }; })();
  throw new Error('no source ' + name);
}
`);
  let browser, server;
  try {
    const built = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'upload-marks-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: resolve(dir, 'dist') }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([built.exited, new Response(built.stdout).text(), new Response(built.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname, file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    const launch = () => open({ host: 'web', url: server.url.href, webDist: resolve(dir, 'dist'), app: dir, storage: 'upload-marks-test' });
    browser = await launch();
    // After a tap (or none), settled: what `up` shows, whether it failed, and what the stand-in was sent.
    const step = async (tap, up, sent, failed = false) => {
      if (tap) await browser.tap(tap);
      expect((await browser.clock('settle')).settled).not.toBe(false);
      const state = await browser.state();
      expect({ tap, up: state.resources.up, failed: !!state.failed?.up, asked: { ...asked } }).toEqual({ tap, up, failed, asked: sent });
    };
    const until = async what => { for (const end = performance.now() + 10000; !what(); await Bun.sleep(20)) if (performance.now() > end) throw new Error('timed out: ' + JSON.stringify(asked)); };
    await step(null, 'load took 1', { 1: 1 });
    await browser.tap('two');
    await until(() => asked[2] === 1);
    // The same request for newer arguments: the upload going on is theirs, and nothing more is sent.
    await browser.tap('twelve');
    await Bun.sleep(300);
    expect(asked).toEqual({ 1: 1, 2: 1 });
    answerTwo();
    await step(null, 'load took 2', { 1: 1, 2: 1 });
    await step('one', 'load took 1', { 1: 1, 2: 1 });
    await step('two', 'load took 2', { 1: 1, 2: 1 });
    await step('three', 'load took 2', { 1: 1, 2: 1 }, true);
    await step('one', 'load took 1', { 1: 1, 2: 1 });
    await step('renew', 'load took 1', { 1: 1, 2: 1 });
    await step('again', 'load took 1', { 1: 1, 2: 1 });
    // A refused commit, in which `up` answered at once: its marks are put back with its value.
    await step('refused', 'load took 1', { 1: 1, 2: 1 });
    await step('again', 'load took 1', { 1: 1, 2: 1 });
    // A plain request in flight, then a refresh: asked again, so the second ask's answer shows.
    await browser.tap('five');
    await until(() => plain.length === 1);
    await browser.tap('again');
    await until(() => plain.length === 2);
    for (const answer of plain.splice(0)) answer();
    await step(null, 'plain 2', { 1: 1, 2: 1 });
    // Refused before it went out: `refresh` asks it again.
    await step('six', 'error 1', { 1: 1, 2: 1 });
    await step('again', 'error 2', { 1: 1, 2: 1 });
    // A relaunch reads the marks the page kept.
    await browser.close();
    browser = await launch();
    await step(null, 'load took 1', { 1: 1, 2: 1 });
    await step('twelve', 'load took 2', { 1: 1, 2: 1 });
  } finally {
    await browser?.close(); server?.stop(true); peer.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 240000);
