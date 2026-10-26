import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { webCompiler } from './module.mjs';

// `clock until state <path> == <literal> within <ms>` (rules/DEFERRED.md, "Agent API", 2026-10-26) in an authored
// test on the JS target. The socket's first message is `hello`, which ends every settlement (LLP 1016.000 D5); the
// mapper answers it with `identify`, as Excord answers Discord's HELLO. The peer sends `ready` 1 s of real time after
// the test taps `arm` (`atStep`), so `ready` can come only inside the step that waits for it. A timer every 100 ms
// shows the clock stood still meanwhile.
test('an authored test waits for a socket state past its first message, the clock frozen, and a miss names the last value', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/until-state-test-'));
  const identified = new Set(), armed = [], sent = [];
  const peer = Bun.serve({ hostname: '127.0.0.1', port: 0,
    fetch(req, server) { return server.upgrade(req) ? undefined : new Response('websocket only'); },
    websocket: {
      open(ws) { ws.send('hello'); },
      message(ws, data) { if (data === 'identify') identified.add(ws); },
      close(ws) { identified.delete(ws); },
    },
  });
  const arm = () => {
    armed.push(performance.now());
    setTimeout(() => { for (const ws of identified) ws.send('ready'); sent.push(performance.now()); }, 1000);
  };
  const url = `ws://127.0.0.1:${peer.port}`;
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Until State', app: { id: 'test.until', name: 'Until State' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `component UntilState
  state n = 0
  state armed = false
  resource gateway = gateway() as shape string else empty()
  task tick when n < 100000 key=n
    after(100, bump)
  action bump
    n = n + 1
  action arm
    armed = true
  view
    column
      text gateway testId="gateway"
      text \`\${n}\` testId="n"
      button press=arm testId="arm"
        text "Arm"
`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.until';
export const grants = ${JSON.stringify(`net.websocket ${url}`)};
export function answer(name: string): any {
  if (name !== 'gateway') throw new Error('no source ' + name);
  return fetch(${JSON.stringify(url)}, { exactStream(event: any, socket?: any) {
    if (event.type !== 'message') return event.type;
    if (event.data === 'hello') socket.send('identify');
    return event.data;
  }});
}
`);
  const file = resolve(dir, 'app.test.contract');
  // The launch lands the stream's first answer, its \`open\`; \`hello\` may come after it.
  writeFileSync(file, `test "ready arrives on real time after hello"
  clock until state gateway == "hello" within 10000
  tap "arm"
  clock until state gateway == "ready" within 10000
  expect state gateway == "ready"
  expect state n == 0

test "a miss fails with the last value"
  clock until state gateway == "hello" within 10000
  tap "arm"
  clock until state gateway == "ready" within 10000
  clock until state gateway == "gone" within 300
`);
  let page;
  try {
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'until-state', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    const r = await runTests({ host: 'web', file, app: dir, url: page.url.href, atStep: (_, st) => { if (st.op === 'tap') arm(); } });
    const [waited, missed] = r.results;
    expect(waited.failures).toEqual([]);
    expect(missed.failures).toEqual([expect.stringMatching(/line 12: clock until state gateway == "gone" within 300: gateway was "ready" after \d+ ms of real time, the clock frozen at 0 ms$/)]);
    expect([armed.length, sent.length]).toEqual([2, 2]);
  } finally {
    page?.stop(true);
    peer.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);
