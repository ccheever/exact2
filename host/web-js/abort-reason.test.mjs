import { test, expect } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { webCompiler } from './module.mjs';

// An aborted fetch on the JS target rejects with its signal's reason, as Fetch and the native prelude do (js/tests/it/
// abort.rs): an `AbortError` for a bare `abort()`, the very value for `abort(reason)`, and a `TimeoutError` for an
// `AbortSignal.timeout`. An `exactTimeout` still fails as the `FetchError` of kind `Timeout` it always was. Each runs
// the real admission.js (`fetchWith`) against a server that holds every request, aborted once the request is there.
const ROOT = resolve(import.meta.dir, '../..');

test('an aborted fetch rejects with its signal\'s reason; an exactTimeout is still a Timeout', async () => {
  const { cmd, pre } = webCompiler();
  if (cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const arrived = [];
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: () => {
    let wake;
    const held = new Promise(resolve => { wake = resolve; });
    arrived.push(wake);
    return held.then(() => new Response('late'));
  } });
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-abort-reason-'));
  try {
    const scratch = resolve(dir, 'grants.txt');
    writeFileSync(scratch, `net.fetch ${server.url.origin}`);
    const run = spawnSync(cmd, [...pre, 'normalize-grants', scratch], { cwd: ROOT, encoding: 'utf8' });
    expect(run.status).toBe(0);
    writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replace(/'\.\.\/web\/([a-z-]+\.js)'/g, "'./$1'"));
    for (const name of ['grant-admission.js', 'faults.js', 'address-space.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
    const { createGrantSet, fetchWith, FetchError } = await import(resolve(dir, 'admission.js'));
    const set = createGrantSet(JSON.parse(run.stdout));
    // The rejection of a fetch made with `init`. With `abort`, it is called once the request has arrived (or the fetch
    // has already settled, which the assertions then catch), within a bound; a timeout's fetch is simply awaited.
    const rejection = async (init, abort) => {
      const before = arrived.length;
      let settled = false;
      const fetched = fetchWith(set, `${server.url.origin}/held`, init).then(() => null, e => e).finally(() => { settled = true; });
      if (abort) {
        const bound = performance.now() + 10_000;
        while (arrived.length === before && !settled) {
          if (performance.now() > bound) throw new Error('the request never reached the server');
          await Bun.sleep(5);
        }
        abort();
      }
      return await fetched;
    };
    const bare = new AbortController();
    const plain = await rejection({ signal: bare.signal }, () => bare.abort());
    expect(plain).toBe(bare.signal.reason);
    expect(plain.name).toBe('AbortError');
    const given = new AbortController(), reason = new Error('by the app');
    expect(await rejection({ signal: given.signal }, () => given.abort(reason))).toBe(reason);
    const timed = AbortSignal.timeout(30);
    const late = await rejection({ signal: timed });
    expect(late).toBe(timed.reason);
    expect(late.name).toBe('TimeoutError');
    const deadline = await rejection({ exactTimeout: 30 });
    expect(deadline).toBeInstanceOf(FetchError);
    expect([deadline.kind, deadline.message]).toEqual(['Timeout', 'the request timed out after 30 ms']);
    // With the app's own signal too, a deadline that ends it first is still the Timeout.
    const beside = new AbortController();
    const first = await rejection({ signal: beside.signal, exactTimeout: 30 });
    expect([first.name, first.kind]).toEqual(['FetchError', 'Timeout']);
  } finally {
    for (const wake of arrived) wake();
    server.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 60_000);
