import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test.each(['tap', 'pan', 'reveal'])('a modal escapes an inert ancestor for agent %s input', async input => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/modal-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Modal Agent Test', app: { id: 'test.modal-agent', name: 'Modal Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component ModalAgent
  state blocked = false
  state presses = 0
  state panned = 0
  action block
    blocked = true
  action pick
    presses = presses + 1
  action pan(dx: number, dy: number)
    panned = panned + dx
  view
    column inert=blocked testId="ancestor"
      button "Open modal" commandfor="modal" command="show-modal" testId="open-modal"
      button "Outside" press=pick testId="outside"
      dialog id="modal" testId="modal"
        button "Block ancestor" press=block testId="block"
        button "Pick" press=pick testId="inside"
        box pan=pan touch-action="none" width=240 height=60 testId="panner"
        column inert=true
          button "Inert child" press=pick testId="inert-child"
        button "Open menu" popovertarget="menu" testId="open-menu"
${input === 'reveal' ? `        box height=1200
        column inert=true
          button "Inert below" press=pick testId="inert-below"
        button "Below scrollport" press=pick testId="below"
` : ''}        column id="menu" popover="manual" testId="menu"
          button "Read menu" press=pick testId="read-menu"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'modal-agent-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      if (['/shared/input-glue.js', '/shared/prompt-glue.js'].includes(path)) return new Response(Bun.file(resolve(root, 'host/web', path.slice('/shared/'.length))));
      const file = resolve(dist, path === '/' ? 'index.html' : path.slice(1));
      return existsSync(file) ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
    await browser.tap('open-modal');
    if (input === 'tap') await browser.tap('open-menu');
    await browser.tap('block');
    expect((await browser.state()).slots.blocked).toBe(true);
    if (input === 'reveal') {
      const scroll = () => browser.carrier.evaluate('document.querySelector("dialog").scrollTop');
      const before = await browser.layout('below');
      expect(before.node.space.viewport.y).toBeGreaterThan(800);
      expect(await scroll()).toBe(0);
      // The wasm caller keeps the default refusal, which must not scroll the dialog.
      expect(await browser.carrier.evaluate(`(async () => {
        const { reveal } = await import('/shared/input-glue.js');
        return reveal(document.querySelector('[data-testid="below"]'), 0).scrolled;
      })()`)).toBe(false);
      expect(await scroll()).toBe(0);
      await expect(browser.tap('inert-below')).rejects.toThrow('hidden or inert');
      expect(await scroll()).toBe(0);
      const tapped = await browser.tap('below');
      expect(tapped.scrolled).toBeTruthy();
      expect(await scroll()).toBeGreaterThan(0);
      expect((await browser.state()).slots.presses).toBe(1);
    } else if (input === 'pan') {
      await browser.tap('panner', { drag: { dx: 40, dy: 0, over: 300 } });
      expect((await browser.state()).slots.panned).toBeGreaterThan(0);
    } else {
      await browser.tap('inside');
      await browser.tap('read-menu');
      expect((await browser.state()).slots.presses).toBe(2);
      for (const target of ['inside', 'read-menu']) expect((await browser.layout(target)).node.visible.inert).toBe(false);
      expect(await browser.carrier.evaluate('document.querySelector("dialog").matches(":modal") && document.querySelector("[popover]").matches(":popover-open")')).toBe(true);
      for (const target of ['outside', 'inert-child']) {
        expect((await browser.layout(target)).node.visible.inert).toBe(true);
        await expect(browser.tap(target)).rejects.toThrow('hidden or inert');
      }
      await browser.carrier.evaluate('document.querySelector("dialog").inert = true');
      expect((await browser.layout('inside')).node.visible.inert).toBe(true);
      await expect(browser.tap('inside')).rejects.toThrow('hidden or inert');
      expect((await browser.state()).slots.presses).toBe(2);
    }
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
