import { test, expect } from 'bun:test';
import { existsSync } from 'node:fs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

async function fixture(run) {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path.endsWith('/names.js')) return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (path === '/') return new Response('<div id="exact-root"></div>', { headers: { 'content-type': 'text/html' } });
    const file = new URL('../..' + path, import.meta.url);
    return new Response(Bun.file(existsSync(file) ? file : new URL('../web/' + path.split('/').pop(), import.meta.url)));
  } });
  const browser = await suiteBrowser(), page = await browser.newPage();
  try {
    await page.goto(server.url.href);
    await page.evaluate(async () => {
      const rt = await import('/host/web-js/rt.js');
      const root = document.querySelector('#exact-root');
      const parent = rt.h(root, 'div', 0);
      const outer = rt.h(parent, 'button', 0, { popovertarget: 'outer-pop' });
      const inner = rt.h(outer, 'button', 0);
      const span = rt.h(inner, 'span', 0, null, 'Press');
      const pops = ['outer-pop', 'inner-pop', 'direct-pop', 'bound-pop'].map(id => rt.h(root, 'div', 0, { id, popover: 'manual' }, id));
      window.fixture = { rt, parent, outer, inner, span, pops, log: [] };
    });
    await run(page);
  } finally { await browser.close(); server.stop(true); }
}

test('only the nearest button activates, including handlerless buttons and SVG descendants', () => fixture(async page => {
  const result = await page.evaluate(async () => {
    const { rt, parent, outer, inner, span, pops, log } = fixture;
    rt.on(parent, 'press', () => log.push('press'), rt.onPress);
    span.click();
    const ordinary = pops[0].matches(':popover-open');
    await new Promise(resolve => setTimeout(resolve));
    pops.forEach(p => { if (p.matches(':popover-open')) p.hidePopover(); });
    inner.setAttribute('popovertarget', 'inner-pop');
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    const rect = document.createElementNS(svg.namespaceURI, 'rect'); svg.append(rect); inner.append(svg);
    rect.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    const nested = pops.slice(0, 2).map(p => p.matches(':popover-open'));
    await new Promise(resolve => setTimeout(resolve));
    return { ordinary, nested, log, restored: outer.getAttribute('popovertarget') };
  });
  expect(result).toEqual({ ordinary: false, nested: [false, true], log: ['press', 'press'], restored: 'outer-pop' });
}), 30000);

test('a reentrant outer click activates after its ancestor press, with the last authored target', () => fixture(async page => {
  const result = await page.evaluate(async () => {
    const { rt, parent, outer, inner, pops, log } = fixture;
    rt.on(parent, 'press', () => { log.push('ancestor'); outer.setAttribute('popovertarget', 'direct-pop'); rt.P(outer, 'popovertarget', () => 'bound-pop'); }, rt.onPress);
    for (const pop of pops) pop.addEventListener('beforetoggle', e => { if (e.newState === 'open') log.push(pop.id); });
    rt.on(inner, 'press', () => { outer.click(); log.push('returned'); }, rt.onPress);
    inner.click();
    await new Promise(resolve => setTimeout(resolve));
    return { log, target: outer.getAttribute('popovertarget'), open: pops.map(p => p.matches(':popover-open')) };
  });
  expect(result).toEqual({ log: ['ancestor', 'bound-pop', 'returned'], target: 'bound-pop', open: [false, false, false, true] });
}), 30000);


test('a browser-delivered click keeps outer popover and dialog invokers closed', () => fixture(async page => {
  await page.evaluate(() => {
    const { rt, outer, inner, span } = fixture;
    outer.setAttribute('commandfor', 'dialog'); outer.setAttribute('command', 'show-modal');
    const dialog = rt.h(document.querySelector('#exact-root'), 'dialog', 0, { id: 'dialog' });
    inner.setAttribute('popovertarget', 'inner-pop');
    span.id = 'press';
    fixture.dialog = dialog;
  });
  await page.locator('#press').click();
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve)));
  expect(await page.evaluate(() => ({
    open: fixture.pops.slice(0, 2).map(p => p.matches(':popover-open')),
    dialog: fixture.dialog.open,
    target: fixture.outer.getAttribute('commandfor'),
  }))).toEqual({ open: [false, true], dialog: false, target: 'dialog' });
}), 30000);


test('a real click can call an outer click without leaving its attributes suppressed', () => fixture(async page => {
  await page.evaluate(() => {
    const { rt, parent, outer, inner, span, log, pops } = fixture;
    span.id = 'press';
    rt.on(parent, 'press', () => { log.push('ancestor'); outer.setAttribute('popovertarget', 'direct-pop'); rt.P(outer, 'popovertarget', () => 'bound-pop'); }, rt.onPress);
    pops[3].addEventListener('beforetoggle', e => { if (e.newState === 'open') log.push('open'); });
    rt.on(inner, 'press', () => { outer.click(); log.push('returned'); }, rt.onPress);
  });
  await page.locator('#press').click();
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve)));
  expect(await page.evaluate(() => ({ log: fixture.log, target: fixture.outer.getAttribute('popovertarget'), open: fixture.pops[3].matches(':popover-open') })))
    .toEqual({ log: ['ancestor', 'open', 'returned'], target: 'bound-pop', open: true });
}), 30000);

for (const name of ['popovertarget', 'commandfor']) {
  test(`an inner click preserves a bound removal of ${name}`, () => fixture(async page => {
    const result = await page.evaluate(async name => {
      const { rt, outer, inner, pops } = fixture;
      outer.removeAttribute('popovertarget');
      const dialog = rt.h(document.querySelector('#exact-root'), 'dialog', 0, { id: 'dialog' });
      outer.setAttribute(name, name === 'commandfor' ? 'dialog' : 'outer-pop');
      outer.setAttribute('command', 'show-modal');
      rt.on(inner, 'press', () => rt.P(outer, name, () => null), rt.onPress);
      inner.click();
      await new Promise(resolve => setTimeout(resolve));
      const target = outer.getAttribute(name);
      outer.click();
      return { target, popover: pops[0].matches(':popover-open'), dialog: dialog.open };
    }, name);
    expect(result).toEqual({ target: null, popover: false, dialog: false });
  }), 30000);
}
