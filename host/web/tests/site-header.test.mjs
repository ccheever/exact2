// LLP 1153 D8, in Chrome, Firefox and WebKit: a route's header that holds a `nav`, or more than one link, is a site's.
// The navigation glue marks it `data-exact-site`, and the shell draws it as written: not sticky, no ground or edge,
// no bar mark, no Back drawn in it. So is a header whose author wrote `appearance: none`. The header iOS makes its bar
// of (one heading, a button) keeps the app bar, and so does one whose second link is in a menu, a popover being the
// top layer's. The page is the shell, the neutral look and the navigation glue, which both runtimes run; a page
// rendered ahead is marked by the same glue's `markBars` over the render's DOM (host/web-js/dom.js).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { createDocument } from '../../web-js/dom.js';
import { markBars } from '../navigation.js';

const ROOT = resolve(import.meta.dir, '../../..');
const heading = '<h2 role="heading" data-exact-text>Pokédex</h2>';
const add = '<button type="button" data-exact-on="press" aria-label="Add"><span data-exact-text>Add</span></button>';
const link = (to) => `<a href="/${to}" data-view><span data-exact-text>${to}</span></a>`;
// A stack of two routes whose pushed one has the header under test.
const stack = (id, inner, header = '') => `<main navigationKey="2" navigationBack="back" style="display:block">
  <div navigationKey="1"><header>${heading}</header></div>
  <div navigationKey="2" id="${id}"><header ${header} style="display:flex;align-items:center;gap:8px">${heading}${inner}</header><p>Content</p></div>
</main>`;
const CASES = {
  bar: [add],
  nav: [`<nav aria-label="Sections">${link('types')}</nav><input type="search" aria-label="Search">`],
  links: [link('all') + link('types')],
  menu: [link('all') + `<button type="button" popovertarget="more">More</button><div popover="auto" id="more">${link('types')}</div>`],
  own: [add, 'data-exact-said="appearance"'],
};
const WANT = {
  bar: { site: false, bar: true, position: 'sticky', edge: '1px', back: true },
  nav: { site: true, bar: false, position: 'static', edge: '0px', back: false },
  links: { site: true, bar: false, position: 'static', edge: '0px', back: false },
  menu: { site: false, bar: true, position: 'sticky', edge: '1px', back: true },
  own: { site: false, bar: false, position: 'static', edge: '0px', back: false },
};

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a site's header is drawn as written, and the app bar stays for a heading and its buttons`, async () => {
  const content = Object.entries(CASES).map(([id, [inner, header]]) => stack(id, inner, header)).join('');
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', () => `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', () => `<script type="module">${readFileSync(resolve(ROOT, 'host/web/navigation.js'), 'utf8')}
      const shell = document.getElementById('exact-root'); navigation.connect(shell, () => false, () => {}); navigation.project(shell, () => {}); globalThis.projected = true;</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 400, height: 900 } });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    await page.waitForFunction(() => globalThis.projected);
    const headers = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('[navigationkey="2"][id] > header')].map((h) => {
      const style = getComputedStyle(h), back = h.shadowRoot?.querySelector('button[part="back"]');
      return [h.parentElement.id, { site: h.hasAttribute('data-exact-site'), bar: h.hasAttribute('data-exact-bar'), position: style.position, edge: style.borderBottomWidth, back: !!back && !back.hidden }];
    })));
    expect(headers).toEqual(WANT);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 60000);

test('a page rendered ahead marks a site\'s header, and the bar only where it is no site\'s', () => {
  const doc = createDocument(), saved = globalThis.document;
  const el = (tag, attrs = {}, ...kids) => { const e = doc.createElement(tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); e.append(...kids); return e; };
  const title = () => el('h2', { role: 'heading', 'data-exact-text': '' }, 'Pokédex');
  const plus = () => el('button', { type: 'button', 'data-exact-on': 'press', 'aria-label': 'Add' });
  const a = (to) => el('a', { href: `/${to}`, 'data-view': '' }, to);
  const kids = {
    bar: () => [plus()],
    nav: () => [el('nav', {}, a('types')), el('input', { type: 'search' })],
    links: () => [a('all'), a('types')],
    menu: () => [a('all'), el('button', { type: 'button', popovertarget: 'more' }, 'More'), el('div', { popover: 'auto', id: 'more' }, a('types'))],
    own: () => [plus()],
  };
  for (const [id, made] of Object.entries(kids)) {
    const header = el('header', id === 'own' ? { 'data-exact-said': 'appearance' } : {}, title(), ...made());
    doc.root.append(el('main', { navigationKey: '1', navigationBack: 'back' }, el('div', { navigationKey: '1', id }, header)));
  }
  globalThis.document = doc;
  try { markBars(doc.root); } finally { globalThis.document = saved; }
  const marks = Object.fromEntries(Object.keys(kids).map((id) => {
    const header = doc.getElementById(id).firstElementChild;
    return [id, { site: header.hasAttribute('data-exact-site'), bar: header.hasAttribute('data-exact-bar') }];
  }));
  expect(marks).toEqual(Object.fromEntries(Object.entries(WANT).map(([id, { site, bar }]) => [id, { site, bar }])));
});
