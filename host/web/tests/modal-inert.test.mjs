import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { glueDeclarations } from './glue-fixture.mjs';

const glue = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
const gpuListen = glueDeclarations(readFileSync(new URL('../gpu-glue.js', import.meta.url), 'utf8'), ['listen']);
// Execute the wasm host's actual entry points with only the runner and unrelated
// presenters stubbed. The browser owns dialog, popover, inert and scrolling behavior.
const shown = glue.split('exact_geometry: { shown: ')[1].split(', read: ')[0];
const agentStart = glue.indexOf('function agentReply(');
const agent = glue.slice(agentStart, glue.indexOf('\n}', agentStart) + 2);
const props = glue.slice(glue.indexOf('function applyProps('), glue.indexOf('function ensureMessageListener('));
const html = `<style>dialog { max-height: 240px; overflow: auto; }</style>
<main id="exact-root">
  <button id="page" data-match>Page</button>
  <div id="ancestor">
    <dialog id="upper" data-match>
      <button id="inside" data-match>Inside</button>
      <div inert><button id="inert-child" data-match>Inert</button></div>
      <div style="display:none"><button id="hidden" data-match>Hidden</button></div>
      <div id="inner-pop" popover="manual" data-match>Menu</div>
      <div style="height:1200px"></div>
      <button id="below" data-match>Below</button>
    </dialog>
  </div>
  <dialog id="lower" data-match>Lower</dialog>
  <div id="outer-pop" popover="manual" data-match>Outer menu</div>
  <div inert><dialog id="popover-dialog" popover="manual" data-match>Inert popover</dialog></div>
</main>
<script type="module">
  import * as nav from '/navigation.js';
  import { reveal } from '/input-glue.js';
  import '/document-glue.js';
  const { inertAncestor, modalLayers, contextAt, editMenu, liveById } = nav;
  const root = document.getElementById('exact-root');
  const views = new Map([...root.querySelectorAll('[id]')].map(el => [el.id, el]));
  const wasm = {}, inputReady = true, tagged = reply => reply, syncMedia = () => {}, syncMarkup = () => {};
  ${agent}
  ${props}
  let structuralModalLayers;
  const shown = ${shown};
  window.read = () => [...root.querySelectorAll('[data-match]')].filter((el, index) => shown(el.id, index === 0)).map(el => el.id);
  window.readCandidates = count => {
    const candidates = [...root.querySelectorAll('[data-candidate]')].slice(0, count);
    return candidates.filter((el, index) => shown(el.id, index === 0)).map(el => el.id);
  };
  window.addCandidates = (parent, count) => {
    for (let i = 0; i < count; i++) {
      const el = document.createElement('div'); el.id = 'candidate-' + i;
      el.setAttribute('data-candidate', ''); parent.append(el); views.set(el.id, el);
    }
  };
  window.agent = agentReply;
  applyProps(views.get('inside'), { contextpopover: 'inner-pop' }, []);
  window.listenGPU = (host, canvas) => {
    const entry = { host, el: canvas, view: 1, id: 1 }, events = [];
    const live = () => entry, recoveringDevice = null, exact = { now: () => 0 };
    const gpu = { gpu_input: (_, json) => { events.push(JSON.parse(json)); return true; } };
    const messages = () => {}, schedule = () => {};
    ${gpuListen}
    listen(entry);
    return { events, stop: () => entry.unlisten() };
  };
  window.ready = true;
</script>`;

async function inBrowser(run) {
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/favicon.ico') return new Response(null, { status: 204 });
    return path === '/' ? new Response(html, { headers: { 'content-type': 'text/html' } })
      : new Response(Bun.file(new URL('..' + path, import.meta.url)));
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const failed = new Promise((_, reject) => page.on('pageerror', reject));
    await Promise.race([page.goto(server.url.href).then(() => page.waitForFunction(() => window.ready)), failed]);
    await run(page);
  } finally { await browser?.close(); server.stop(true); }
}

for (const stage of ['capture', 'replay']) test(`document modal input escapes inert ancestry at ${stage}`, () => inBrowser(async page => {
  const results = await page.evaluate(async stage => {
    const results = {};
    for (const state of ['escaped', 'child inert', 'modal inert', 'disabled', 'disabled ancestor', 'nonmodal']) {
      const root = document.createElement('div');
      root.innerHTML = `<div><dialog><section><button data-view="1" data-exact-on="press">Save</button>
        <input data-view="2" data-exact-on="input"></section></dialog></div>`;
      document.body.append(root);
      const ancestor = root.firstElementChild, modal = root.querySelector('dialog'), group = root.querySelector('section');
      const button = root.querySelector('button'), input = root.querySelector('input');
      modal.showModal();
      const checkpoint = document.createElement('script');
      checkpoint.type = 'application/vnd.exact.checkpoint'; checkpoint.dataset.activate = 'interaction';
      document.head.append(checkpoint);
      const events = [], logs = [], views = new Map([[1, button], [2, input]]);
      for (const el of views.values()) el.exactHandlers = el.dataset.exactOn.split(' ');
      const capture = exact.documentBoot({ root, views, dispatch: (...args) => events.push(args.slice(0, 3)), log: line => logs.push(line) });
      checkpoint.remove();
      let started = false;
      capture.started.then(() => { started = true; });
      const constrain = () => {
        ancestor.inert = true;
        if (state === 'child inert') group.inert = true;
        if (state === 'modal inert') modal.inert = true;
        if (state === 'disabled') button.disabled = input.disabled = true;
        if (state === 'disabled ancestor') group.setAttribute('disabled', 'true');
        if (state === 'nonmodal') { modal.close(); modal.show(); }
      };
      if (stage === 'capture') constrain();
      button.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
      await Promise.resolve();
      const intent = started;
      button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      input.value = 'draft'; input.dispatchEvent(new InputEvent('input', { bubbles: true }));
      if (stage === 'replay') constrain();
      // Capture refusals stay refused even if the controls become usable at replay.
      if (stage === 'capture' && state !== 'escaped') {
        group.inert = modal.inert = false; button.disabled = input.disabled = false;
        group.removeAttribute('disabled');
        if (state === 'nonmodal') { modal.close(); modal.showModal(); }
      }
      capture.hold({ ops: [{ op: 'adopt', adopted: true }] });
      capture.release(() => {});
      await Promise.resolve();
      results[stage + '/' + state] = { intent, events, dropped: logs.length };
      modal.close(); root.remove();
    }
    return results;
  }, stage);
  for (const state of ['escaped', 'child inert', 'modal inert', 'disabled', 'disabled ancestor', 'nonmodal']) {
    expect(results[stage + '/' + state]).toEqual({
      intent: stage === 'replay' || state === 'escaped',
      events: state === 'escaped' ? [[1, 0, ''], [2, 23, 'draft']] : [],
      dropped: stage === 'replay' && state !== 'escaped' ? 2 : 0,
    });
  }
}), 30000);

for (const path of ['pointer', 'keyboard', 'assistive click']) test(`GPU modal controls accept ${path} past inert ancestors`, () => inBrowser(async page => {
  const results = await page.evaluate(path => {
    const host = document.getElementById('ancestor'), modal = document.getElementById('upper');
    const button = document.getElementById('inside'), canvas = document.createElement('canvas');
    host.append(canvas); button.dataset.action = 'jump'; modal.showModal(); host.inert = true;
    const listener = listenGPU(host, canvas), results = {};
    const send = () => {
      if (path === 'pointer') for (const phase of ['down', 'up']) button.dispatchEvent(new PointerEvent('pointer' + phase, { bubbles: true, cancelable: true, pointerId: 7 }));
      if (path === 'keyboard') for (const phase of ['down', 'up']) button.dispatchEvent(new KeyboardEvent('key' + phase, { bubbles: true, cancelable: true, key: ' ', code: 'Space' }));
      if (path === 'assistive click') button.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 0 }));
      return listener.events.splice(0).filter(e => e.t === 'control').map(e => [e.name, e.phase]);
    };
    results.escaped = send();
    button.inert = true; results.childInert = send(); button.inert = false;
    modal.inert = true; results.modalInert = send(); modal.inert = false;
    button.disabled = true; results.disabled = send(); button.disabled = false;
    host.hidden = true; results.hiddenAncestor = send(); host.hidden = false;
    modal.close(); modal.show(); results.nonmodal = send();
    listener.stop();
    return results;
  }, path);
  expect(results).toEqual({ escaped: [['jump', 'down'], ['jump', 'up']], childInert: [], modalInert: [], disabled: [], hiddenAncestor: [], nonmodal: [] });
}), 30000);

test('wasm shown retains the front modal and later popovers in tree order', () => inBrowser(async page => {
  const snapshots = await page.evaluate(() => {
    const el = id => document.getElementById(id), snapshots = { page: read() };
    el('lower').showModal(); el('outer-pop').showPopover();
    snapshots.lower = read();
    el('upper').showModal(); el('inner-pop').showPopover();
    snapshots.upper = read();
    el('ancestor').inert = true;
    snapshots.escape = read();
    el('inner-pop').inert = true;
    snapshots.ownInert = read();
    el('inner-pop').inert = false; el('upper').inert = true;
    snapshots.inertModal = read();
    el('upper').inert = false;
    el('outer-pop').hidePopover(); el('outer-pop').showPopover();
    el('popover-dialog').showPopover();
    snapshots.reopened = read();
    el('ancestor').style.display = 'none';
    snapshots.hiddenAncestor = read();
    el('ancestor').style.display = ''; el('upper').close();
    snapshots.closedUpper = read();
    el('lower').close();
    snapshots.closedAll = read();
    el('lower').addEventListener('beforetoggle', e => e.preventDefault(), { once: true });
    el('lower').showModal();
    snapshots.cancelled = read();
    el('outer-pop').remove();
    snapshots.removed = read();
    return snapshots;
  });
  expect(snapshots).toEqual({
    page: ['page'], lower: ['lower', 'outer-pop'],
    upper: ['upper', 'inside', 'inner-pop', 'below'],
    escape: ['upper', 'inside', 'inner-pop', 'below'],
    ownInert: ['upper', 'inside', 'below'], inertModal: [],
    reopened: ['upper', 'inside', 'inner-pop', 'below', 'outer-pop'],
    hiddenAncestor: ['outer-pop'], closedUpper: ['lower', 'outer-pop'],
    closedAll: ['page', 'outer-pop'], cancelled: ['page', 'outer-pop'], removed: ['page'],
  });
}), 30000);

test('wasm tap accepts modal controls under inert ancestors and refuses inert descendants', () => inBrowser(async page => {
  const results = await page.evaluate(() => {
    const el = id => document.getElementById(id), tap = id => agent({ op: 'tap', id });
    el('upper').showModal(); el('ancestor').inert = true;
    const results = [tap('inside'), tap('inert-child')];
    el('inside').style.visibility = 'hidden'; results.push(tap('inside'));
    el('inside').style.visibility = ''; el('upper').inert = true; results.push(tap('inside'));
    return results;
  });
  expect(results).toEqual([
    { guest: false }, { handled: true, error: 'view inert-child is hidden or inert' },
    { handled: true, error: 'view inside is hidden or inert' },
    { handled: true, error: 'view inside is hidden or inert' },
  ]);
}), 30000);

test('wasm reveal scrolls a modal control past an inert ancestor', () => inBrowser(async page => {
  const result = await page.evaluate(() => {
    const el = id => document.getElementById(id), dialog = el('upper');
    dialog.showModal(); el('ancestor').inert = true;
    const before = dialog.scrollTop;
    el('below').inert = true;
    const refused = agent({ op: 'reveal', id: 'below' }).scrolled, afterRefusal = dialog.scrollTop;
    el('below').inert = false;
    const revealed = agent({ op: 'reveal', id: 'below' }).scrolled;
    return { before, refused, afterRefusal, revealed, moved: dialog.scrollTop > 0 };
  });
  expect(result).toEqual({ before: 0, refused: false, afterRefusal: 0, revealed: true, moved: true });
}), 30000);

test('wasm context popovers escape ancestor inertness but recheck their source before opening', () => inBrowser(async page => {
  const result = await page.evaluate(async () => {
    const el = id => document.getElementById(id), source = el('inside'), pop = el('inner-pop');
    el('upper').showModal(); el('ancestor').inert = true;
    const menu = async block => {
      source.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
      if (block) source.inert = true;
      await new Promise(resolve => setTimeout(resolve)); // after contextAt's queued opening
      return pop.matches(':popover-open');
    };
    const opened = await menu(false);
    if (opened) pop.hidePopover();
    const revoked = await menu(true), inert = await menu(false);
    return { opened, revoked, inert };
  });
  expect(result).toEqual({ opened: true, revoked: false, inert: false });
}), 30000);

test('wasm structural reads scan modal layers once regardless of the candidate count', () => inBrowser(async page => {
  const result = await page.evaluate(async () => {
    const root = document.getElementById('exact-root'), upper = document.getElementById('upper');
    addCandidates(root, 100);
    const query = document.querySelectorAll.bind(document); let scans = 0;
    document.querySelectorAll = selector => {
      if (selector === 'dialog:modal,:popover-open') scans++;
      return query(selector);
    };
    const measure = count => {
      scans = 0;
      const hits = readCandidates(count);
      return { count: hits.length, scans };
    };
    const page = [1, 10, 100].map(measure);
    upper.showModal();
    const covered = measure(100);
    upper.append(...root.querySelectorAll('[data-candidate]'));
    const modal = [1, 10, 100].map(measure);
    upper.close();
    const closed = measure(100);
    root.append(...upper.querySelectorAll('[data-candidate]'));
    const restored = measure(100);
    await Promise.resolve();
    const later = measure(100);
    return { page, covered, modal, closed, restored, later };
  });
  const counts = [1, 10, 100].map(count => ({ count, scans: 1 }));
  expect(result).toEqual({
    page: counts, covered: { count: 0, scans: 1 }, modal: counts,
    closed: { count: 0, scans: 1 }, restored: { count: 100, scans: 1 }, later: { count: 100, scans: 1 },
  });
}), 30000);
