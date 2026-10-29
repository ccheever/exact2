import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { webCompiler } from './module.mjs';

// LLP 1146 D2 on the JS target: a store write outside a commit (here a mutation's, after its fetch) asks every store
// reader again, a failed one included, as the runner's commit after a module turn does. `renew` refreshes nothing, so
// only that rule can make `seen` read again.
test("a store write after a fetch asks a failed store reader again", async () => {
  await drive(`// Read now, fail after a turn, as a read that awaits its request does.
    const value = store.get('readers.value');
    return Promise.resolve().then(() => { if (value === null) throw new Error('nothing kept yet'); return value; });`, `test "the failed reader reads the renewed value"
  clock data
  expect text "seen" == ""
  tap "renew"
  clock data
  expect text "seen" == "renewed"
`);
}, 180_000);

// A read that throws at once, before any `await`, makes `seen` a store reader too: a write outside a commit asks it
// again (LLP 1146 §2.3). Its first answer, of round 0, read nothing.
test("a store read that throws at once is asked again when the store changes", async () => {
  await drive(`if (_args[0] === 0) return 'start';
    const value = store.get('readers.value');
    if (value === null) throw new Error('nothing kept yet');
    return value;`, `test "the reader that threw reads the renewed value"
  clock data
  expect text "seen" == "start"
  tap "next"
  clock data
  expect text "seen" == "start"
  tap "renew"
  clock data
  expect text "seen" == "renewed"
`);
}, 180_000);

// A read between two `await`s counts while the answer is still out: a write outside a commit while its second
// request is out asks it again after that reply (LLP 1146 §2.3). The server answers `renew`'s first request only once
// `seen`'s `/slow` has arrived, so the write comes after the read.
test("a store read between awaits is asked again when the store changes", async () => {
  expect(await drive(`return fetch(${'`'}\${origin}/ping${'`'}).then(() => {
      const first = store.get('readers.value') ?? 'none';
      return fetch(${'`'}\${origin}/slow${'`'}).then(() => first);
    });`, `test "the reader between awaits reads the renewed value"
  before data
  tap "renew"
  clock data
  expect text "seen" == "renewed"
`, undefined, { gate: true })).toBe(2);
}, 180_000);

// A read made only after an `await` makes `seen` a store reader too, as the runner counts a reply's turn (LLP 1146 §2.3).
test("a store read after an await is asked again when the store changes", async () => {
  await drive(`return fetch(${'`'}\${origin}/ping${'`'}).then(() => store.get('readers.value') ?? 'none');`, `test "the late reader reads the renewed value"
  clock data
  expect text "seen" == "none"
  tap "renew"
  clock data
  expect text "seen" == "renewed"
`);
}, 180_000);

// On the web, a write outside a commit asks a store reader in flight again once its reply lands, not at once (LLP 1146
// §2.3, §2.4). The test starts before the boot's data (`before data`), so `renew` runs while `seen`'s first `/slow` is
// out; the server holds it until `renew`, after its write, asks `/release`. `seen` says whether it was asked again
// while its first request was out.
test("a store reader in flight is asked again after its reply, not replaced", async () => {
  expect(await drive(slowReader('{}'), `test "the reader in flight reads the renewed value after its reply"
  before data
  tap "renew"
  clock data
  expect text "seen" == "renewed alone"
`)).toBe(2);
}, 180_000);

// A late write's commit that is refused (here `other`, asked again in it, throws the refusal the web's admission
// throws, admission.js) still leaves the reader in flight to be asked again after its reply.
test("a refused late-write commit keeps a reader in flight asked again", async () => {
  expect(await drive(slowReader('{}'), `test "the reader in flight is asked again after the refusal"
  before data
  tap "renew"
  clock data
  expect text "seen" == "renewed alone"
`, `if (store.get('readers.value') === 'renewed') throw Object.assign(new Error('refused'), { refuse: true });
    return 'none';`)).toBe(2);
}, 180_000);

// A reader whose request may never land (here a held fetch) is asked again at once, as natively (LLP 1146 §2.3).
test("a held store reader is asked again at once", async () => {
  expect(await drive(slowReader('{ exactHeld: true }'), `test "the held reader is asked again and reads the renewed value"
  clock data
  tap "renew"
  clock data
  expect text "seen" == "renewed overlapped"
`)).toBe(2);
}, 180_000);

// A `seen` body that reads the store, asks `/slow` with `options`, and says whether an earlier ask was still out.
function slowReader(options) {
  return `store.get('readers.value');
    if (out > 0) overlapped = true;
    out++;
    return fetch(${'`'}\${origin}/slow${'`'}, ${options}).then(() => ${'`'}\${store.get('readers.value') ?? 'none'} \${overlapped ? 'overlapped' : 'alone'}${'`'}).finally(() => out--);`;
}

// Build an app whose `seen` source is `seen`'s body (`origin` names the test's server; `out` and `overlapped` are the
// module's) and whose `other` source is `other`'s, with `renew` as above, and run `steps` on the web host. How many
// times `/slow` was asked: the server holds the first until `/release`, or answers it at once after one. With `gate`,
// it answers `/value` only once a `/slow` has arrived.
async function drive(seen, steps, other = "return 'none';", { gate = false } = {}) {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/store-readers-test-'));
  const cors = { headers: { 'Access-Control-Allow-Origin': '*' } }, held = [];
  let slow = 0, released = false;
  const gated = [];
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: req => {
    const path = new URL(req.url).pathname;
    if (path === '/slow') {
      for (const answer of gated.splice(0)) answer();
      if (++slow > 1 || released) return new Response('slow', cors);
      return new Promise(answer => held.push(() => answer(new Response('slow', cors))));
    }
    if (path === '/release') { released = true; for (const answer of held.splice(0)) answer(); }
    if (path === '/value' && gate && !slow) return new Promise(answer => gated.push(() => answer(new Response('renewed', cors))));
    return new Response('renewed', cors);
  } });
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Readers', app: { id: 'test.store-readers', name: 'Readers' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Done
  ok: bool

component Readers
  state round = 0
  resource seen = seen(round) as shape string else empty()
  resource other = other() as shape string
  mutation renewed as shape Done
  action renew
    send renewed = renew()
  action next
    round = round + 1
  view
    column
      text seen testId="seen"
      text other testId="other"
      button press=renew testId="renew"
        text "Renew"
      button press=next testId="next"
        text "Next"
`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.store-readers';
export const grants = ${JSON.stringify(`net.fetch ${server.url.origin}\nsecret.keep readers.value`)};
type Store = { get(name: string): string | null; set(name: string, value: string): void };
const origin = ${JSON.stringify(server.url.origin)};
let out = 0, overlapped = false;
export function answer(name: string, _args: unknown[], store: Store): any {
  if (name === 'seen') {
    ${seen}
  }
  if (name === 'other') {
    ${other}
  }
  if (name === 'renew') return (async () => {
    const reply = await fetch(${JSON.stringify(server.url.origin + '/value')});
    store.set('readers.value', await reply.text());
    await fetch(origin + '/release');
    return { ok: true };
  })();
  throw new Error('no source ' + name);
}
`);
  const file = resolve(dir, 'app.test.contract');
  writeFileSync(file, steps);
  let page;
  try {
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'readers', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    const r = await runTests({ host: 'web', file, app: dir, url: page.url.href });
    expect(r.results[0].failures).toEqual([]);
    return slow;
  } finally {
    page?.stop(true);
    server.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}
