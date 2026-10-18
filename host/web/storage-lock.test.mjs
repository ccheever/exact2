// Actual Web Locks: wait for a competing owner, cancel bounded waits, and
// release ancestor locks after a partial acquisition fails.
import { expect, test } from 'bun:test';
import { chromium as browserType } from 'playwright-core';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../../scripts/agent-launch.mjs';

test('storage locks queue and time out without retaining acquired ancestors', async () => {
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path.endsWith('.js')) return new Response(readFileSync(resolve(import.meta.dirname, path.slice(1))), {headers: {'content-type': 'text/javascript'}});
    return new Response('<!doctype html>');
  }});
  const browser = await browserType.launch({executablePath: chromium().executable, args: ['--no-sandbox']});
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
