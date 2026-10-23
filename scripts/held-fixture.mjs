// An app whose one resource asks a server that holds the request, as a long poll does (LLP 1016.000 D9),
// and its server, for the hosts' held-request tests (host/web-js/held.test.mjs, host/apple/held.test.mjs):
// `poll("held")` fetches `/changes` with `exactHeld: true`; `poll("plain")` the same unmarked. Once the
// held reply lands, the answer fetches `/detail` (unmarked): its work after the held reply, which a clock
// waits on again. A timer every 50 ms makes every jump cross timers.
import { mkdtempSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CONTRACT = `component HeldTest
  state n = 0
  state mode = "held"
  state k = 0
  resource item = poll(mode, k) as shape string else empty()
  task tick when n < 100000 key=n
    after(50, bump)
  action bump
    n = n + 1
  action unmark
    mode = "plain"
  action again
    k = k + 1
  view
    column
      text \`\${n}\` testId="n"
      text item testId="item"
      button press=unmark testId="unmark"
        text "unmark"
      button press=again testId="again"
        text "again"
`;

/** The server: `/changes` is held until `release(mode)` (or, unmarked, `plainAfter` ms); `/detail` answers
 * after `detailDelay` ms. `seen` lists each request's path and mode in order. `port`: a fixed one, for a native
 * build, whose grants name it. */
export function holdingServer({ detailDelay = 0, plainAfter = 3000, port = 0 } = {}) {
  const seen = [], waiting = new Map();
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*' };
  const server = Bun.serve({ hostname: '127.0.0.1', port, idleTimeout: 0, fetch(req) {
    const url = new URL(req.url);
    if (req.method === 'OPTIONS') return new Response(null, { headers: cors });
    if (url.pathname === '/changes') {
      const mode = url.searchParams.get('mode');
      seen.push(`changes ${mode}`);
      return new Promise(ok => {
        const answer = () => ok(new Response(`r${seen.length}`, { headers: cors }));
        waiting.set(mode, answer);
        if (mode === 'plain') setTimeout(answer, plainAfter);
      });
    }
    if (url.pathname === '/detail') {
      seen.push('detail');
      return new Promise(ok => setTimeout(() => ok(new Response('detail', { headers: cors })), detailDelay));
    }
    return new Response('no', { status: 404, headers: cors });
  } });
  return {
    origin: `http://127.0.0.1:${server.port}`, seen,
    release: mode => { const answer = waiting.get(mode); waiting.delete(mode); answer?.(); return !!answer; },
    stop: () => server.stop(true),
  };
}

/** The app in a fresh directory under `root` (target/), asking `origin`. */
export function heldApp(root, origin) {
  const dir = mkdtempSync(resolve(root, 'target/held-test-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Held Test', app: { id: 'test.held', name: 'Held Test' }, host: { web: {}, macos: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), CONTRACT);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.held';
export const grants = ${JSON.stringify(`net.fetch ${origin}`)};
export function answer(name: string, args: any): any {
  if (name !== 'poll') throw new Error('no source ' + name);
  const mode = String(args[0]);
  return fetch(${JSON.stringify(origin)} + '/changes?mode=' + mode, { exactHeld: mode === 'held' })
    .then(r => r.text())
    .then(text => fetch(${JSON.stringify(`${origin}/detail`)}).then(d => d.text()).then(detail => text + '|' + detail));
}
`);
  return dir;
}

/** The drive every host runs, on an open agent session `s` (scripts/agent.mjs `open`): returns what it saw,
 * for the test to check. */
export async function driveHeld(s, server) {
  const out = {};
  const timed = async (spec) => { const t0 = performance.now(); const r = await s.clock(spec); return { ms: performance.now() - t0, r }; };
  // Each look asks the host its state: the Linux host runs its requests when its agent is asked something.
  const until = async (pass, ms = 15000) => { for (const end = performance.now() + ms; !pass(); ) { if (performance.now() > end) throw new Error(`not seen: ${server.seen.join(', ')}`); await Bun.sleep(25); await s.state().catch(() => null); } };
  await until(() => server.seen.includes('changes held'));
  // 1. A jump across timers goes on at once, and names the held ask.
  out.jump = await timed(1000);
  // 2. `clock data` lands what is waited on, and names it too.
  out.data = await timed('data');
  // `clock settle` settles beside it, naming it.
  out.settle = await timed('settle');
  out.state = await s.state();
  // 3. Released, the answer's next fetch (unmarked, answered after detailDelay) is waited on again.
  server.release('held');
  await until(() => server.seen.includes('detail'));
  out.after = await timed(1400);
  out.item = (await s.state()).resources?.item;
  // 4. `clock held`: asked again, the held ask's reply (the server answers in 1 s) is waited for in real time with
  // the clock frozen, so no timer fires meanwhile; then `clock data` lands the work it started.
  const asks = () => server.seen.filter(x => x === 'changes held').length;
  await s.tap('again');
  await until(() => asks() >= 2);
  out.nBefore = (await s.state()).slots?.n;
  setTimeout(() => server.release('held'), 1000);
  out.held = await timed('held');
  out.nAfter = (await s.state()).slots?.n;
  out.heldItem = (await s.state()).resources?.item;
  // 5. Unanswered within its bound, `clock held` says which held target had no reply.
  await s.tap('again');
  await until(() => asks() >= 3);
  out.timeout = await timed('held 1s');
  server.release('held');
  // 6. The same ask unmarked is waited on before a due timer (the server answers it after plainAfter).
  await s.tap('unmark');
  await until(() => server.seen.includes('changes plain'));
  out.plain = await timed(1800);
  return out;
}

// `bun scripts/held-fixture.mjs native <dir> [port]`: the app as an `exact new` app at `dir` (outside the repo),
// asking 127.0.0.1:<port> (47711), to build with `(cd <dir> && bun exact.mjs mac)` and test with
// EXACT_HELD_APP=<dir> EXACT_HELD_PORT=<port> bun test host/apple/held.test.mjs (the wasm build: see
// host/web/held.test.mjs).
if (import.meta.main) {
  const [verb, dir, port = '47711'] = process.argv.slice(2);
  if (verb !== 'native' || !dir) { console.error('usage: bun scripts/held-fixture.mjs native <dir> [port]'); process.exit(2); }
  const { spawnSync } = await import('node:child_process');
  const { cpSync, rmSync } = await import('node:fs');
  const root = resolve(import.meta.dir, '..');
  const made = spawnSync(process.execPath, ['scripts/exact.mjs', 'new', resolve(dir), '--name', 'Held Test', '--bundle-id', 'test.held'], { cwd: root, stdio: 'inherit' });
  if (made.status) process.exit(made.status);
  const app = heldApp(root, `http://127.0.0.1:${port}`);
  for (const f of ['app.contract', 'app.ts']) cpSync(resolve(app, f), resolve(dir, f));
  rmSync(app, { recursive: true, force: true });
  rmSync(resolve(dir, 'app.test.contract'), { force: true });
  console.log(`${dir}: build it with (cd ${dir} && bun exact.mjs mac)`);
}
