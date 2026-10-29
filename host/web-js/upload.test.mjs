import { test, expect } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// LLP 1016.000 D10, end to end on the web JS target: an app downloads a file
// into its store (`exactBodyTo`), then uploads it as a resource's stream with
// `exactUploadProgress`, against a stand-in that takes the body slowly. The
// resource shows the bytes sent as they rise, is pending (and in
// `state.pending`) while they do, and `clock settle` waits for the reply,
// which ends it as `load`. A second upload sends a `Uint8Array` as its body,
// whose bytes the stand-in sums. A third, with no body and so no progress,
// fails: the stand-in answers 503, on which its mapper throws. An upload is
// asked once for its arguments: a refresh sends neither the finished one nor
// the failed one again.
const SIZE = 8 << 20;
test('an upload shows its bytes sent as they rise, and every clock waits for its reply', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/upload-test-'));
  // An upload listener makes XMLHttpRequest ask a preflight across origins.
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET, POST', 'access-control-allow-headers': '*' };
  const asked = { upload: 0, fail: 0 };
  const peer = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { status: 204, headers: cors });
    const path = new URL(req.url).pathname;
    if (path === '/big') return new Response(new Uint8Array(SIZE).fill(7), { headers: cors });
    if (path === '/fail') { asked.fail++; await req.arrayBuffer(); return new Response('busy', { status: 503, headers: cors }); }
    asked.upload++;
    let took = 0, sum = 0;
    // About 2 s for the large body, whatever the size of the chunks Bun hands over.
    for await (const chunk of req.body) { took += chunk.length; for (const b of chunk) sum += b; await Bun.sleep(Math.ceil(chunk.length / 4096)); }
    return new Response(JSON.stringify({ took, sum }), { status: 201, headers: { ...cors, 'content-type': 'application/json' } });
  } });
  const origin = `http://127.0.0.1:${peer.port}`;
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Upload Test', app: { id: 'test.upload', name: 'Upload Test' }, host: { web: {} }, rust: false }));
  writeFileSync(resolve(dir, 'app.contract'), `component UploadTest
  state go = false
  resource ready = prepare() as shape string else empty()
  resource sent = upload(go) as shape string else empty()
  resource small = bytes(go) as shape string else empty()
  resource broken = broken(go) as shape string else empty()
  action start
    go = true
  action retry
    refresh sent
    refresh small
    refresh broken
  view
    column
      text ready testId="ready"
      text sent testId="sent"
      text small testId="small"
      button press=start testId="start"
        text "start"
      button press=retry testId="retry"
        text "retry"
`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.upload';
export const grants = ${JSON.stringify(`net.fetch ${origin}\nfs.read app:/data\nfs.write app:/data`)};
export function answer(name: string, args: any[]): any {
  if (name === 'prepare') return fetch(${JSON.stringify(origin)} + '/big', { exactBodyTo: 'app:/data/big.bin' } as RequestInit).then((r: any) => 'ready ' + r.exactBodyTo.bytes);
  if (name === 'upload' && args[0]) return fetch(${JSON.stringify(origin)} + '/upload', {
    method: 'POST', exactBodyFrom: 'app:/data/big.bin', exactUploadProgress: true,
    exactStream: (e: any) => e.type === 'progress' ? 'progress ' + e.loaded + '/' + e.total + ' ' + e.lengthComputable : e.type === 'load' ? 'load ' + e.status + ' ' + e.data : e.type + ' ' + e.kind + ' ' + e.message,
  } as RequestInit);
  if (name === 'bytes' && args[0]) return fetch(${JSON.stringify(origin)} + '/upload', {
    method: 'POST', body: new Uint8Array(65536).fill(7), exactUploadProgress: true,
    exactStream: (e: any) => e.type === 'load' ? 'load ' + e.status + ' ' + e.data : e.type,
  } as RequestInit);
  if (name === 'broken' && args[0]) return fetch(${JSON.stringify(origin)} + '/fail', {
    method: 'POST', exactUploadProgress: true,
    exactStream: (e: any) => { if (e.type === 'load' && e.status >= 500) throw new Error('the upload failed'); return e.type; },
  } as RequestInit);
  return name === 'upload' || name === 'bytes' || name === 'broken' ? 'idle' : '';
}
`);
  let browser, server;
  try {
    const built = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'upload-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: resolve(dir, 'dist') }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([built.exited, new Response(built.stdout).text(), new Response(built.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    // A file the build did not write (`exact.json`, which only a bake has) is a 404.
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname, file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    // The app's files (its download, then its upload's body) are the drive's store.
    browser = await open({ host: 'web', url: server.url.href, webDist: resolve(dir, 'dist'), app: dir, storage: 'upload-test' });
    await browser.clock('data');
    expect((await browser.state()).resources.ready).toBe(`ready ${SIZE}`);
    await browser.tap('start');
    // Each state read is one moment: a value short of the whole is an upload in flight, and pending.
    const seen = [], deadline = performance.now() + 20000;
    for (;;) {
      const state = await browser.state(), sent = state.resources.sent;
      if (sent !== seen.at(-1)) seen.push(sent);
      const loaded = /^progress (\d+)\//.exec(sent)?.[1];
      if (loaded !== undefined && Number(loaded) < SIZE) {
        expect(state.pending).toContainEqual(expect.objectContaining({ name: 'sent' }));
        break;
      }
      if (sent.startsWith('load') || performance.now() > deadline) throw new Error('no progress before the end: ' + JSON.stringify(seen) + '\n' + (await browser.logs()).lines.join('\n'));
      await new Promise(r => setTimeout(r, 20));
    }
    // The upload is in flight until its reply, so `clock settle` waits for it.
    const settled = await browser.clock('settle');
    expect(settled.settled).not.toBe(false);
    const after = await browser.state();
    expect(after.resources.sent).toBe(`load 201 {"took":${SIZE},"sum":${SIZE * 7}}`);
    expect(after.resources.small).toBe(`load 201 {"took":65536,"sum":${65536 * 7}}`);
    expect(after.pending.filter(p => p.name === 'sent')).toEqual([]);
    expect(after.resources.broken).toBe('idle');
    expect(after.failed?.broken).toContain('the upload failed');
    expect(asked).toEqual({ upload: 2, fail: 1 });
    // Asked once for its arguments: a refresh sends none of them again, finished or failed.
    await browser.tap('retry');
    expect((await browser.clock('settle')).settled).not.toBe(false);
    expect(asked).toEqual({ upload: 2, fail: 1 });
    const retried = await browser.state();
    expect(retried.resources.sent).toBe(after.resources.sent);
    expect(retried.failed?.broken).toContain('the upload failed');
    const sent = seen.filter(s => s.startsWith('progress ')).map(s => Number(/^progress (\d+)\/(\d+) true$/.exec(s)[1]));
    expect(sent.every((n, i) => i === 0 || sent[i - 1] <= n)).toBe(true);
  } finally {
    await browser?.close(); server?.stop(true); peer.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 240000);

// What the JS target refuses of an upload (ts-fetch.js, ts-stream.js `uploadOf`), in Hermes's
// words and order (js/src/binary.js; js/tests/it/upload.rs has Hermes's): `fetch` itself rejects
// what it can tell alone, and the stream's opening ends the answer with the rest.
test('the JS target refuses an upload where Hermes does, in its words', async () => {
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-upload-'));
  writeFileSync(resolve(dir, 'source.js'), `export const appId = 'test.upload'; export const grants = '';
import { fetch } from './ts-fetch.js';
const url = 'https://example.test/upload', exactStream = e => e.type, post = { method: 'POST', exactStream };
const inits = [
  [url, { ...post, exactUploadProgress: 'yes' }],
  [url, { method: 'POST', exactUploadProgress: true }],
  [url, { exactStream, exactUploadProgress: true }],
  ['wss://example.test/upload', { ...post, exactUploadProgress: true }],
  [url, { exactStream, exactUploadProgress: true, exactLongPoll: { cursor: 'since', from: 'seq' } }],
  [url, { ...post, exactUploadProgress: true, redirect: 'manual' }],
  [url, { ...post, exactTimeout: 30000 }],
  [url, { ...post, exactUploadProgress: true, exactTimeout: 0 }],
  ['exact-native:', { ...post, exactUploadProgress: true }],
];
export function answer(name, args) { return fetch(...inits[args[0]]); }
`);
  writeFileSync(resolve(dir, 'ts-data.js'), readFileSync(resolve(root, 'host/web-js/ts-data.js'), 'utf8')
    .replace('__APP_TS__', pathToFileURL(resolve(dir, 'source.js')).href).replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', ''));
  writeFileSync(resolve(dir, 'rt.js'), 'export const clock={agent:false,now:0},journal=[],Resources=[],inflight={n:0};export const drive=()=>{},Origin={now:()=>"app"};export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n');
  writeFileSync(resolve(dir, 'rt-link.js'), "import * as rt from './rt.js'; export { rt };\n");
  writeFileSync(resolve(dir, 'names.js'), 'export const sourceTypes={};\n');
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(root, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'").replaceAll("'../web/address-space.js'", "'./address-space.js'"));
  writeFileSync(resolve(dir, 'admission-data.js'), "import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(null);\n");
  for (const name of ['grant-admission.js', 'address-space.js', 'faults.js', 'navigation.js', 'storage-environment.js', 'http-body.js']) cpSync(resolve(root, 'host/web', name), resolve(dir, name));
  for (const name of ['ts-fetch.js', 'ts-stream.js', 'ts-body-from.js', 'shape.js', 'uploads.js']) cpSync(resolve(root, 'host/web-js', name), resolve(dir, name));
  try {
    const ts = await import(pathToFileURL(resolve(dir, 'ts-data.js')).href), data = { q: [] };
    ts.install(data);
    const said = [];
    for (let i = 0; i < 9; i++) {
      const r = data.answer('upload', [i], new Map());
      said.push(r.stream ? (await r.stream(() => {}, new AbortController())).error : await r.promise.then(() => 'answered', e => e.name + ': ' + e.message));
    }
    expect(said).toEqual([
      'exactUploadProgress is true or false',
      "TypeError: exactUploadProgress reports an upload's progress to its stream: give exactStream too",
      'exactUploadProgress: a GET or HEAD request sends no body',
      'exactUploadProgress: a WebSocket sends no body',
      'exactUploadProgress: a long poll sends no body',
      'exactUploadProgress: an upload follows its redirects',
      'TypeError: exactTimeout: a stream has no timeout',
      'exactTimeout must be an integer number of milliseconds from 1 to 3600000',
      'exactUploadProgress: an upload is an http: or https: request',
    ]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
