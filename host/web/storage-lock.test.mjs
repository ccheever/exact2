// Actual Web Locks: wait for a competing owner, cancel bounded waits, and
// release ancestor locks after a partial acquisition fails.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('storage locks queue and time out without retaining acquired ancestors', async () => {
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path.endsWith('.js')) return new Response(readFileSync(resolve(import.meta.dirname, path.slice(1))), {headers: {'content-type': 'text/javascript'}});
    return new Response('<!doctype html>');
  }});
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const result = await page.evaluate(async () => {
      const {lockPaths} = await import('/storage-fs.js');
      const release = await lockPaths('test', ['app:/data/a.sqlite']);
      const waiting = lockPaths('test', ['app:/data/a.sqlite']);
      // query() observes the request queued; no timing guess decides it.
      while (!(await navigator.locks.query()).pending.length) await new Promise(resolve => setTimeout(resolve, 0));
      const queued = (await navigator.locks.query()).pending.length;
      await release();
      const next = await waiting;
      const unrelated = await lockPaths('test', ['app:/data/b.sqlite']);
      await unrelated();
      let refusal;
      try { await lockPaths('test', ['app:/data/a.sqlite'], 30); }
      catch (error) { refusal = {code: error.code, message: error.message}; }
      await next();
      // An exclusive root proves the aborted wait released its shared root.
      const root = await lockPaths('test', ['app:/data'], 1000);
      await root();
      return {queued, refusal, locks: await navigator.locks.query()};
    });
    expect(result.queued).toBeGreaterThan(0);
    expect(result.refusal).toMatchObject({code: 'EBUSY'});
    expect(result.refusal.message).toContain('after 30 ms');
    expect(result.locks).toEqual({held: [], pending: []});
  } finally { await browser.close(); server.stop(true); }
}, 60_000);

// rt1's review of the worker's timers (passed to lockPaths from module-worker.js's closure): the only callback they
// run is the lock's own abort, one per wait however many paths it takes, cleared when the wait ends, and its bound
// is the lock's own, never longer than asked.
test('a lock asks the timers it is given for its own bound alone, one per wait, cleared when it ends', async () => {
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path.endsWith('.js')) return new Response(readFileSync(resolve(import.meta.dirname, path.slice(1))), {headers: {'content-type': 'text/javascript'}});
    return new Response('<!doctype html>');
  }});
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const result = await page.evaluate(async () => {
      const {lockPaths} = await import('/storage-fs.js');
      const sets = [], cleared = [];
      const timers = {set(f, ms) { sets.push(ms); return setTimeout(f, ms); }, clear(id) { cleared.push(id); clearTimeout(id); }};
      const many = Array.from({length: 40}, (_, i) => `app:/data/deep/${i}/file`);
      const release = await lockPaths('timers', many, 5000, timers);
      await release();
      const held = await lockPaths('timers', ['app:/data/x'], 5000, timers);
      let refusal;
      try { await lockPaths('timers', ['app:/data/x'], 40, timers); } catch (error) { refusal = error.code; }
      await held();
      return {sets, cleared: cleared.length, refusal, locks: await navigator.locks.query()};
    });
    expect(result.sets).toEqual([5000, 5000, 40]);
    expect(result.cleared).toBe(3);
    expect(result.refusal).toBe('EBUSY');
    expect(result.locks).toEqual({held: [], pending: []});
  } finally { await browser.close(); server.stop(true); }
}, 60_000);
