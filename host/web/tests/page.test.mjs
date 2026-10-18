import { beforeAll, afterAll, expect, spyOn } from 'bun:test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp, chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const { executable, unavailable } = chromium(), check = need('chrome', unavailable);
let child, profile, cdp, server;
function browser() {
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-page-ready-'));
  const child = spawn(executable, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'],
  { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  return { child, profile, cdp: new Cdp(child.stdio[3], child.stdio[4]) };
}
beforeAll(() => {
  if (unavailable) return;
  server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    const pages = {
      '/missing': '<script type="module">import "./missing.js"; window.ready = true;</script>',
      '/blocked': '<script src="/blocked.js"></script>',
      '/throw': '<script>throw new Error("fixture startup failed"); window.ready = true;</script>',
      '/reject': '<script type="module">await Promise.reject(new Error("fixture initialization rejected")); window.ready = true;</script>',
      '/ready': '<script>window.ready = true;</script>',
      '/never': '<script>window.initialized = true;</script>',
      '/async': '<script type="module">const image = new Image(); image.src = "/missing-image"; await image.decode().catch(() => {}); window.ready = true;</script>',
    };
    return pages[path] ? new Response(pages[path], { headers: { 'content-type': 'text/html' } })
      : new Response('missing', { status: 404 });
  } });
  ({ child, profile, cdp } = browser());
});
afterAll(() => {
  child?.kill(); server?.stop(true);
  if (profile) rmSync(profile, { recursive: true, force: true });
});

async function withPage(run) {
  const { browserContextId } = await cdp.send('Target.createBrowserContext');
  const listeners = cdp.listeners.length;
  const closeListeners = cdp.closeListeners.size;
  try {
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank', browserContextId });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await run((path, options) => readyPage(cdp, sessionId, `${server.url.origin}${path}`, options),
      (method, params) => cdp.send(method, params, sessionId));
    expect(cdp.listeners.length).toBe(listeners);
    expect(cdp.closeListeners.size).toBe(closeListeners);
  } finally {
    await cdp.send('Target.disposeBrowserContext', { browserContextId });
  }
}

check('readiness reports a missing module with its URL and HTTP status', () => withPage(async open => {
  await expect(open('/missing')).rejects.toThrow(/404.*missing\.js/);
  await expect(open('/missing-page')).rejects.toThrow(/404.*missing-page/);
}), 60000);

check('readiness reports a script request that fails without an HTTP response', () => withPage(async (open, call) => {
  await call('Network.enable');
  await call('Network.setBlockedURLs', { urls: ['*/blocked.js'] });
  await expect(open('/blocked')).rejects.toThrow(/blocked: inspector.*blocked\.js/);
}), 60000);

check('readiness reports startup exceptions and rejected initialization', () => withPage(async open => {
  await expect(open('/throw')).rejects.toThrow('fixture startup failed');
  await expect(open('/reject')).rejects.toThrow('fixture initialization rejected');
}), 60000);

check('readiness observes immediate and asynchronous completion across navigations', () => withPage(async open => {
  await open('/ready');
  await open('/async');
}), 60000);

check('readiness diagnoses a page that never signals before its caller times out', () => withPage(async open => {
  await expect(open('/never', { timeoutMs: 50 })).rejects.toThrow('window.ready was not set within 50 ms');
}), 60000);

check('the default readiness bound leaves thirty seconds for initialization', async () => {
  const schedule = globalThis.setTimeout, delays = [];
  const timer = spyOn(globalThis, 'setTimeout').mockImplementation((callback, ms, ...args) => {
    delays.push(ms);
    return schedule(callback, ms, ...args);
  });
  try {
    await withPage(open => open('/ready'));
    expect(delays).toContain(30000);
  } finally { timer.mockRestore(); }
}, 60000);

async function duringInitialization(run, intercept = () => {}) {
  const { child, profile, cdp } = browser();
  const closed = once(child, 'close');
  let outcome;
  try {
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const send = cdp.send.bind(cdp);
    let navigated, settled = false;
    const navigation = new Promise(resolve => { navigated = resolve; });
    cdp.send = async (...args) => {
      await intercept(args[0], child);
      const result = await send(...args);
      if (args[0] === 'Page.navigate') navigated();
      return result;
    };
    const url = `${server.url.origin}/never`;
    const readiness = readyPage(cdp, sessionId, url);
    outcome = readiness.catch(error => error).finally(() => { settled = true; });
    await Promise.race([navigation, readiness]);
    await run({ child, cdp, targetId, sessionId, url, outcome, closed, settled: () => settled });
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await outcome;
    await closed;
    rmSync(profile, { recursive: true, force: true });
  }
}

check('the initialization barrier propagates setup, navigation and early browser failures', async () => {
  for (const method of ['Page.enable', 'Page.navigate', 'browser exit']) {
    await expect(duringInitialization(() => { throw new Error('navigation unexpectedly succeeded'); }, async (command, child) => {
      if (command === method) throw new Error(`fixture refused ${method}`);
      if (method === 'browser exit' && command === 'Page.navigate') {
        const closed = once(child, 'close'), pid = child.pid;
        process.kill(pid, 'SIGKILL');
        await closed;
      }
    })).rejects.toThrow(method === 'browser exit' ? 'the DevTools pipe closed' : `fixture refused ${method}`);
  }
}, 60000);

check('readiness rejects when Chrome exits after navigation has answered', () => duringInitialization(async ({ child, cdp, url, outcome, closed, settled }) => {
  expect(cdp.pending.size).toBe(0);
  const pid = child.pid;
  process.kill(pid, 'SIGKILL');
  await closed;
  // Child close follows the pipe events; let their promise continuations finish.
  await new Promise(setImmediate);
  expect(cdp.closed).toBe('the DevTools pipe closed');
  expect(settled()).toBe(true);
  expect((await outcome).message).toBe(`${url}: the DevTools pipe closed`);
  expect(cdp.listeners.length).toBe(0);
  expect(cdp.closeListeners.size).toBe(0);
}), 60000);

check('readiness rejects when its target detaches, but ignores another target closing', () => duringInitialization(async ({ cdp, targetId, sessionId, url, outcome, settled }) => {
  const closeTarget = async (targetId, sessionId) => {
    let listener;
    const detached = new Promise(resolve => {
      listener = message => {
        if (message.method === 'Target.detachedFromTarget' && message.params.sessionId === sessionId) resolve();
      };
      cdp.listeners.push(listener);
    });
    try {
      await cdp.send('Target.closeTarget', { targetId });
      await Promise.race([detached, outcome.then(error => { throw error; })]);
    } finally { cdp.listeners.splice(cdp.listeners.indexOf(listener), 1); }
  };
  const other = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const attached = await cdp.send('Target.attachToTarget', { targetId: other.targetId, flatten: true });
  await closeTarget(other.targetId, attached.sessionId);
  await new Promise(setImmediate);
  expect(settled()).toBe(false);

  await closeTarget(targetId, sessionId);
  await new Promise(setImmediate);
  expect(settled()).toBe(true);
  expect((await outcome).message).toBe(`${url}: page target detached (session ${sessionId})`);
  expect(cdp.closed).toBeNull();
  expect((await cdp.send('Browser.getVersion')).product).toContain('Chrome');
  expect(cdp.listeners.length).toBe(0);
  expect(cdp.closeListeners.size).toBe(0);
}), 60000);

check('a page fixture disposes its browser context when attachment fails', async () => {
  const send = cdp.send.bind(cdp), before = await send('Target.getBrowserContexts');
  const failure = new Error('fixture attachment refused');
  const calls = spyOn(cdp, 'send').mockImplementation((method, ...args) =>
    method === 'Target.attachToTarget' ? Promise.reject(failure) : send(method, ...args));
  try {
    await expect(withPage(() => {})).rejects.toBe(failure);
    expect(await send('Target.getBrowserContexts')).toEqual(before);
  } finally {
    calls.mockRestore();
    const { browserContextIds } = await send('Target.getBrowserContexts');
    for (const id of browserContextIds.filter(id => !before.browserContextIds.includes(id)))
      await send('Target.disposeBrowserContext', { browserContextId: id });
  }
}, 60000);
