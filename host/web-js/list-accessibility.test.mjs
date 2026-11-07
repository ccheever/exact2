import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { withLook } from '../web/look.mjs';
import { createDocument } from './dom.js';
import { listItem, unwrapListItem } from '../web/navigation.js';

test('the render DOM writes list items and unwraps their rows', () => {
  const previous = globalThis.document, doc = createDocument();
  globalThis.document = doc;
  try {
    const link = doc.createElement('a'); link.setAttribute('href', '#item'); link.textContent = 'Item';
    const wrapper = listItem(link); doc.root.append(wrapper);
    expect(wrapper.getAttribute('role')).toBe('listitem');
    expect(doc.rootHTML()).toContain('style="display:contents;"');
    expect(unwrapListItem(wrapper)).toBe(link);
    expect(doc.rootHTML()).toBe('<a href="#item">Item</a>');
  } finally { if (previous === undefined) delete globalThis.document; else globalThis.document = previous; }
});

test('list role fallbacks agree with Chrome’s native accessibility tree', async () => {
  const previous = globalThis.document, doc = createDocument();
  globalThis.document = doc;
  const cases = [['future LIST group', 'widget LISTITEM note', 'listitem'], ['group list', 'note', 'note'], ['list', 'note listitem', 'listitem'], ['directory list', 'note', 'listitem'], ['doc-chapter list', 'note', 'note'], ['list', 'doc-tip listitem', 'listitem'], ['graphics-document list', 'note', 'note'], ['list', 'graphics-object listitem', 'listitem'], ['comment list', 'note', 'note'], ['list', 'mark listitem', 'listitem'], ['doc-invalid list', 'note', 'listitem']];
  try {
    for (const [i, [ownerRole, rowRole]] of cases.entries()) {
      const owner = doc.createElement('div'), row = doc.createElement('span');
      owner.setAttribute('id', `role-case-${i}`); owner.setAttribute('role', ownerRole); row.setAttribute('role', rowRole); row.textContent = 'Row';
      owner.append(listItem(row, owner)); doc.root.append(owner);
    }
  } finally { if (previous === undefined) delete globalThis.document; else globalThis.document = previous; }
  const browser = await suiteBrowser('chromium');
  try {
    const page = await browser.newPage();
    await page.setContent(doc.rootHTML());
    const session = await page.context().newCDPSession(page);
    // ariaSnapshot parses DOM roles itself and treats uppercase tokens differently from Chrome.
    const { nodes } = await session.send('Accessibility.getFullAXTree');
    const byId = new Map(nodes.map(n => [n.nodeId, n]));
    const exposed = ids => (ids ?? []).flatMap(id => { const n = byId.get(id); return n.ignored ? exposed(n.childIds) : [n.role.value]; });
    const { root } = await session.send('DOM.getDocument');
    for (const [i, [, , expected]] of cases.entries()) {
      const { nodeId } = await session.send('DOM.querySelector', { nodeId: root.nodeId, selector: `#role-case-${i}` });
      const { node } = await session.send('DOM.describeNode', { nodeId });
      const owner = nodes.find(n => n.backendDOMNodeId === node.backendNodeId);
      expect(exposed(owner.childIds), `case ${i}`).toEqual([expected]);
    }
  } finally { await browser.close(); }
}, 60000);

test.each(['chromium', 'webkit'])('list items keep links, layout and identity on %s', async engine => {
  const root = resolve(import.meta.dir, '../..');
  const dir = mkdtempSync(resolve(root, 'target/list-items-'));
  const style = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
  let server, browser;
  try {
    const compiled = spawnSync(resolve(root, 'target/debug/exact-web-js'), ['js', resolve(import.meta.dir, 'conformance/list-items.contract'), '-o', dir], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${style}</style><link rel="stylesheet" href="/app.css"><div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`, { headers: { 'content-type': 'text/html' } });
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response(null, { status: 404 });
    } });
    browser = await suiteBrowser(engine);
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('[data-testid="row-Orchard"]'));
    await page.evaluate(async () => { await (await import('/rt.js')).pieces(); });
    const items = id => page.locator(`[data-testid="${id}"]`).ariaSnapshot();
    expect(await items('rows')).toMatch(/listitem:[\s\S]*link "Orchard:0"[\s\S]*listitem:[\s\S]*link "Harbor:0"[\s\S]*listitem:[\s\S]*link "Meadow:0"/);
    expect((await items('rows')).match(/listitem/g)).toHaveLength(3);
    expect((await items('virtual')).match(/listitem/g)).toHaveLength(3);
    expect((await items('conditional')).match(/listitem/g)).toHaveLength(3);
    expect((await items('role-list')).match(/listitem/g)).toHaveLength(1);
    expect(await items('role-fallbacks')).not.toContain('listitem');
    expect((await items('earlier-role')).match(/listitem/g)).toHaveLength(1);
    expect((await items('directory-role')).match(/listitem/g)).toHaveLength(1);
    for (const id of ['dpub-owner', 'graphics-owner', 'comment-owner']) expect(await page.locator(`[data-testid="${id}"] > [data-exact-list-item][role="listitem"]`).count()).toBe(0);
    for (const id of ['dpub-row', 'graphics-row', 'mark-row', 'unknown-owner']) expect(await page.locator(`[data-testid="${id}"] > [data-exact-list-item][role="listitem"]`).count()).toBe(1);
    const initial = await page.evaluate(() => {
      const node = id => document.querySelector(`[data-testid="${id}"]`);
      const boxes = id => { const p = node(id).getBoundingClientRect(); return [...node(id).querySelectorAll('a')].map(el => { const b = el.getBoundingClientRect(); return [b.x-p.x,b.y-p.y,b.width,b.height]; }); };
      globalThis.keptListLink = node('row-Orchard');
      return { grid: boxes('grid'), reference: boxes('reference'), inset: getComputedStyle(node('row-Orchard')).paddingLeft, separator: getComputedStyle(node('row-Orchard')).boxShadow, virtualWrapped: !!node('virtual').querySelector('[data-exact-list-item]') };
    });
    expect(initial.grid).toEqual(initial.reference);
    expect(initial.inset).toBe('16px');
    expect(initial.separator).not.toBe('none');
    expect(initial.virtualWrapped).toBe(false);
    expect(await page.evaluate(async () => {
      const { fitText, listItem } = await import('/navigation.js');
      return [false, true].map(wrapped => {
        const host = document.createElement('div'), text = document.createElement('span');
        host.style.width = '320px'; text.textContent = 'Short';
        text.style.cssText = 'display:block;--exact-fit:shrink';
        host.append(wrapped ? listItem(text) : text); document.body.append(host);
        fitText(text); const zoom = text.style.zoom; host.remove(); return zoom;
      });
    })).toEqual(['', '']);
    await page.locator('[data-testid="row-Harbor"]').hover();
    expect(await page.locator('[data-testid="row-Harbor"]').evaluate(el => getComputedStyle(el).backgroundImage)).not.toBe('none');
    await page.mouse.down();
    expect(await page.evaluate(() => ['Orchard', 'Harbor'].map(name => getComputedStyle(document.querySelector(`[data-testid="row-${name}"]`)).boxShadow))).toEqual(['none', 'none']);
    await page.mouse.up();
    await page.locator('[data-testid="row-Orchard"]').click();
    await page.evaluate(() => { keptListLink.focus(); document.querySelector('[data-testid="reverse"]').click(); });
    expect(await page.evaluate(() => ({ same: keptListLink === document.querySelector('[data-testid="row-Orchard"]'), focused: document.activeElement === keptListLink, label: keptListLink.textContent }))).toEqual({ same: true, focused: true, label: 'Orchard:1' });
    const leaving = await page.evaluate(async () => {
      const list = document.querySelector('[data-testid="rows"]');
      document.querySelector('[data-testid="drop"]').click();
      const ghosts = [...list.querySelectorAll('[data-exiting]')];
      const before = { items: list.querySelectorAll('[data-exact-list-item]').length, ghosts: ghosts.length, hidden: ghosts.every(el => el.inert && el.getAttribute('aria-hidden') === 'true') };
      await Promise.all(document.getAnimations().map(a => (a.finish(), a.finished)));
      return { ...before, remaining: list.querySelectorAll('[data-exiting]').length };
    });
    expect(leaving).toEqual({ items: 2, ghosts: 1, hidden: true, remaining: 0 });
    expect((await items('rows')).match(/listitem/g)).toHaveLength(2);
    await page.locator('[data-testid="toggle"]').click();
    expect((await items('conditional')).match(/listitem/g)).toHaveLength(2);
    expect(await items('role-list')).not.toContain('listitem');
    await page.locator('[data-testid="clear"]').click();
    await page.evaluate(async () => { await Promise.all(document.getAnimations().map(a => (a.finish(), a.finished))); });
    expect(await items('rows')).toBe('- list');
    expect(await page.locator('[data-testid="rows"] > [data-exact-list-item]').count()).toBe(0);
  } finally { await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true }); }
}, 60000);
