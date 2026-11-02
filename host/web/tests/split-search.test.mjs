// The navigation split's list search (navigation.js `pushingList`, `splitOf`): found from the route's lists rather
// than by walking every element of the route on every projection. Held, in Chrome, to answer exactly what the walk
// answered: the walk is kept here as the reference, and both run over seeded random routes (lists sideways by class,
// by inline style and by an estimated item width, nested lists under taken and under sideways lists, undisplayed
// ancestors by class and inline, popovers, leaving elements, `navigationKey` sub-routes, rows whose press and href say
// push or not, a retained list left with no rows), at a phone's width and a desktop's; over a live page changed in each
// way the answer reads (a list added and removed, a class that lays a list sideways, an ancestor hidden, the colour
// scheme, a sheet swapped, the route replaced, a push and a pop, the retained list emptied); and in the scope the walk
// had (`children`: the light DOM, slotted elements included, not a shadow root's or a frame's).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

const page = `<!doctype html><style>
  .row { display: flex; flex-direction: row } .col { display: flex; flex-direction: column } .gone { display: none }
  @media (prefers-color-scheme: dark) { .flip { display: flex; flex-direction: row } }
  [navigationKey] { position: absolute; inset: 0 }
</style><div id="exact-root"><main id="nav" navigationKey="1" navigationBack="back" style="position:relative;width:100%;height:100%"></main></div>
<script type="module">
  import { navigation, pushingList, splitOf } from './navigation.js';
  // The router's matcher, with a hook a test sets to change the page while the search runs (production's is pure: router.js routeMatches, glue.js's exact_route_match).
  const linkedPaths = href => { window.onLinked?.(href); return href.startsWith('/in/') && href !== '/in/hook'; };
  navigation.connect(document.getElementById('exact-root'), () => false, () => {}, null, null, linkedPaths);
  // The walk navigation.js had before the search, verbatim but for its inputs (retained, linked) passed in.
  const excluded = el => el.hasAttribute('popover') || el.hasAttribute('data-exiting') || getComputedStyle(el).display === 'none';
  const linked = href => href?.startsWith('/') && !href.startsWith('//') && linkedPaths(href);
  function listRows(list) {
    const rows = [], walk = parent => { for (const el of parent.children) { if (excluded(el)) continue; const press = el.getAttribute('data-exact-router-press'), href = el.getAttribute('href') || null; if (press !== null || href) rows.push({ el, press, href }); else walk(el); } };
    walk(list); return rows;
  }
  function walkedList(route, retained) {
    const stack = [...route.children].reverse();
    let empty = null;
    while (stack.length) {
      const el = stack.pop();
      if (el.hasAttribute('navigationKey') || excluded(el)) continue;
      const style = getComputedStyle(el), sideways = /flex/.test(style.display) && style.flexDirection.startsWith('row')
        || Number(el.getAttribute('data-exact-estimated-item-width')) > 0;
      if (el.hasAttribute('data-exact-list') && !sideways) {
        const rows = listRows(el), found = { list: el, rows };
        if (rows.some(r => r.press === 'true' || r.press === null && linked(r.href))) return found;
        if (el === retained && !rows.length) empty = found;
        continue;
      }
      stack.push(...[...el.children].reverse());
    }
    return empty;
  }
  const refLists = new WeakMap(), wide = () => matchMedia('(min-width: 900px)').matches, sheet = r => ['modal', 'fullscreen'].includes(r?.getAttribute('navigationPresentation'));
  function walkedSplit([route]) {
    const said = route?.getAttribute('navigationSplit'), explicit = Number(said);
    const found = route && said !== 'none' && !sheet(route) ? walkedList(route, refLists.get(route)) : null;
    const width = route && said !== 'none' && !sheet(route) && wide() && /^(absolute|fixed)$/.test(getComputedStyle(route).position)
      ? Number.isFinite(explicit) && explicit > 0 ? explicit : found ? 320 : 0 : 0;
    if (route) { if (found && (width || refLists.has(route))) refLists.set(route, found.list); else refLists.delete(route); }
    return { width, list: found?.list, rows: found?.rows ?? [] };
  }
  // Answers as comparable values: elements by their test ids.
  const id = el => el ? el.dataset.t : null;
  const shape = s => s && { list: id(s.list), width: s.width, rows: s.rows.map(r => [id(r.el), r.press, r.href]) };
  const found = f => f && { list: id(f.list), rows: f.rows.map(r => [id(r.el), r.press, r.href]) };
  window.compare = routes => [shape(splitOf(routes)), shape(walkedSplit(routes))];
  window.compareList = (route, retained) => [found(pushingList(route, retained)), found(walkedList(route, retained))];
  compareList.walkOnly = (route, retained = null) => found(walkedList(route, retained));
  window.answeredBy = route => pushingList(route, null)?.list ?? null;
  // mulberry32, its full 32 bits.
  const rng = seed => () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  let tid = 0; // unique test ids across routes; each route has its own element budget
  /** A random route of up to \`size\` elements: lists (some sideways three ways, some nested), rows, every way to be unreachable. */
  window.grow = (seed, size) => {
    const r = rng(seed), pick = a => a[Math.floor(r() * a.length)], nav = document.getElementById('nav');
    nav.replaceChildren();
    const route = document.createElement('div'); route.setAttribute('navigationKey', '1'); route.dataset.t = 'route'; nav.append(route);
    const lists = [], opened = []; let made = 0;
    const add = (parent, depth) => {
      if (depth > 7 || made >= size) return;
      for (let n = 1 + Math.floor(r() * 4); n-- > 0 && made < size;) {
        const x = r(), el = document.createElement(x < 0.25 ? 'a' : x < 0.45 ? 'button' : 'div'); el.dataset.t = 't' + ++tid; made++;
        if (r() < 0.22) { el.setAttribute('data-exact-list', ''); lists.push(el);
          const s = r(); if (s < 0.12) el.className = 'row'; else if (s < 0.22) el.style.cssText = 'display:flex;flex-direction:row'; else if (s < 0.3) el.setAttribute('data-exact-estimated-item-width', pick(['40', '0', '-3', 'x'])); else if (s < 0.4) el.className = 'col'; }
        if (r() < 0.06) el.className += ' gone'; if (r() < 0.04) el.style.display = 'none';
        if (r() < 0.04) { el.setAttribute('popover', ''); if (r() < 0.6) opened.push(el); } if (r() < 0.04) el.setAttribute('data-exiting', ''); if (r() < 0.04) el.setAttribute('navigationKey', String(2 + Math.floor(r() * 9)));
        if (r() < 0.35) el.setAttribute('data-exact-router-press', pick(['true', 'false', 'true']));
        if (el.localName === 'a' && r() < 0.8) el.setAttribute('href', pick(['/in/a', '/in/b', '/out', '//in/x', '']));
        el.textContent = r() < 0.3 ? 'x' : '';
        parent.append(el);
        if (r() < 0.75) add(el, depth + 1);
      }
    };
    add(route, 0);
    // Some popovers shown (a closed one is undisplayed by the UA sheet anyway): their attribute alone excludes them.
    for (const p of opened) try { p.showPopover(); } catch {}
    return { route, lists, made };
  };
  /** Which way each candidate list was decided, as the walk would decide it (coverage of the generator, not the answer):
   * unreachable by its own or an ancestor's navigationKey, popover, data-exiting, class or inline display:none; owned by a
   * taken list above it; skipped as sideways by class, by inline style or by an estimated width; or its rows read. Counted
   * only for the candidates the search reached (to the one it answered), so a count is a decision the search took. */
  window.branches = (route, into, answered = null) => { // up to the list the search answered: the decisions it took
    const why = el => el.hasAttribute('navigationKey') ? 'navigationKey' : el.matches(':popover-open') ? 'popover' : el.hasAttribute('popover') ? 'closedPopover' : el.hasAttribute('data-exiting') ? 'exiting'
      : getComputedStyle(el).display === 'none' ? (el.style.display === 'none' ? 'inlineNone' : 'classNone') : null;
    const across = el => { const s = getComputedStyle(el); return /flex/.test(s.display) && s.flexDirection.startsWith('row') ? (el.style.flexDirection ? 'inlineRow' : 'classRow') : Number(el.getAttribute('data-exact-estimated-item-width')) > 0 ? 'estimatedWidth' : null; };
    for (const el of route.querySelectorAll('[data-exact-list]')) {
      let reason = why(el) && 'self:' + why(el), underSideways = false;
      for (let up = el.parentElement; !reason && up !== route; up = up.parentElement) {
        if (why(up)) reason = 'ancestor:' + why(up);
        else if (up.hasAttribute('data-exact-list')) { if (across(up)) underSideways = true; else reason = 'owned'; }
      }
      reason ??= across(el) ? 'sideways:' + across(el) : 'read';
      into[reason] = (into[reason] ?? 0) + 1;
      if (underSideways && reason === 'read') into.readUnderSideways = (into.readUnderSideways ?? 0) + 1;
      if (el === answered) break;
    }
  };
  window.ready = true;
</script>`;

async function inChrome(test, w = 1280) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    if (req.url === '/frame') { res.writeHead(200, { 'content-type': 'text/html' }); res.end('<div data-exact-list data-t="f-list"><a href="/in/f" data-exact-router-press="true" data-t="f-row">f</a></div>'); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=1400,900'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    const size = async (width) => {
      await call('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
      await evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    };
    await size(w);
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    await test(evaluate, size, call);
  } finally {
    await suite.close();
    server.close();
  }
}

check('seeded random routes: the search answers what the walk answered, at a phone width and a desktop width', () => inChrome(async (evaluate, size) => {
  for (const width of [390, 1280]) {
    await size(width);
    const tally = await evaluate(`(() => {
      const t = { trees: 0, found: 0, emptyRetained: 0, none: 0, wide: 0, empty: 0, branches: {}, mismatches: [] };
      for (let seed = 1; seed <= 400; seed++) {
        const { route, lists, made } = grow(seed * 7919 + ${width}, 40 + (seed % 7) * 40);
        if (!made) t.empty++;
        branches(route, t.branches, answeredBy(route));
        // The split twice (the second reads what the first retained), then the list with each list retained in turn.
        for (let k = 0; k < 2; k++) { const [a, b] = compare([route]); if (JSON.stringify(a) !== JSON.stringify(b)) t.mismatches.push({ seed, k, a, b }); if (a.width) t.wide++; }
        for (const retained of [null, ...lists.slice(0, 6)]) {
          // A retained list whose rows are gone: its rows emptied for this comparison.
          const [a, b] = compareList(route, retained);
          if (JSON.stringify(a) !== JSON.stringify(b)) t.mismatches.push({ seed, retained: retained?.dataset.t, a, b });
          if (a?.rows.length) t.found++; else if (a) t.emptyRetained++; else t.none++;
        }
        for (const list of lists.slice(0, 3)) {
          const kept = [...list.children]; list.replaceChildren();
          const [a, b] = compareList(route, list);
          if (JSON.stringify(a) !== JSON.stringify(b)) t.mismatches.push({ seed, emptied: list.dataset.t, a, b });
          if (a && !a.rows.length) t.emptyRetained++;
          list.replaceChildren(...kept);
        }
        t.trees++;
      }
      return t;
    })()`);
    expect(tally.mismatches.slice(0, 3)).toEqual([]);
    expect(tally.trees).toBe(400);
    // The generator reached every kind of answer: a list found, the retained list with no rows, none.
    expect(tally.found).toBeGreaterThan(200);
    expect(tally.emptyRetained).toBeGreaterThan(50);
    expect(tally.none).toBeGreaterThan(50);
    // Every route had elements, and every way a candidate is decided came up (at each width).
    expect(tally.empty).toBe(0);
    for (const b of ['self:navigationKey', 'self:popover', 'self:exiting', 'self:classNone', 'self:inlineNone', 'ancestor:navigationKey', 'ancestor:popover', 'ancestor:exiting',
      'ancestor:classNone', 'ancestor:inlineNone', 'owned', 'sideways:classRow', 'sideways:inlineRow', 'sideways:estimatedWidth', 'read', 'readUnderSideways']) expect([b, (tally.branches[b] ?? 0) > 0]).toEqual([b, true]);
    if (width === 1280) expect(tally.wide).toBeGreaterThan(50); else expect(tally.wide).toBe(0);
  }
}), 120000);

check('a live page changed in each way the answer reads: the search answers what the walk answered after each', () => inChrome(async (evaluate, size, call) => {
  await evaluate(`(() => {
    const nav = document.getElementById('nav');
    nav.innerHTML = '<div navigationKey="1" data-t="r1"><header data-t="h">x</header><div data-t="box"><div data-exact-list data-t="L"><a href="/in/a" data-t="a">a</a><button data-exact-router-press="true" data-t="b">b</button></div></div></div><div navigationKey="2" data-t="r2"><div data-t="d">d</div></div>';
    window.routes = () => [...nav.children];
    window.both = () => compare(routes());
  })()`);
  const same = async (what) => { const [a, b] = await evaluate('both()'); expect([what, a]).toEqual([what, b]); return a; };
  await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  expect((await same('first')).list).toBe('L');
  await evaluate(`document.querySelector('[data-t="L"]').remove()`); expect((await same('a list removed')).list).toBe(null);
  await evaluate(`document.querySelector('[data-t="box"]').insertAdjacentHTML('beforeend', '<div data-exact-list data-t="L2"><a href="/in/c" data-t="c">c</a></div>')`); expect((await same('a list added')).list).toBe('L2');
  await evaluate(`document.querySelector('[data-t="L2"]').className = 'row'`); expect((await same('a class lays it sideways')).list).toBe(null);
  await evaluate(`document.querySelector('[data-t="L2"]').className = 'flip'`); expect((await same('light scheme: not sideways')).list).toBe('L2');
  await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
  expect((await same('dark scheme: sideways')).list).toBe(null);
  await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  expect((await same('back to light')).list).toBe('L2');
  await evaluate(`document.querySelector('[data-t="box"]').classList.add('gone')`); expect((await same('an ancestor hidden by class')).list).toBe(null);
  await evaluate(`document.querySelector('[data-t="box"]').classList.remove('gone')`);
  await evaluate(`document.head.insertAdjacentHTML('beforeend', '<style id="swap">[data-t="box"] { display: none }</style>')`); expect((await same('a sheet that hides it')).list).toBe(null);
  await evaluate(`document.getElementById('swap').remove()`); expect((await same('the sheet swapped back')).list).toBe('L2');
  await evaluate(`document.getElementById('nav').insertAdjacentHTML('beforeend', '<div navigationKey="3" data-t="r3"></div>')`); expect((await same('a later route pushed: the root\'s answer unaffected')).list).toBe('L2');
  await evaluate(`document.querySelector('[data-t="r3"]').remove()`); expect((await same('the later route popped: the root\'s answer unaffected')).list).toBe('L2');
  await evaluate(`(() => { const old = document.querySelector('[data-t="r1"]'), next = document.createElement('div'); next.setAttribute('navigationKey', '1'); next.dataset.t = 'r1b'; next.innerHTML = '<div data-exact-list data-t="M"><a href="/in/m" data-t="m">m</a></div>'; old.replaceWith(next); })()`);
  expect((await same('the route replaced')).list).toBe('M');
  // Retained on a wide window (a split showing), then its rows gone: the retained list with none, both ways.
  await size(1280); expect((await same('retained wide')).width).toBe(320);
  await evaluate(`document.querySelector('[data-t="M"]').replaceChildren()`); const empty = await same('the retained list emptied');
  expect([empty.list, empty.rows]).toEqual(['M', []]);
  await size(390); await same('narrow');
}), 60000);

check('the scope the walk had: light DOM and slotted elements, never a shadow root or a frame', () => inChrome(async (evaluate) => {
  const answers = await evaluate(`(async () => {
    const nav = document.getElementById('nav');
    const route = document.createElement('div'); route.setAttribute('navigationKey', '1'); route.dataset.t = 'route'; nav.replaceChildren(route);
    // A list in a shadow root: neither sees it.
    const host = document.createElement('div'); host.dataset.t = 'host'; route.append(host);
    host.attachShadow({ mode: 'open' }).innerHTML = '<div data-exact-list data-t="s-list"><a href="/in/s" data-exact-router-press="true" data-t="s-row">s</a></div><slot></slot>';
    const inShadow = compare([route]);
    // A same-origin frame's list: neither sees it.
    const frame = document.createElement('iframe'); frame.src = '/frame'; route.append(frame); await new Promise(r => frame.onload = r);
    if (!frame.contentDocument.querySelector('[data-exact-list]')) throw new Error('the frame has no list to miss');
    const inFrame = compare([route]);
    // A light child of the shadow host, slotted into it: both see it (it is the route's light DOM).
    host.insertAdjacentHTML('beforeend', '<div data-exact-list data-t="slotted"><a href="/in/l" data-t="l-row">l</a></div>');
    const slotted = compare([route]);
    return { inShadow, inFrame, slotted };
  })()`);
  for (const [what, [search, walked]] of Object.entries(answers)) expect([what, search]).toEqual([what, walked]);
  expect([answers.inShadow[0].list, answers.inFrame[0].list, answers.slotted[0].list]).toEqual([null, null, 'slotted']);
}), 60000);

// Each way a candidate is decided, alone, where it changes the answer: the first list is passed over for the reason
// named (or not), so the answer is the second (or the first). Both the search and the walk must give the answer named.
check('each branch decides the answer in a fixed case', () => inChrome(async (evaluate) => {
  const row = (t, press = '') => `<a href="/in/${t}" data-t="${t}-row" ${press}>${t}</a>`, list = (t, attrs = '', inner = row(t)) => `<div data-exact-list data-t="${t}" ${attrs}>${inner}</div>`;
  const cases = [
    ['an ancestor navigationKey', `<div navigationKey="9">${list('A')}</div>${list('B')}`, 'B'],
    ['an ancestor popover, shown', `<div popover>${list('A')}</div>${list('B')}`, 'B'],
    ['an ancestor data-exiting', `<div data-exiting>${list('A')}</div>${list('B')}`, 'B'],
    ['an ancestor hidden by class', `<div class="gone">${list('A')}</div>${list('B')}`, 'B'],
    ['an ancestor hidden inline', `<div style="display:none">${list('A')}</div>${list('B')}`, 'B'],
    ['itself navigationKey', `${list('A', 'navigationKey="9"')}${list('B')}`, 'B'],
    ['itself a popover, shown', `${list('A', 'popover')}${list('B')}`, 'B'],
    ['itself data-exiting', `${list('A', 'data-exiting')}${list('B')}`, 'B'],
    ['itself hidden by class', `${list('A', 'class="gone"')}${list('B')}`, 'B'],
    ['itself hidden inline', `${list('A', 'style="display:none"')}${list('B')}`, 'B'],
    // A list taken whose rows don't push owns the list inside it (a row of it, pressed false): the next list answers.
    ['owned by a list taken', `${list('A', '', list('N', 'data-exact-router-press="false"'))}${list('B')}`, 'B'],
    ['sideways by class', `${list('A', 'class="row"')}${list('B')}`, 'B'],
    ['sideways inline', `${list('A', 'style="display:flex;flex-direction:row"')}${list('B')}`, 'B'],
    ['sideways by an estimated width', `${list('A', 'data-exact-estimated-item-width="40"')}${list('B')}`, 'B'],
    ['not sideways: an estimated width of 0', `${list('A', 'data-exact-estimated-item-width="0"')}${list('B')}`, 'A'],
    ['a list under a sideways list is still a candidate', `${list('S', 'class="row"', list('N'))}${list('B')}`, 'N'],
    ['rows that do not push: passed over', `${list('A', '', '<a href="/out" data-t="o">o</a>')}${list('B')}`, 'B'],
    ['a row pressed true pushes', `${list('A', '', '<button data-exact-router-press="true" data-t="p">p</button>')}${list('B')}`, 'A'],
  ];
  const results = await evaluate(`(() => {
    const out = [], nav = document.getElementById('nav');
    for (const [what, html] of ${JSON.stringify(cases.map(([w, h]) => [w, h]))}) {
      nav.innerHTML = '<div navigationKey="1" data-t="route">' + html + '</div>';
      // A popover shown, so its attribute, not the UA sheet's display:none for a closed one, is what excludes it.
      for (const p of nav.querySelectorAll('[popover]')) { p.showPopover(); if (getComputedStyle(p).display === 'none') throw new Error(what + ': the popover is not displayed'); }
      const route = nav.firstElementChild, [a, b] = compareList(route, null);
      out.push([what, a?.list ?? null, b?.list ?? null]);
    }
    // The retained list with no rows: the fallback.
    nav.innerHTML = '<div navigationKey="1" data-t="route"><div data-exact-list data-t="E"></div></div>';
    const route = nav.firstElementChild, [a, b] = compareList(route, route.firstElementChild);
    out.push(['the retained list with no rows', a?.list ?? null, b?.list ?? null]);
    return out;
  })()`);
  const expected = [...cases.map(([what, , answer]) => [what, answer, answer]), ['the retained list with no rows', 'E', 'E']];
  expect(results).toEqual(expected);
}), 60000);

// The search reads the lists once. If the page changes while it runs (a matcher that edits it, which production's
// pure matchers never do), a list taken out is skipped: here, a nested list taken out before the search reaches it,
// which the walk doesn't reach either. (A direct child of the route taken out may differ: the walk, which had it on its
// stack, would answer the detached element, where the search goes on to a later live list. That is outside the
// guarantee, a DOM the search leaves alone.)
check('a list removed while the search runs is not answered', () => inChrome(async (evaluate) => {
  const answers = await evaluate(`(() => {
    const nav = document.getElementById('nav'), html = '<div navigationKey="1" data-t="route"><div data-exact-list data-t="A"><a href="/in/hook" data-t="h">h</a></div><div data-t="C"><div data-exact-list data-t="B"><a href="/in/b" data-exact-router-press="true" data-t="b">b</a></div></div></div>';
    const run = which => {
      nav.innerHTML = html; const route = nav.firstElementChild;
      window.onLinked = href => { if (href === '/in/hook') document.querySelector('[data-t="B"]')?.remove(); };
      try { return which(route); } finally { window.onLinked = null; }
    };
    const searched = run(route => compareList(route, null)[0]);
    const walked = run(route => compareList.walkOnly(route));
    nav.innerHTML = html; const untouched = compareList(nav.firstElementChild, null);
    return { searched: searched?.list ?? null, walked: walked?.list ?? null, untouched: untouched.map(x => x?.list ?? null) };
  })()`);
  // Unchanged, B answers both ways; with B taken out mid-search, neither answers it.
  expect(answers).toEqual({ searched: null, walked: null, untouched: ['B', 'B'] });
}), 60000);
