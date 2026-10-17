import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test.each(['tap', 'pan', 'reveal', 'pointer', 'hover', 'wheel', 'check', 'contextpopover', 'autofocus', 'drop'])('a modal escapes an inert ancestor for agent %s input', async input => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/modal-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Modal Agent Test', app: { id: 'test.modal-agent', name: 'Modal Agent Test' }, host: { web: {} }, file_handlers: [{ action: '/', accept: { 'text/plain': ['.txt'] } }] }));
    writeFileSync(resolve(dir, 'app.contract'), `component ModalAgent
  state blocked = false
  state presses = 0
  state panned = 0
  state drops = 0
  state downs = 0
  state ups = 0
  state moves = 0
  state wheels = 0
  state editing = false
  action drop
    drops = drops + 1
  action down
    downs = downs + 1
  action up
    ups = ups + 1
  action move
    moves = moves + 1
  action wheel
    wheels = wheels + 1
  action edit
    editing = true
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
        button "Pick" press=pick pointerdown=down pointerup=up wheel=wheel contextPopover="menu" testId="inside"
        box drop=drop width=240 height=40 testId="drop"
        box pointermove=move width=240 height=40 testId="hover"
        button "Edit" press=edit testId="edit"
        if editing
          input autofocus=true testId="editor"
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
      if (path.startsWith('/shared/')) return new Response(Bun.file(resolve(root, 'host/web', path.slice('/shared/'.length))));
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
    } else if (input === 'pointer') {
      await browser.tap('inside');
      expect((await browser.state()).slots).toMatchObject({ downs: 1, ups: 1, presses: 1 });
      await browser.carrier.evaluate(`(() => {
        const target = document.querySelector('[data-testid="inside"]'); target.inert = true;
        for (const type of ['pointerdown', 'pointerup']) target.dispatchEvent(new PointerEvent(type, { bubbles: true, isPrimary: true, pointerId: 1 }));
      })()`);
      expect((await browser.state()).slots).toMatchObject({ downs: 1, ups: 1 });
    } else if (input === 'hover') {
      await browser.tap('hover', { hover: true });
      expect((await browser.state()).slots.moves).toBeGreaterThan(0);
    } else if (input === 'wheel') {
      await browser.tap('inside', { wheel: [0, 40] });
      expect((await browser.state()).slots.wheels).toBe(1);
      await browser.carrier.evaluate(`(() => {
        const target = document.querySelector('[data-testid="inside"]'); target.inert = true;
        target.dispatchEvent(new WheelEvent('wheel', { bubbles: true, deltaY: 40 }));
      })()`);
      expect((await browser.state()).slots.wheels).toBe(1);
    } else if (input === 'check') {
      const tree = (await browser.tree()).nodes;
      const checked = (await browser.op({ op: 'check' })).items.map(item => item.id);
      expect(checked).toContain(tree.find(n => n.props.testId === 'inside').id);
      for (const id of ['outside', 'inert-child']) expect(checked).not.toContain(tree.find(n => n.props.testId === id).id);
      await browser.carrier.evaluate('document.querySelector("dialog").inert = true');
      expect((await browser.op({ op: 'check' })).items).toHaveLength(0);
    } else if (input === 'contextpopover') {
      await browser.tap('inside', { contextmenu: true });
      expect(await browser.carrier.evaluate('document.querySelector("[popover]").matches(":popover-open")')).toBe(true);
    } else if (input === 'autofocus') {
      await browser.tap('edit');
      expect(await browser.carrier.evaluate('document.activeElement.dataset.testid')).toBe('editor');
    } else if (input === 'drop') {
      expect(await browser.carrier.evaluate(`(() => {
        const target = document.querySelector('[data-testid="drop"]'), dataTransfer = new DataTransfer();
        dataTransfer.items.add(new File(['hello'], 'note.txt', { type: 'text/plain' }));
        return ['dragover', 'drop'].map(type => {
          const event = new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer });
          target.dispatchEvent(event); return event.defaultPrevented;
        });
      })()`)).toEqual([true, true]);
      await browser.clock('settle');
      expect((await browser.state()).slots.drops).toBe(1);
      expect(await browser.carrier.evaluate(`(() => {
        const target = document.querySelector('[data-testid="drop"]'), dataTransfer = new DataTransfer();
        target.inert = true; dataTransfer.items.add(new File(['hello'], 'note.txt', { type: 'text/plain' }));
        return ['dragover', 'drop'].map(type => {
          const event = new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer });
          target.dispatchEvent(event); return event.defaultPrevented;
        });
      })()`)).toEqual([false, false]);
      await browser.clock('settle');
      expect((await browser.state()).slots.drops).toBe(1);
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


test('a dev checkpoint restores modal focus past inert ancestors, but not within inert content', async () => {
  const { chromium } = await import('playwright-core');
  const { chromium: browserPath } = await import('../../scripts/agent-launch.mjs');
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/rt.js') return new Response('export const clock = {}; export const carryRouter = null, launchLocation = null, res = null, sig = null;', { headers: { 'content-type': 'text/javascript' } });
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    if (['/checkpoint.js', '/focus.js'].includes(path)) return new Response(Bun.file(new URL('.' + path, import.meta.url)));
    return new Response(`<main id="exact-root" inert><dialog data-carry-type="dialog"><input data-carry-type="input"></dialog></main>
      <script type="application/vnd.exact.dev-checkpoint">{"focus":{"path":[0,0],"type":"input"}}</script>
      <script type="module">
        import { prepareDev } from '/checkpoint.js';
        document.querySelector('dialog').showModal();
        window.restore = prepareDev();
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true, args: ['--no-sandbox'] });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => typeof window.restore === 'function');
    expect(await page.evaluate(() => { document.activeElement.blur(); restore(); return document.activeElement.localName; })).toBe('input');
    expect(await page.evaluate(() => { document.activeElement.blur(); document.querySelector('input').inert = true; restore(); return document.activeElement.localName; })).toBe('body');
    expect(await page.evaluate(() => { document.querySelector('input').inert = false; document.querySelector('dialog').inert = true; restore(); return document.activeElement.localName; })).toBe('body');
  } finally { await browser?.close(); server.stop(true); }
}, 30000);
