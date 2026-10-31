// LLP 1116.000 D4, in Chrome, Firefox and WebKit: a pushed route's bar draws the system Back for the root's
// `navigationBack` control, where iOS draws UIKit's (NavigationBarIOS.swift `projectBack`): the arrow alone for a
// control with no words, else its words, the control pressed through it. A control that is an item of the bar iOS makes
// of its header is not shown; one elsewhere in the route, the title's own pressable or in a menu stays where it is, and
// one in any other header that shows it is that header's way back, with no Back beside it (the web's choice: iOS draws
// UIKit's there too). A control hidden in that header leaves a drawn Back. The bar discounts its own hiding mark
// when deciding whether the control shows. The Back presses its control wherever it is. A disabled control, one the author
// made inert, a route with no bar or whose header's author wrote `appearance: none`, a stack's root and a sheet leave
// no Back, and so does a control whose author wrote `appearance: none`, which stays where it is (iOS's `authoredBack`). The Back is
// a web app's: an arrow at the bar's font and "Back", or the control's words, in the bar's ink. The page is the shell, the neutral look and the navigation glue, which both runtimes run, over nodes as
// either writes them: an author's rows inline (the wasm runtime) or in a class (the JS target).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open } from '../../../scripts/agent.mjs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { withLook } from '../look.mjs';
import { serveBuildTree } from '../serve.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
// A stack of two routes whose pushed one holds `control` in its header or its content (`place`), its header holding
// `headings` and written with `header`'s attributes.
const heading = (text) => `<h2 role="heading" data-exact-text>${text}</h2>`;
const stack = (id, control, place = 'header', attributes = '', headings = heading('Updates'), header = '') => `<main navigationKey="2" navigationBack="back" style="display:block">
  <div navigationKey="1"><header><h1>Settings</h1></header></div>
  <div navigationKey="2" id="${id}" ${attributes}><header ${header} style="display:flex;align-items:center">${place === 'header' ? control : ''}${headings}</header>${place === 'content' ? control : ''}</div>
</main>`;
const symbol = (attributes = '') => `<button type="button" id="back" data-exact-on="press" aria-label="Back" ${attributes}><img data-symbol-source="symbol:back" data-symbol-path="M15 5 8 12l7 7" style="width:20px;height:20px"></button>`;
const hiddenControls = [
  ['hidden-self', symbol('hidden class="hidden-control"')],
  ['display-self', symbol('style="display:none"')],
  ['hidden-box', `<div hidden>${symbol()}</div>`],
  ['display-box', `<div style="display:none">${symbol()}</div>`],
  ['visibility-self', symbol('style="visibility:hidden"')],
  ['visibility-box', `<div style="visibility:hidden">${symbol()}</div>`],
];
const content = `<style>#exact-root#exact-root { & .sized { width: 44px; height: 44px; display: flex; } & .hidden-control[hidden] { display: none; } }</style>
  ${stack('symbol', symbol('class="sized"'))}
  ${stack('inline', symbol('style="width:44px;height:44px;display:flex"'))}
  ${stack('titled', '<button type="button" id="back" data-exact-on="press"><span data-exact-text>Done</span></button>')}
  ${stack('long', '<button type="button" id="back" data-exact-on="press"><span data-exact-text>All my conversations</span></button>')}
  ${stack('keys', symbol('aria-keyshortcuts="Meta+["'))}
  ${stack('bare', symbol(), 'header', 'data-navigationbar="none"')}${stack('bare-content', symbol(), 'content', 'data-navigationbar="none"')}
  ${stack('headless', symbol(), 'header', '', '')}${stack('two', symbol(), 'header', '', heading('Updates') + heading('Today'))}
  ${hiddenControls.map(([id, control]) => stack(id, control, 'header', '', heading('Updates') + heading('Today'))).join('')}
  ${stack('held-visible', symbol())}${stack('held-hidden', symbol())}
  ${['visible', 'hidden'].map(kind => stack(`held-ancestor-${kind}`, `<div id="back" data-exact-on="press">${symbol().replace('id="back"', 'id="inner"')}</div>`)).join('')}
  ${stack('row', symbol(), 'header', '', '<div role="heading" aria-level="1"><span data-exact-text>Updates</span></div>')}
  ${stack('authored', symbol(), 'header', '', heading('Updates'), 'data-exact-said="appearance"')}${stack('authored-content', symbol(), 'content', '', heading('Updates'), 'data-exact-said="appearance"')}
  ${stack('authored-alone', '', 'content', '', heading('Updates'), 'data-exact-said="appearance"')}${stack('alone', '', 'content')}
  ${stack('own-back', symbol('data-exact-said="appearance"'))}
  ${stack('wraps', `<button type="button" id="back" data-exact-on="press" aria-label="Back"><img data-symbol-source="symbol:back" data-symbol-path="M15 5 8 12l7 7" style="width:20px;height:20px">${heading('Inbox')}</button>`, 'header', '', '')}
  ${stack('in-menu', `<div popover="manual" id="back-menu">${symbol()}</div>`)}${stack('in-dialog', `<dialog id="back-dialog">${symbol()}</dialog>`)}
  ${stack('hidden-content', `<div hidden>${symbol()}</div>`, 'content')}${stack('inert-content', `<div inert>${symbol()}</div>`, 'content')}
  ${stack('hidden-header', symbol(), 'header', '', heading('Updates'), 'style="display:none"')}${stack('two-dialog', `<dialog id="two-dialog">${symbol()}</dialog>`, 'header', '', heading('Updates') + heading('Today'))}${stack('search', '<input type="search" id="back" data-exact-on="press" aria-label="Back">')}
  ${stack('pressing', symbol(), 'header', '', heading('Updates'), 'data-exact-on="press"')}
  ${stack('commanded', symbol(), 'header', '', `<button type="button" commandfor="dialog" command="show-modal">${heading('Updates')}</button><dialog id="dialog"></dialog>`)}
  ${stack('popover-first', symbol(), 'header', '', `<div popover id="menu"><button type="button" data-exact-on="press">${heading('Menu')}</button></div><div data-exact-on="press">${heading('Updates')}</div>`)}
  ${stack('titled-tap', symbol(), 'header', '', `<div data-exact-on="press">${heading('Updates')}</div><button type="button" data-exact-on="press">${heading('Edit')}</button>`)}
  ${stack('foreign', symbol())}${stack('moved', symbol())}${stack('route-accent', symbol(), 'header', 'style="--exact-accent: rgb(204, 0, 0)"')}
  ${stack('app-accent', symbol()).replace('style="display:block"', 'style="display:block; --exact-accent: rgb(0, 0, 204)"')}
  <div>${stack('nested-accent', symbol()).replace('style="display:block"', 'style="display:block; --exact-accent: rgb(0, 128, 0)"')}</div>
  ${stack('disabled', symbol('disabled'))}${stack('dimmed', symbol('disabled').replace(' data-exact-on="press"', ''))}${stack('dimmed-link', symbol('disabled role="link"').replace(' data-exact-on="press"', ''))}
  ${stack('content', symbol(), 'content')}
  ${stack('sheet', symbol(), 'header', 'navigationPresentation="modal"')}
  <main navigationKey="1" navigationBack="back" style="display:block"><div navigationKey="1" id="root"><header>${symbol()}${heading('Settings')}</header></div></main>`;

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a pushed route's bar draws the system Back for its Back control`, async () => {
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', () => `<style>#symbol > header::part(back) { color: rgb(255, 0, 0); }</style><div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', () => `<script type="module">${readFileSync(resolve(ROOT, 'host/web/navigation.js'), 'utf8')}
      // A header whose shadow root is someone else's.
      document.querySelector('#foreign > header').attachShadow({ mode: 'open' }).innerHTML = '<slot></slot>';
      const shell = document.getElementById('exact-root'); navigation.connect(shell, () => false, () => {}); navigation.project(shell, () => {});
      window.project = () => navigation.project(shell, () => {});
      window.pressed = []; for (const control of document.querySelectorAll('#back')) control.addEventListener('click', () => pressed.push(control.closest('[id]:not(#back)')?.id ?? ''));
      window.ready = true;</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 400, height: 900 }, deviceScaleFactor: 4 });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    await page.waitForFunction(() => window.ready);
    // Per route: the bar's Back shown, its words, its name and its shortcut, and whether the control shows (in its
    // menu, opened). Words too long for the bar are "Back"; with no control, the Back has the title beneath. A route
    // with no bar, a header iOS makes no bar of and one whose shadow root is someone else's leave the control the way
    // back. The bar is kernel/src/bars.rs `header_bar`'s: a header whose author wrote `appearance: none` is none, nor
    // is one with no heading text, two, or a row whose role is `heading`; a heading in the title's pressable counts,
    // wrapped in a button that shows a dialog too, and one in another pressable (an item's) or a popover does not; the
    // header's own `press` is not read. Its items are the pressables its walk reaches, not the title's nor a search
    // field, and a disabled button, pressed or not (`fact::DISABLED`): a disabled control that presses nothing is not
    // shown, unless its role is not a button's (a link's). A control in a closed dialog or hidden in the content is not
    // shown, and the Back is drawn for it, as it is for one in a header not shown or in a dialog of a header iOS makes no
    // bar of; one the author made inert has none.
    // The shell's button display rule overrides bare `hidden`; assert actual visibility for every fixture.
    const hiddenIds = hiddenControls.map(([id]) => id);
    expect(await page.evaluate((ids) => ids.map(id => document.querySelector(`#${id} #back`).checkVisibility({ visibilityProperty: true })), hiddenIds)).toEqual(hiddenIds.map(() => false));
    expect(await page.evaluate(() => ['held-visible', 'held-hidden'].map(id => document.querySelector(`#${id} #back`).hasAttribute('data-exact-bar-back')))).toEqual([true, true]);
    await page.evaluate(() => {
      for (const id of ['held-visible', 'held-hidden']) {
        const route = document.getElementById(id);
        route.querySelector('header').append(route.querySelector('h2').cloneNode(true));
        if (id === 'held-hidden') route.querySelector('#back').style.display = 'none';
      }
      project();
    });
    expect(await page.evaluate(() => ['held-visible', 'held-hidden'].map(id => {
      const control = document.querySelector(`#${id} #back`);
      return [control.hasAttribute('data-exact-bar-back'), control.checkVisibility({ visibilityProperty: true })];
    }))).toEqual([[false, true], [false, false]]);
    // A different Back control can be inside the item the previous projection hid.
    expect(await page.evaluate(() => ['visible', 'hidden'].map(kind => document.querySelector(`#held-ancestor-${kind} #back`).hasAttribute('data-exact-bar-back')))).toEqual([true, true]);
    expect(await page.evaluate(() => ['visible', 'hidden'].map(kind => {
      const route = document.getElementById(`held-ancestor-${kind}`), header = route.querySelector('header');
      route.parentElement.setAttribute('navigationBack', 'inner');
      header.append(header.querySelector('h2').cloneNode(true));
      if (kind === 'hidden') route.querySelector('#back').style.display = 'none';
      project();
      return [kind, route.querySelectorAll('[data-exact-bar-back]').length,
        route.querySelector('#inner').checkVisibility({ visibilityProperty: true }),
        header.shadowRoot.querySelector('button[part="back"]').checkVisibility({ visibilityProperty: true })];
    }))).toEqual([['visible', 0, true, false], ['hidden', 0, false, true]]);
    const backIds = [...hiddenIds, 'held-hidden'];
    expect(await page.evaluate((ids) => ids.map(id => {
      const back = document.querySelector(`#${id} > header`).shadowRoot?.querySelector('button[part="back"]');
      return [id, back?.checkVisibility({ visibilityProperty: true }) ?? false];
    }), [...backIds, 'held-visible', 'two'])).toEqual([...backIds.map(id => [id, true]), ['held-visible', false], ['two', false]]);
    expect(await page.evaluate((ids) => ids.flatMap(id => {
      document.querySelector(`#${id} > header`).shadowRoot.querySelector('button[part="back"]').click();
      return pressed.splice(0);
    }), backIds)).toEqual(backIds);
    await page.evaluate(() => document.getElementById('back-menu').showPopover());
    const bars = (ids) => page.evaluate((ids) => ids.map((id) => {
      const route = document.getElementById(id), back = route.querySelector(':scope > header').shadowRoot?.querySelector('button[part="back"]');
      const shown = !!back && !back.hidden;
      return [id, shown, shown ? back.lastChild.hidden ? '' : back.lastChild.textContent : null, shown ? back.getAttribute('aria-label') : null, shown ? back.getAttribute('aria-keyshortcuts') : null, route.querySelector('#back')?.getClientRects().length > 0];
    }), ids);
    expect(await bars(['wraps', 'in-menu', 'search', 'alone', 'authored-content', 'authored-alone', 'in-dialog', 'hidden-content', 'hidden-header', 'two-dialog', 'inert-content'])).toEqual([
      ['wraps', true, 'Inbox', 'Back', null, true], ['in-menu', true, '', 'Back', null, true], ['search', true, '', 'Back', null, true],
      ['alone', true, 'Back', 'Back to Settings', null, false], ['authored-content', false, null, null, null, true],
      ['authored-alone', false, null, null, null, false], ['in-dialog', true, '', 'Back', null, false], ['hidden-content', true, '', 'Back', null, false],
      ['hidden-header', true, '', 'Back', null, false], ['two-dialog', true, '', 'Back', null, false], ['inert-content', false, null, null, null, true],
    ]);
    await page.evaluate(() => document.getElementById('back-menu').hidePopover());
    // The Back presses a control wherever it is, as UIKit's Back does: in a closed menu or dialog, or hidden in the content.
    expect(await page.evaluate(() => ['in-menu', 'in-dialog', 'hidden-content', 'two-dialog'].flatMap((id) => (document.querySelector(`#${id} > header`).shadowRoot.querySelector('button[part="back"]').click(), pressed.splice(0)))))
      .toEqual(['back-menu', 'back-dialog', 'hidden-content', 'two-dialog']);
    expect(await bars(['symbol', 'inline', 'titled', 'long', 'keys', 'disabled', 'dimmed', 'dimmed-link', 'content', 'bare', 'bare-content', 'headless', 'two', 'row', 'titled-tap', 'authored', 'own-back', 'pressing', 'commanded', 'popover-first', 'foreign', 'sheet', 'root'])).toEqual([
      ['symbol', true, '', 'Back', null, false], ['inline', true, '', 'Back', null, false], ['titled', true, 'Done', 'Done', null, false],
      ['long', true, 'Back', 'All my conversations', null, false], ['keys', true, '', 'Back', 'Meta+[', false],
      ['disabled', false, null, null, null, false], ['dimmed', false, null, null, null, false],
      ['dimmed-link', false, null, null, null, true], ['content', true, '', 'Back', null, true],
      ['bare', false, null, null, null, true], ['bare-content', false, null, null, null, true], ['headless', false, null, null, null, true],
      ['two', false, null, null, null, true], ['row', false, null, null, null, true], ['titled-tap', true, '', 'Back', null, false],
      ['authored', false, null, null, null, true], ['own-back', false, null, null, null, true], ['pressing', true, '', 'Back', null, false], ['commanded', true, '', 'Back', null, false],
      ['popover-first', true, '', 'Back', null, false], ['foreign', false, null, null, null, true], ['sheet', false, null, null, null, true], ['root', false, null, null, null, true],
    ]);
    // A control the header no longer holds is shown again.
    await page.evaluate(() => { const route = document.getElementById('moved'); route.append(route.querySelector('#back')); project(); });
    expect(await bars(['moved'])).toEqual([['moved', true, '', 'Back', null, true]]);
    // Until input is ready the wasm glue disables every control itself (`aria-busy`, glue.js): a control it disabled is
    // no item, so it stays shown, beside the bar's Back.
    const busy = (on) => page.evaluate((on) => { document.getElementById('exact-root').setAttribute('aria-busy', String(on)); project(); }, on);
    await busy(true);
    expect(await bars(['dimmed'])).toEqual([['dimmed', true, '', 'Back', null, true]]);
    await busy(false);
    expect(await bars(['dimmed'])).toEqual([['dimmed', false, null, null, null, false]]);
    // The bar's Back takes the focus the control cannot; a press of it, by pointer or key, presses the control. (The
    // sheet, which covers the page, goes first.)
    await page.evaluate(() => document.getElementById('sheet').parentElement.remove());
    const back = page.locator('#symbol > header').locator('button[part="back"]');
    await back.click();
    await back.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => [pressed.splice(0), (document.querySelector('#symbol #back').focus(), document.activeElement.id)])).toEqual([['symbol', 'symbol'], '']);
    // The Back is a web app's (LLP 1115.000 D4.14): its arrow and words in the bar's label ink whatever accent the app,
    // the root or a route says, in dark and in increased contrast, and hovered and held, over a different fill for each.
    const inks = () => page.evaluate(() => ['titled', 'route-accent', 'app-accent', 'nested-accent'].map((id) => {
      const back = document.querySelector(`#${id} > header`).shadowRoot.querySelector('button[part="back"]');
      return [getComputedStyle(back).color, getComputedStyle(back.querySelector('path')).stroke, getComputedStyle(back.lastChild).color];
    }));
    const titled = page.locator('#titled > header').locator('button[part="back"]'), look = () => titled.evaluate((b) => [getComputedStyle(b).color, getComputedStyle(b).backgroundColor]);
    for (const media of [{ colorScheme: 'light' }, { colorScheme: 'dark' }, { colorScheme: 'light', contrast: 'more' }]) {
      await page.emulateMedia(media);
      await page.mouse.move(0, 0);
      const ink = await page.evaluate(() => getComputedStyle(document.querySelector('#titled > header')).color), rest = await look();
      await titled.hover();
      const hovered = await look();
      await page.mouse.down();
      const held = await look();
      await page.mouse.up();
      expect([media, await inks(), [rest[0], hovered[0], held[0]], new Set([rest[1], hovered[1], held[1]]).size])
        .toEqual([media, Array(4).fill([ink, ink, ink]), [ink, ink, ink], 3]);
    }
    await page.emulateMedia({ colorScheme: 'light', contrast: 'no-preference' });
    // The arrow is a square of the bar's font, 1em a side.
    expect(await back.locator('svg').evaluate((svg) => { const r = svg.getBoundingClientRect(), em = parseFloat(getComputedStyle(svg).fontSize); return [r.width / em, r.height / em]; })).toEqual([1, 1]);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 30000);

// Exercise the shared input helpers with the browser's real shadow trees, inert ancestry and scroll boxes.
async function withBackPage(engine, run) {
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', () => `<div id="exact-root"><div id="wrapper">${stack('detail', symbol())}</div></div>`)
    .replace('<script type="module" src="./glue.js"></script>', () => `<script type="module">
      import { navigation, inertAncestor } from '/navigation.js';
      import { reveal, viewBox } from '/agent-input.js';
      const root = document.getElementById('exact-root'); navigation.connect(root, () => false, () => {});
      window.project = () => navigation.project(root, () => {}); project();
      Object.assign(window, { reveal, viewBox, inertAncestor, ready: true });
    </script>`);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/') return new Response(html, { headers: { 'content-type': 'text/html' } });
    const file = Bun.file(resolve(ROOT, 'host/web', '.' + path));
    return await file.exists() ? new Response(file) : new Response(null, { status: 404 });
  } });
  let suite;
  try {
    suite = await suiteBrowser(engine);
    const page = await suite.newPage({ viewport: { width: 400, height: 600 } });
    await page.goto(server.url.href);
    await page.waitForFunction(() => window.ready);
    await run(page);
  } finally { await suite?.close(); server.stop(true); }
}

for (const engine of ['chromium', 'firefox', 'webkit']) {
  test(`${engine}: Back under inert ancestry leaves scrolling unchanged`, () => withBackPage(engine, async page => {
    const results = await page.evaluate(() => {
      const wrapper = document.getElementById('wrapper'), control = document.querySelector('#detail #back');
      wrapper.style.marginTop = '2000px';
      const results = [];
      for (const held of [true, false]) {
        if (!held) {
          control.style.visibility = 'hidden';
          control.closest('header').append(document.querySelector('#detail h2').cloneNode(true)); project();
        }
        wrapper.inert = true;
        for (const check of [undefined, inertAncestor]) {
          scrollTo(0, 0);
          const before = scrollY, offscreen = viewBox(control).top > innerHeight;
          results.push([held, offscreen, reveal(control, 'back', check).scrolled, scrollY - before]);
        }
        wrapper.inert = false;
      }
      return results;
    });
    expect(results).toEqual([[true, true, false, 0], [true, true, false, 0], [false, true, false, 0], [false, true, false, 0]]);
  }), 30000);
  test(`${engine}: Back on a covered route retains its visible box`, () => withBackPage(engine, async page => {
    const results = await page.evaluate(() => {
      const route = document.getElementById('detail'), nav = route.parentElement, control = route.querySelector('#back');
      nav.insertAdjacentHTML('beforeend', '<div navigationKey="3" navigationPresentation="modal"><header><h2>Sheet</h2></header></div>');
      nav.setAttribute('navigationKey', '3'); project();
      const results = [];
      for (const held of [true, false]) {
        if (!held) {
          control.style.visibility = 'hidden';
          control.closest('header').append(route.querySelector('h2').cloneNode(true)); project();
        }
        const back = route.querySelector('header').shadowRoot.querySelector('button[part="back"]');
        const visible = back.checkVisibility({ visibilityProperty: true }), box = back.getBoundingClientRect();
        results.push([held, route.inert, control.hasAttribute('data-exact-bar-back'), visible, box.width > 0,
          JSON.stringify(viewBox(control)) === JSON.stringify(box)]);
      }
      return results;
    });
    expect(results).toEqual([[true, true, true, true, true, true], [false, true, false, true, true, true]]);
  }), 30000);

  test(`${engine}: a retained modal skips the covered route Back shortcut`, () => withBackPage(engine, async page => {
    const facts = await page.evaluate(async () => {
      const { createInputHandlers, backButton } = await import('/input-glue.js');
      const root = document.getElementById('exact-root'), route = document.getElementById('detail'), nav = route.parentElement;
      const control = route.querySelector('#back');
      route.insertAdjacentHTML('beforeend', '<dialog><button id="command" aria-keyshortcuts="Alt+B">Command</button></dialog>');
      const dialog = route.querySelector('dialog'), command = dialog.querySelector('button');
      dialog.prepend(control); control.style.display = 'none'; control.setAttribute('aria-keyshortcuts', 'Alt+B');
      dialog.showModal();
      nav.insertAdjacentHTML('beforeend', '<div navigationKey="3" navigationPresentation="modal"><header><h2>Sheet</h2></header></div>');
      nav.setAttribute('navigationKey', '3'); project();
      window.pressed = []; control.onclick = () => pressed.push('back'); command.onclick = () => pressed.push('command');
      createInputHandlers({ root, views: new Map(), retiredViews: new WeakSet(), ready: () => true, inertAncestor, dispatch() {} });
      command.focus();
      return [dialog.matches(':modal'), route.inert, !!inertAncestor(control), command.checkVisibility({ visibilityProperty: true }), viewBox(control).width > 0, !!backButton(control)];
    });
    await page.keyboard.press('Alt+b');
    const pressed = await page.evaluate(() => window.pressed);
    expect([facts, pressed]).toEqual([[true, true, false, true, true, false], ['command']]);
  }), 30000);
}

// Compiled, through each runtime and the agent: a tap that names the control the bar stands for presses the bar's
// Back, which presses the control. A header iOS makes no bar of draws Back when it shows no visible Back control.
// Build the wasm fixture first: EXACT_WEB_LINK=all EXACT_WEB_DIST=<dist> bun host/web/build.mjs caltrain-web --wasm.
// Run this file with EXACT_WEB_DIST=<dist> to exercise both runtimes.
const SOURCE = `routes nav
  home "/"
    detail "/detail"
    full "/full"
    row "/row"
    authored "/authored"
    pressing "/pressing"
    commanded "/commanded"
    popfirst "/popfirst"
    wraps "/wraps"
    kept "/kept"
    hidden "/hidden"
    hiddenNone "/hidden-none"
    contentHidden "/content-hidden"
    contentNone "/content-none"

component App
  state added = 0
  action add
    added = added + 1
  action open
    nav = push(nav, path("detail"))
  action openFull
    nav = push(nav, path("full"))
  action openRow
    nav = push(nav, path("row"))
  action openAuthored
    nav = push(nav, path("authored"))
  action openPressing
    nav = push(nav, path("pressing"))
  action openCommanded
    nav = push(nav, path("commanded"))
  action openPopfirst
    nav = push(nav, path("popfirst"))
  action openWraps
    nav = push(nav, path("wraps"))
  action openHidden
    nav = push(nav, path("hidden"))
  action openHiddenNone
    nav = push(nav, path("hiddenNone"))
  action openContentHidden
    nav = push(nav, path("contentHidden"))
  action openContentNone
    nav = push(nav, path("contentNone"))
  action openKept
    nav = push(nav, path("kept"))
  action back
    nav = back(nav)
  action noop
    nav = nav
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" testId="nav" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` navigationBar=(e.name == "full" ? "none" : "auto") position="absolute" inset=0 background-color="Canvas"
          if e.name == "wraps"
            header display="flex" align-items="center"
              button id="back" press=back testId="back" aria-label="Back"
                image "symbol:back" font-size=17 width=20 height=20
                text "Updates" role="heading" aria-level=1
          else if e.name == "kept"
            header display="flex" align-items="center" appearance="none"
              text "Updates" role="heading" aria-level=1
              button press=add testId="add" aria-label="Add"
                image "symbol:add" font-size=13
            button id="back" press=back testId="back" aria-label="Back" width=44 height=44
              image "symbol:back" font-size=17 width=20 height=20
          else if e.name == "pressing"
            header display="flex" align-items="center" press=noop
              button id="back" press=back testId="back" aria-label="Back" width=44 height=44
                image "symbol:back" font-size=17 width=20 height=20
              text "Updates" role="heading" aria-level=1
          else
            if e.name == "authored"
              header display="flex" align-items="center" appearance="none"
                button id="back" press=back testId="back" aria-label="Back" width=44 height=44
                  image "symbol:back" font-size=17 width=20 height=20
                text "Updates" role="heading" aria-level=1
                button press=add testId="add" aria-label="Add"
                  image "symbol:add" font-size=13
            else
              header display="flex" align-items="center"
                if e.name != "home" and e.name != "contentHidden" and e.name != "contentNone"
                  button id="back" press=back testId="back" aria-label="Back" width=44 height=44 visibility=(e.name == "hidden" ? "hidden" : "visible") display=(e.name == "hiddenNone" ? "none" : "flex") aria-keyshortcuts="Alt+B"
                    image "symbol:back" font-size=17 width=20 height=20
                if e.name == "hidden" or e.name == "hiddenNone"
                  text "Today" role="heading" aria-level=2
                if e.name == "row"
                  row role="heading" aria-level=1
                    text "Updates"
                else
                  if e.name == "commanded"
                    button commandfor="sheet" command="show-modal"
                      text "Updates" role="heading" aria-level=1
                  else
                    if e.name == "popfirst"
                      column popover="auto" id="menu"
                        button press=noop
                          text "Menu" role="heading" aria-level=2
                      button press=noop
                        text "Updates" role="heading" aria-level=1
                    else
                      text (e.name == "home" ? "Home" : "Updates") role="heading" aria-level=1
                if e.name != "home"
                  button press=add testId="add" aria-label="Add"
                    image "symbol:add" font-size=13
          if e.name == "contentHidden" or e.name == "contentNone"
            button id="back" press=back testId="back" aria-label="Back" width=44 height=44 visibility=(e.name == "contentHidden" ? "hidden" : "visible") display=(e.name == "contentNone" ? "none" : "flex") aria-keyshortcuts="Alt+B"
              image "symbol:back" font-size=17 width=20 height=20
          if e.name == "commanded"
            dialog id="sheet"
              text "Sheet"
          if e.name == "home"
            button press=open testId="open"
              text "Open"
            button press=openFull testId="open-full"
              text "Full"
            button press=openRow testId="open-row"
              text "Row"
            button press=openAuthored testId="open-authored"
              text "Authored"
            button press=openPressing testId="open-pressing"
              text "Pressing"
            button press=openCommanded testId="open-commanded"
              text "Commanded"
            button press=openPopfirst testId="open-popfirst"
              text "Popover first"
            button press=openWraps testId="open-wraps"
              text "Wraps"
            button press=openHidden testId="open-hidden"
              text "Hidden"
            button press=openHiddenNone testId="open-hidden-none"
              text "Hidden without a box"
            button press=openContentHidden testId="open-content-hidden"
              text "Hidden in content"
            button press=openContentNone testId="open-content-none"
              text "Content without a box"
            button press=openKept testId="open-kept"
              text "Kept"
`;
for (const [runtime, webDist] of Object.entries({ wasm: process.env.EXACT_WEB_DIST, JS: true })) {
  const check = need('fixture', webDist ? null : 'no wasm web dist: name one with EXACT_WEB_DIST');
  check(`${runtime}: a tap on the Back control goes back through the bar's Back, and a route with no bar keeps its control`, async () => {
    const dir = mkdtempSync(resolve(tmpdir(), 'exact-bar-back-'));
    let server;
    try {
      const plan = resolve(dir, 'app.plan');
      writeFileSync(resolve(dir, 'app.contract'), SOURCE);
      const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(dir, 'app.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
      expect(built.status, built.stderr).toBe(0);
      let options = { plan, webDist: runtime === 'wasm' ? resolve(webDist) : undefined };
      if (runtime === 'JS') {
        const out = resolve(dir, 'dist'), build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', out, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
        expect(build.status, build.stderr).toBe(0);
        server = createServer((req, res) => serveBuildTree(out, req, res));
        await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
        options = { url: `http://127.0.0.1:${server.address().port}/` };
      }
      for (const browser of ['chrome', 'firefox', 'webkit']) {
        const s = await open({ host: 'web', browser, ...options });
        try {
          await s.tap('open');
          await s.clock('settle');
          // The routes, the bar's Back's words (null for none) and name, and whether the control shows.
          const shown = () => s.carrier.evaluate(`(() => { const routes = [...document.querySelectorAll('main > [navigationkey]')], back = routes.at(-1).querySelector(':scope > header').shadowRoot?.querySelector('button'), drawn = !!back && !back.hidden; return [routes.length, drawn ? back.lastChild.textContent : null, drawn ? back.getAttribute('aria-label') : null, document.querySelector('[data-testid=back]')?.getClientRects().length > 0]; })()`);
          expect([browser, await shown()]).toEqual([browser, [2, '', 'Back', false]]);
          // The glue marks the bar (LLP 1115.000 D4.15), whose symbol is the bar's size, 22/17 of the 16 px body in its
          // 1.2em box, over the 13 px written on it; a header iOS makes no bar of keeps the written size.
          const barOf = () => s.carrier.evaluate(`(() => { const route = [...document.querySelectorAll('main > [navigationkey]')].at(-1), box = route.querySelector('[data-testid=add] img').getBoundingClientRect(); return [route.querySelector(':scope > header').hasAttribute('data-exact-bar'), Math.round(box.width * 10) / 10]; })()`);
          const [marked, width] = await barOf();
          expect([browser, marked, Math.abs(width - 16 * 22 / 17 * 1.2) <= 0.06]).toEqual([browser, true, true]);
          await s.tap('back');
          await s.clock('settle');
          expect([browser, (await shown())[0]]).toEqual([browser, 1]);
          const hiddenRoutes = ['open-hidden', 'open-hidden-none', 'open-content-hidden', 'open-content-none'], results = [];
          for (const route of hiddenRoutes) for (const input of ['tap', 'shortcut']) {
            await s.tap(route);
            await s.clock('settle');
            expect([browser, route, await s.carrier.evaluate(`(() => {
              const control = document.querySelector('[data-testid=back]'), back = control.closest('[navigationkey]').querySelector(':scope > header').shadowRoot?.querySelector('button[part="back"]');
              return [control.checkVisibility({ visibilityProperty: true }), control.hasAttribute('data-exact-bar-back'), back?.checkVisibility({ visibilityProperty: true }) ?? false, back?.getAttribute('aria-keyshortcuts')];
            })()`)]).toEqual([browser, route, [false, false, true, 'Alt+B']]);
            let error = null;
            try { if (input === 'tap') await s.tap('back'); else await s.type('nav', { key: 'Alt+b' }); }
            catch (e) { error = e.message; }
            await s.clock('settle');
            const count = (await shown())[0];
            results.push([route, input, count, error]);
            // Recover a failed input through the platform's Back so every case reports its result.
            if (count > 1) { await s.back(); await s.clock('settle'); }
          }
          expect([browser, results]).toEqual([browser, hiddenRoutes.flatMap(route => ['tap', 'shortcut'].map(input => [route, input, 1, null]))]);
          // A route with no bar, one whose header's heading is a row and one whose header says `appearance: none`
          // (holding the control or not), which iOS makes no bar of: the control shows, no Back is drawn, and the
          // control goes back. A header that hears `press`, a title in a button that shows a dialog, and a popover
          // before the title make a bar, as the kernel's walk has it: the control is the bar's Back's, which goes back.
          // A control that is the title's own pressable is the title, beside the Back it gives its words.
          const none = [2, null, null, true], bar = [2, '', 'Back', false];
          for (const [route, expected] of [['open-full', none], ['open-row', none], ['open-authored', none], ['open-kept', none], ['open-pressing', bar], ['open-commanded', bar], ['open-popfirst', bar], ['open-wraps', [2, 'Updates', 'Back', true]]]) {
            await s.tap(route);
            await s.clock('settle');
            expect([browser, route, await shown()]).toEqual([browser, route, expected]);
            // A header iOS makes no bar of keeps the sizes written on its symbols (LLP 1115.000 D4.15): the Back
            // control's 20 px box, and Add's 13 px in a native button face's 1.5em box.
            if (expected === none) expect([browser, route, await s.carrier.evaluate(`(r => [r.width, r.height])(document.querySelector('[data-testid=back] img').getBoundingClientRect())`), await barOf()]).toEqual([browser, route, [20, 20], [false, 13 * 1.5]]);
            await s.tap('back');
            await s.clock('settle');
            expect([browser, route, (await shown())[0]]).toEqual([browser, route, 1]);
          }
        } finally { await s.close(); }
      }
    } finally {
      server?.close();
      rmSync(dir, { recursive: true, force: true });
    }
  }, 600000);
}
