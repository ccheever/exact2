import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// LLP 1016.000 D10, end to end on the web JS target: an app downloads a file
// into its store (`exactBodyTo`), then uploads it as a resource's stream with
// `exactUploadProgress`, against a stand-in that takes the body slowly. The
// resource shows the bytes sent as they rise, is pending (and in
// `state.pending`) while they do, and `clock settle` waits for the reply,
// which ends it as `load`. A second upload sends a `Uint8Array` as its body,
// whose bytes the stand-in sums.
const SIZE = 8 << 20;
test('an upload shows its bytes sent as they rise, and every clock waits for its reply', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/upload-test-'));
  // An upload listener makes XMLHttpRequest ask a preflight across origins.
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET, POST', 'access-control-allow-headers': '*' };
  const peer = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { status: 204, headers: cors });
    if (new URL(req.url).pathname === '/big') return new Response(new Uint8Array(SIZE).fill(7), { headers: cors });
    let took = 0, sum = 0;
    for await (const chunk of req.body) { took += chunk.length; for (const b of chunk) sum += b; await Bun.sleep(4); }
    return new Response(JSON.stringify({ took, sum }), { status: 201, headers: { ...cors, 'content-type': 'application/json' } });
  } });
  const origin = `http://127.0.0.1:${peer.port}`;
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Upload Test', app: { id: 'test.upload', name: 'Upload Test' }, host: { web: {} }, rust: false }));
  writeFileSync(resolve(dir, 'app.contract'), `component UploadTest
  state go = false
  resource ready = prepare() as shape string else empty()
  resource sent = upload(go) as shape string else empty()
  resource small = bytes(go) as shape string else empty()
  action start
    go = true
  view
    column
      text ready testId="ready"
      text sent testId="sent"
      text small testId="small"
      button press=start testId="start"
        text "start"
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
  return name === 'upload' || name === 'bytes' ? 'idle' : '';
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
    const sent = seen.filter(s => s.startsWith('progress ')).map(s => Number(/^progress (\d+)\/(\d+) true$/.exec(s)[1]));
    expect(sent.every((n, i) => i === 0 || sent[i - 1] <= n)).toBe(true);
  } finally {
    await browser?.close(); server?.stop(true); peer.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 240000);
