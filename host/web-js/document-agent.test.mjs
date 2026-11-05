import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('navigation waits for unknown status capability', async () => {
  const { pageServer } = await import('./render.mjs');
  const page = pageServer('/unused', { begin: async () => ({
    policy: 'request', head: '<!doctype html>',
    finish: async () => ({ html: 'Missing', rest: 'Missing', status: 404, policy: 'request', failed: [], location: '/missing' }),
  }) }, () => {});
  const response = await page('/missing', name => name === 'sec-fetch-dest' ? 'document' : undefined);
  expect(response.status).toBe(404);
});

test.each([
  ['navigation waits for a page that can declare its status', [
    ['plain', '/', 200, false], ['inactive', '/inactive', 200, false],
    ['missing', '/missing', 404, true], ['gone', '/gone', 410, true], ['unavailable', '/unavailable', 503, true],
    ['static', '/static', 410, true], ['matched', '/matched', 404, true], ['rows', '/rows', 410, true],
    ['virtual', '/virtual', 410, true], ['selection', '/selection', 404, true],
    ['childSelection', '/child-selection', 404, true], ['tabSelection', '/tab-selection', 404, true],
    ['panelRole', '/panel-role', 404, true], ['panelId', '/panel-id', 404, true], ['tabControls', '/tab-controls', 404, true],
    ['siblingSelection', '/sibling-selection', 404, true], ['siblingRemoval', '/sibling-removal', 404, true],
    ['cachedUnavailable', '/cached-unavailable', 503, true], ['cachedUnavailable', '/cached-unavailable', 503, true],
    ['cachedOk', '/cached-ok', 200, true], ['client', '/client', 200, false],
  ]],
  ['navigation waits for store-driven status revalidation', [['storeStatus', '/store-status', 404, true]]],
  ['navigation waits for a discarded request that can change status', [['discardedStatus', '/discarded-status', 404, true]]],
  ['navigation waits for discarded held requests', [['discardedHeldStatus', '/discarded-held-status', 404, true]]],
  ['navigation waits for fetches started in a plain continuation', [
    ['discardedLooseStatus', '/discarded-loose-status', 404, true],
    ['discardedDetachedStatus', '/discarded-detached-status', 404, true],
  ]],
  ['navigation waits for I/O that outlives a source answer', [
    ['backgroundSync', '/background-sync', 404, true], ['backgroundHeldSync', '/background-held-sync', 404, true],
    ['backgroundAnswer', '/background-answer', 404, true], ['backgroundHeldAnswer', '/background-held-answer', 404, true],
  ]],
  ['I/O remains unfinished after the source answers', [
    ['backgroundSync', '/background-sync', 503, true, { deadline: 0 }],
    ['backgroundHeldSync', '/background-held-sync', 503, true, { deadline: 0 }],
    ['backgroundAnswer', '/background-answer', 503, true, { deadline: 0 }],
    ['backgroundHeldAnswer', '/background-held-answer', 503, true, { deadline: 0 }],
  ]],
  ['an inferred page recovers when I/O outlives its answer', [['backgroundClient', '/background-client', 503, true, { deadline: 0, recover: true }]]],
  ['discarded work keeps a render unsettled at the deadline', [
    ['discardedStatus', '/discarded-status', 503, true, { deadline: 0 }],
    ['discardedHeldStatus', '/discarded-held-status', 503, true, { deadline: 0 }],
    ['discardedLooseStatus', '/discarded-loose-status', 503, true, { deadline: 0 }],
    ['discardedDetachedStatus', '/discarded-detached-status', 503, true, { deadline: 0 }],
  ]],
  ['an inferred page recovers from a discarded-work deadline', [['discardedClient', '/discarded-client', 503, true, { deadline: 0, recover: true }]]],
  ['a status-free snapshot keeps discarded held work off its wait', [['discardedPlain', '/discarded-plain', 200, false, { deadline: 0, settled: true }]]],
  ['a discarded timer does not hold a settled status page', [
    ['discardedTimerStatus', '/discarded-timer-status', 200, true, { deadline: 100, settled: true, timer: true }],
    ['discardedLateTimerStatus', '/discarded-late-timer-status', 200, true, { deadline: 100, settled: true, timer: true }],
  ]],
  ['navigation tracks id candidates across ownership boundaries', [
    ['idShadow', '/id-shadow', 404, true], ['idConditional', '/id-conditional', 404, true], ['idRows', '/id-rows', 404, true],
  ]],
  ['navigation tracks an id shadow inside an inactive nested route', [['idInactiveRows', '/id-inactive-rows', 404, true]]],
  ['navigation waits for a route mounted in an inactive tab', [
    ['conditionalTab', '/conditional-tab', 404, true],
    ['conditionalMatch', '/conditional-match', 404, true],
    ['conditionalRows', '/conditional-rows', 404, true],
    ['conditionalNaming', '/conditional-naming', 404, true],
  ]],
  ['navigation sends its head before an unselectable conditional status', [['conditionalInactive', '/conditional-inactive', 200, false]]],
  ['navigation sends its head before an unselectable conditional tab', [['conditionalInactiveTab', '/conditional-inactive-tab', 200, false]]],
  ['navigation ignores data confined to an inactive route', [
    ['inactiveData', '/inactive-data', 200, false], ['inactiveSelector', '/inactive-selector', 200, false],
  ]],
  ['navigation waits for an unnamed copy of a repeated panel', [['repeatedPanel', '/repeated-panel', 404, true]]],
])('%s', async (_name, cases) => {
  if (webCompiler().cmd === 'cargo') { throw new Error('build the compiler first: cargo build -p exact-web-js'); }
  const root = resolve(import.meta.dirname, '../..'), dir = mkdtempSync(resolve(root, 'target/status-document-test-'));
  const held = new Set(), waiting = new Map(), expiredPages = new Map();
  let arrived, site, browser;
  const api = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    if (request.method === 'OPTIONS') { return new Response(null, { status: 204, headers: { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET', 'access-control-allow-headers': '*' } }); }
    const name = new URL(request.url).pathname.slice(1);
    return new Promise(done => {
      const release = () => {
        held.delete(release);
        done(Response.json({ ready: name !== 'cachedOk', key: 'second', label: name + ' loaded', rows: name === 'repeatedPanel' ? [410, 503] : [410] }, { headers: { 'cache-control': 'no-store', 'access-control-allow-origin': '*' } }));
      };
      held.add(release);
      waiting.set(name, release);
      arrived?.(name);
    });
  } });
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Status Document Test', app: { id: 'test.status-document', name: 'Status Document Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `shape Page
  ready: bool
  key: string
  label: string
  rows: number[]

routes nav
  plain "/" render=request activate=never
  missing "/missing" render=request activate=never
  gone "/gone" render=request activate=never
  unavailable "/unavailable" render=request activate=never
  static "/static" render=request activate=never
  matched "/matched" render=request activate=never
  rows "/rows" render=request activate=never
  virtual "/virtual" render=request activate=never
  inactive "/inactive" render=request activate=never
  conditionalInactive "/conditional-inactive" render=request activate=never
  storeStatus "/store-status" render=request activate=never
  discardedStatus "/discarded-status" render=request activate=never
  discardedHeldStatus "/discarded-held-status" render=request activate=never
  discardedLooseStatus "/discarded-loose-status" render=request activate=never
  discardedDetachedStatus "/discarded-detached-status" render=request activate=never
  discardedClient "/discarded-client" render=request
  discardedPlain "/discarded-plain" render=request activate=never
  discardedTimerStatus "/discarded-timer-status" render=request activate=never
  discardedLateTimerStatus "/discarded-late-timer-status" render=request activate=never
  backgroundSync "/background-sync" render=request activate=never
  backgroundHeldSync "/background-held-sync" render=request activate=never
  backgroundAnswer "/background-answer" render=request activate=never
  backgroundHeldAnswer "/background-held-answer" render=request activate=never
  backgroundClient "/background-client" render=request
  idShadow "/id-shadow" render=request activate=never
  idConditional "/id-conditional" render=request activate=never
  idRows "/id-rows" render=request activate=never
  idInactiveRows "/id-inactive-rows" render=request activate=never
  conditionalInactiveTab "/conditional-inactive-tab" render=request activate=never
  repeatedPanel "/repeated-panel" render=request activate=never
  selection "/selection" render=request activate=never
  childSelection "/child-selection" render=request activate=never
  siblingSelection "/sibling-selection" render=request activate=never
  siblingRemoval "/sibling-removal" render=request activate=never
  tabSelection "/tab-selection" render=request activate=never
  conditionalTab "/conditional-tab" render=request activate=never
  conditionalMatch "/conditional-match" render=request activate=never
  conditionalRows "/conditional-rows" render=request activate=never
  conditionalNaming "/conditional-naming" render=request activate=never
  inactiveData "/inactive-data" render=request activate=never
  inactiveSelector "/inactive-selector" render=request activate=never
  panelRole "/panel-role" render=request activate=never
  panelId "/panel-id" render=request activate=never
  tabControls "/tab-controls" render=request activate=never
  cachedUnavailable "/cached-unavailable" render=cached activate=never
  cachedOk "/cached-ok" render=cached activate=never
  client "/client" render=request activate=idle

component App
  state pressed = false
  state selected = "first"
  resource page = page(top(nav).name) as shape Page else empty(key="first")
  resource stored = stored() as shape bool
  resource mode = mode() as shape string
  resource discarded = discarded(mode, top(nav).name) as shape number
  resource initialized = initialized(top(nav).name) as shape number
  action flip
    pressed = not pressed
  view
    main
      if top(nav).name == "plain" or top(nav).name == "discardedPlain"
        head title="Plain"
      else if top(nav).name == "storeStatus" or top(nav).name == "discardedStatus" or top(nav).name == "discardedHeldStatus" or top(nav).name == "discardedLooseStatus" or top(nav).name == "discardedDetachedStatus" or top(nav).name == "discardedClient" or top(nav).name == "discardedTimerStatus" or top(nav).name == "discardedLateTimerStatus" or top(nav).name == "backgroundSync" or top(nav).name == "backgroundHeldSync" or top(nav).name == "backgroundAnswer" or top(nav).name == "backgroundHeldAnswer" or top(nav).name == "backgroundClient"
        if stored
          head title="Missing" status=404
        else
          head title="Checking"
        if top(nav).name == "discardedClient" or top(nav).name == "backgroundClient"
          button press=flip testId="recovery-press"
            text "Change"
      else if top(nav).name == "conditionalInactive"
        view navigationBack="" navigationKey="first"
          view navigationKey="first"
            head title="First"
            view navigationBack="" navigationKey="nested"
              row role="tablist"
                button role="tab" aria-controls="unrelated-nested-panel"
                  text "Nested"
              view role="tabpanel" id="unrelated-nested-panel"
                view navigationKey="nested"
                  text "Nested page"
          if page.ready
            view navigationKey="second"
              head title="Missing" status=404
      else if top(nav).name == "inactiveSelector"
        view navigationBack="" navigationKey=selected
          view navigationKey="first"
            head title="First"
          view navigationKey="second"
            view navigationBack="" navigationKey=page.key
              view navigationKey="first"
                head title="Nested first"
              view navigationKey="second"
                head title="Missing" status=404
      else if top(nav).name == "inactiveData"
        view navigationBack="" navigationKey=selected
          view navigationKey="first"
            head title="First"
          view navigationKey="second" id=(page.ready ? "changed" : "initial")
            if page.ready
              view role="group"
                view navigationBack="" navigationKey="nested"
                  view navigationKey="nested"
                    head title="Missing" status=404
      else if top(nav).name == "conditionalInactiveTab"
        view navigationBack="" navigationKey="sibling"
          view id="fixed-first-panel"
        view navigationBack="" navigationKey="first"
          view navigationBack="" navigationKey="unrelated"
            row role="tablist"
              button role="tab" aria-controls="fixed-second-panel"
                text "Unrelated tabs"
          if false
            row role="tablist"
              button role="tab" aria-controls="fixed-second-panel"
                text "Unreachable"
          if true
            row role="tablist"
              if true
                button role="tab" aria-controls="fixed-first-panel"
                  text "First"
                button role="tab" aria-controls="fixed-second-panel"
                  text "Second"
          view role="tabpanel" id="fixed-first-panel"
            view navigationKey="first"
              head title="First"
          if page.ready
            view role="tabpanel" id="fixed-second-panel"
              view navigationKey="second"
                head title="Missing" status=404
      else if top(nav).name == "idShadow" or top(nav).name == "idConditional" or top(nav).name == "idRows" or top(nav).name == "idInactiveRows"
        view navigationBack="" navigationKey="first"
          row role="tablist"
            button role="tab" aria-controls="id-status-panel"
              text "Status"
            button role="tab" aria-controls="id-plain-panel"
              text "Plain"
          if top(nav).name == "idShadow"
            view id=(page.ready ? "id-status-panel" : "unused")
          if top(nav).name == "idRows"
            view navigationBack="" navigationKey="nested"
              for code in page.rows key=code
                view id="id-status-panel"
          if top(nav).name == "idInactiveRows"
            view navigationBack="" navigationKey="nested"
              view navigationKey="nested"
                text "Selected"
              view navigationKey="covered"
                for code in page.rows key=code
                  view id="id-status-panel"
          view role="tabpanel" id="id-status-panel"
            view navigationKey="second"
              head title="Missing" status=404
              if top(nav).name == "idConditional" and page.ready
                view id="id-plain-panel"
          view role="tabpanel" id="id-plain-panel"
            view navigationKey="first"
              head title="First"
      else if top(nav).name == "repeatedPanel"
        view navigationBack="" navigationKey="first"
          row role="tablist"
            button role="tab" aria-controls="repeated-first-panel"
              text "First"
            button role="tab" aria-controls="repeated-second-panel"
              text "Second"
          view role="tabpanel" id="repeated-first-panel"
            view navigationKey="first"
              head title="First"
          for code in page.rows key=code
            view role="tabpanel" id="repeated-second-panel"
              view navigationKey="second"
                head title="Missing" status=404
      else if top(nav).name == "client"
        if pressed
          head title="Client missing" status=404
        else
          head title="Client"
        button press=flip testId="flip"
          text "Change head"
      else if top(nav).name == "inactive" or top(nav).name == "selection"
        view navigationBack="" navigationKey=(top(nav).name == "selection" ? page.key : "first")
          view navigationKey="first"
            head title="First"
            text "First"
          view navigationKey="second"
            head title="Missing" status=404
            text "Missing"
      else if top(nav).name == "gone"
        if page.ready
          head title="Gone" status=410
      else if top(nav).name == "unavailable" or top(nav).name == "cachedUnavailable"
        if page.ready
          head title="Unavailable" status=503
      else if top(nav).name == "static"
        head title="Static" status=410
      else if top(nav).name == "childSelection"
        view navigationBack="" navigationKey="second"
          view navigationKey=page.key
            head title="Missing" status=404
          view navigationKey=(page.ready ? "first" : "second")
            head title="First"
      else if top(nav).name == "tabSelection" or top(nav).name == "panelRole" or top(nav).name == "panelId" or top(nav).name == "tabControls"
        view navigationBack="" navigationKey=(top(nav).name == "tabSelection" ? "second" : "first")
          row role="tablist"
            button role="tab" aria-controls=(top(nav).name == "tabControls" and page.ready ? "unused" : "status-panel")
              text "Status"
            button role="tab" aria-controls="plain-panel"
              text "Plain"
          view role=(top(nav).name == "panelRole" and page.ready ? "group" : "tabpanel") id=(top(nav).name == "panelId" and page.ready ? "unused" : "status-panel")
            view navigationKey=(top(nav).name == "tabSelection" ? page.key : "second")
              head title="Missing" status=404
          view role="tabpanel" id="plain-panel"
            view navigationKey=(top(nav).name == "tabSelection" and not page.ready ? "second" : "first")
              head title="First"
      else if top(nav).name == "siblingSelection" or top(nav).name == "siblingRemoval"
        view navigationBack="" navigationKey="first"
          if top(nav).name == "siblingSelection"
            view navigationKey=(page.ready ? "third" : "first")
              text "Plain"
          else if not page.ready
            view navigationKey="first"
              text "Plain"
          view navigationKey="second"
            head title="Missing" status=404
      else if top(nav).name == "conditionalTab" or top(nav).name == "conditionalMatch" or top(nav).name == "conditionalRows" or top(nav).name == "conditionalNaming"
        view navigationBack="" navigationKey="first"
          view role="tabpanel" id="future-panel"
            if top(nav).name == "conditionalNaming"
              if page.ready
                row role="tablist"
                  button role="tab" aria-controls="future-panel"
                    text "Future"
              view navigationKey="second"
                head title="Missing" status=404
            else if top(nav).name == "conditionalTab"
              if page.ready
                view navigationKey="first"
                  head title="Missing" status=404
            else if top(nav).name == "conditionalMatch"
              match (page.ready ? some(page.label) : none)
                case some(label)
                  view navigationKey="first"
                    head title=label status=404
                case none
                  text "Waiting"
            else
              for code in page.rows key=code
                view navigationKey="first"
                  head title="Missing" status=404
          view role="tabpanel" id="current-panel"
            view navigationKey="first"
              head title="First"
          row role="tablist"
            button role="tab" aria-controls="future-panel"
              text "Future"
            button role="tab" aria-controls="current-panel"
              text "Current"
      else if top(nav).name == "matched"
        match (page.ready ? some(page.label) : none)
          case some(label)
            head title=label status=404
          case none
            text "Waiting"
      else if top(nav).name == "rows"
        for code in page.rows key=code
          view
            head title="Row" status=410
            text "Row"
      else if top(nav).name == "virtual"
        list virtualized=true height=200
          for code in page.rows key=code
            view height=24
              head title="Virtual row" status=410
              text "Virtual row"
      else
        if page.ready
          head title="Result" status=404
      text page.label testId="loaded"
`);
    writeFileSync(resolve(dir, 'app.ts'), `import type { Answer } from './app.contract.d.ts';
export const appId = 'test.status-document';
export const grants = ${JSON.stringify('net.fetch ' + api.url.origin + '\nsecret.keep status\nsecret.keep mode')};
export const answer: Answer = (source, args, store) => {
  const [name] = args;
  if (source === 'stored') { return (store.get('status') === 'missing') as never; }
  if (source === 'mode') { return (store.get('mode') ?? 'old') as never; }
  if (source === 'initialized') {
    if (String(name).startsWith('discarded')) { store.set('mode', 'new'); }
    return 0 as never;
  }
  if (source === 'discarded') {
    const [mode, page] = args;
    const background = String(page).startsWith('background');
    if ((!String(page).startsWith('discarded') && !background) || mode !== 'old') { return 0 as never; }
    if (page === 'discardedTimerStatus' || page === 'discardedLateTimerStatus') {
      return new Promise(done => setTimeout(() => { store.set('status', 'missing'); done(0); }, page === 'discardedTimerStatus' ? 0 : 10_000)) as never;
    }
    if (page === 'discardedHeldStatus' || page === 'discardedLooseStatus') { setTimeout(() => {}, 10_000); }
    const request = () => fetch(${JSON.stringify(api.url.origin)} + '/' + page, { cache: 'no-store', exactHeld: !['discardedStatus', 'backgroundSync', 'backgroundAnswer'].includes(String(page)) });
    const loose = page === 'discardedLooseStatus' || page === 'discardedDetachedStatus';
    const answer = (loose ? Promise.resolve().then(request) : request()).then(() => {
      store.set('status', 'missing');
      return 0;
    });
    if (page === 'discardedDetachedStatus' || background) {
      answer.catch(() => {});
      return (page === 'backgroundAnswer' || page === 'backgroundHeldAnswer' ? Promise.resolve(0) : 0) as never;
    }
    return answer as never;
  }
  if (String(name).startsWith('discarded') || String(name).startsWith('background')) {
    return { ready: false, key: 'first', label: name + ' loaded', rows: [] } as never;
  }
  return (async () => {
    const value = await (await fetch(${JSON.stringify(api.url.origin)} + '/' + name, { cache: 'no-store' })).json();
    if (name === 'storeStatus') { store.set('status', 'missing'); }
    return value;
  })() as never;
};
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'status-document-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const { pageServer, renderer } = await import('./render.mjs');
    const serve = pageServer(dist, undefined, () => {});
    site = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
      const path = new URL(request.url).pathname;
      if (path === '/favicon.ico') { return new Response(null, { status: 404 }); }
      const asset = Bun.file(resolve(dist, path.slice(1)));
      if (path.includes('.') && await asset.exists()) { return new Response(asset); }
      if (expiredPages.has(path)) { const reply = expiredPages.get(path); return new Response(reply.html, { status: reply.status, headers: { 'content-type': 'text/html' } }); }
      const reply = await serve(path, name => request.headers.get(name) ?? undefined);
      return new Response(reply.body, { status: reply.status, headers: reply.headers });
    } });
    browser = await suiteBrowser();
    const page = await browser.newPage();
    for (const [name, path, expected, waits, options] of cases) {
      let seen = new Promise(done => { arrived = found => { if (found === name) { done(); } }; });
      const waitForData = () => {
        let bound;
        return Promise.race([seen, new Promise((_, reject) => {
          bound = setTimeout(() => reject(new Error(name + ' never asked for its data')), 10_000);
        })]).finally(() => clearTimeout(bound));
      };
      if (options?.deadline !== undefined) {
        const render = renderer(dist); render.ahead = false;
        const begun = await render.begin(path);
        if (!options.timer) { await waitForData().catch(e => { begun.dispose(); throw e; }); }
        const reply = await begun.finish(options.deadline);
        expect({ status: reply.status, settled: reply.settled }).toEqual({ status: expected, settled: options.settled ?? false });
        if (!options.timer) { waiting.get(name)(); waiting.delete(name); }
        if (options.recover) {
          expiredPages.set(path, reply);
          seen = new Promise(done => { arrived = found => { if (found === name) { done(); } }; });
          expect((await page.goto(site.url.origin + path, { waitUntil: 'commit' })).status()).toBe(expected);
          await waitForData();
          waiting.get(name)(); waiting.delete(name);
          await page.waitForFunction(() => document.title === 'Missing');
        }
        continue;
      }
      let headers = false;
      const navigation = page.goto(site.url.origin + path, { waitUntil: 'commit' }).then(response => { headers = true; return response; });
      navigation.catch(() => {});
      await waitForData();
      if (waits) {
        expect(headers, name + ' commits its status after the data answers').toBe(false);
      } else {
        expect((await navigation).status(), name + ' sends its head before the data answers').toBe(200);
      }
      waiting.get(name)();
      const response = await navigation;
      expect(response.status(), name).toBe(expected);
      if (expected !== 200) { expect(response.headers()['cache-control'], name).toBe('no-store'); }
      if (expected === 503) { expect(response.headers()['retry-after'], name).toBe('1'); }
      if (name === 'cachedOk') { expect(response.headers()['cache-control']).toBe('public, max-age=0, s-maxage=60'); }
      await page.locator('[data-testid="loaded"]').waitFor({ state: 'attached' });
      expect(await page.locator('[data-testid="loaded"]').textContent()).toBe(name + ' loaded');
      if (name === 'client') {
        await page.locator('[data-testid="flip"]').click();
        await page.waitForFunction(() => document.title === 'Client missing');
      }
      waiting.delete(name);
    }
  } finally {
    for (const release of held) { release(); }
    await browser?.close(); site?.stop(true); api.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120_000);

test('a second navigation root keeps its split list live while its detail supplies the head', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Agent Test', app: { id: 'test.heads', name: 'Head Agent Test' }, host: { web: {} } }));
    // The earlier covered copy must lose testId lookup to the visible split list's control.
    const source = readFileSync(new URL('./conformance/heads-split.contract', import.meta.url), 'utf8').replace('      main navigationKey=', `      main navigationKey="1002" navigationBack="unused" height=24
        column navigationKey="1001"
          button press=toggle testId="list-commit"
            text "Covered control"
        column navigationKey="1002"
          text "Another navigation root"
      main navigationKey=`);
    writeFileSync(resolve(dir, 'app.contract'), source);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'head-agent-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [1000, 900] });
    expect(await browser.carrier.evaluate('document.querySelectorAll("[navigationBack]").length')).toBe(2);
    await browser.tap('open-detail');
    await browser.tap('commit');
    let commits = 1;
    for (const width of [1000, 420, 1000]) {
      await browser.resize(width, 900);
      await browser.clock('settle');
      const wide = width >= 900, copies = (await browser.tree()).nodes.filter(n => n.props.testId === 'list-commit');
      expect(copies).toHaveLength(2);
      // A covered route is hidden too (LLP 1153 D8), and so is the split's list where the detail covers it.
      const [covered, list] = copies, observed = await browser.op({ op: 'check' });
      expect({ covered: [!!covered.inactive, !!covered.hidden], list: [!!list.inactive, !!list.hidden], checked: observed.items.some(item => item.id === list.id && item.control === 'button') })
        .toEqual({ covered: [true, true], list: [!wide, !wide], checked: wide });
      if (wide) {
        expect((await browser.tree('list-commit', { shallow: true })).nodes[0].id).toBe(list.id);
        await browser.tap('list-commit');
        commits++;
      }
      const state = await browser.state();
      expect(state.slots).toMatchObject({ commits, named: true });
      expect(state.head).toMatchObject({ title: 'Ada', description: 'Directory description', canonical: '/directory' });
      expect(await browser.carrier.evaluate('document.title')).toBe('Ada');
    }
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

test('a JavaScript-rendered direct detail selects its retained href at boot', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/split-document-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Split Document Agent Test', app: { id: 'test.split-document', name: 'Split Document Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  home "/" render=build
    alpha "/a" render=build

component SplitDocumentAgent
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" position="relative" testId="navigation"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0 testId=\`route-\${e.name}\`
          if e.name == "home"
            list
              link href="/a" testId="row-alpha"
                text "Alpha"
          if e.name == "alpha"
            text "Alpha detail"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'split-document-agent-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path === '/a' ? 'a/index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: new URL('a', server.url).href, size: [1280, 844] });
    const state = await browser.state();
    expect(state.navigation).toMatchObject({ url: '/a', route: '1', stack: ['0', '1'] });
    expect(await browser.carrier.evaluate(`(() => ({
      list: document.querySelector('[data-testid="route-home"]').getAttribute('data-exact-split'),
      detail: document.querySelector('[data-testid="route-alpha"]').getAttribute('data-exact-split'),
      selected: document.querySelector('[data-testid="row-alpha"]').hasAttribute('data-exact-split-row'),
    }))()`)).toEqual({ list: 'list', detail: 'detail', selected: true });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

// @ref LLP 1075.003 §9.14 — a page rendered ahead holds every tab panel, and the boot that adopts it builds
// every one over what was rendered, wiring each panel's presses; none waits on that page.
test('an adopted page builds every tab over the rendered ones', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/tabs-document-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Tabs Document Agent Test', app: { id: 'test.tabs-document', name: 'Tabs Document Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  tab a "/" render=build
  tab b "/b"

component TabsDocument
  state n = 0
  action pick(name: string)
    nav = select(nav, name)
  action bump
    n = n + 1
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 position="relative"
        for t in nav.tabs key=t.name
          column role="tabpanel" id=\`panel-\${t.name}\` testId=\`panel-\${t.name}\` position="absolute" inset=0
            for e in t.stack key=e.id
              column navigationKey=\`\${e.id}\` position="absolute" inset=0
                button press=bump testId=\`bump-\${e.name}\`
                  text \`\${e.name} \${n}\`
      row role="tablist"
        button role="tab" aria-controls="panel-a" aria-selected=(nav.tab == "a") press=pick("a") testId="tab-a"
          text "A"
        button role="tab" aria-controls="panel-b" aria-selected=(nav.tab == "b") press=pick("b") testId="tab-b"
          text "B"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'tabs-document-agent-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const html = readFileSync(resolve(dist, 'index.html'), 'utf8');
    expect(html).toContain('data-testid="bump-a"');
    expect(html).toContain('data-testid="bump-b"');
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      // `/b` is served the page rendered for `/`, which selects tab A.
      const file = Bun.file(resolve(dist, path === '/' || path === '/b' ? 'index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
    expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
    const panels = async () => Object.fromEntries((await browser.tree()).nodes.filter(n => n.props.accessibilityRole === 'tabpanel').map(n => [n.props.testId, !!n.unopened]));
    expect(await panels()).toEqual({ 'panel-a': false, 'panel-b': false });
    await browser.tap('bump-a');
    expect((await browser.state()).slots).toMatchObject({ n: 1 });
    await browser.tap('tab-b');
    await browser.tap('bump-b');
    expect((await browser.state()).slots).toMatchObject({ n: 2 });
    expect(await panels()).toEqual({ 'panel-a': false, 'panel-b': false });
    // A launch that selects B over the page rendered with A selected adopts it, B's panel included.
    await browser.close();
    browser = await open({ host: 'web', url: new URL('b', server.url).href, size: [420, 900] });
    expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
    expect((await browser.state()).navigation).toMatchObject({ url: '/b' });
    await browser.tap('bump-b');
    expect((await browser.state()).slots).toMatchObject({ n: 1 });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

// LLP 1153 D8 — a flowed paragraph's runs are held off the page while its fragments paint (flow.js), so the tree
// gives them their paragraph's route flags: hidden under a pushed route, inactive under a sheet.
test('a flowed paragraph\'s runs keep their route\'s flags', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/flow-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Flow Agent Test', app: { id: 'test.flow-agent', name: 'Flow Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  home "/"
    detail "/detail"
    sheet "/sheet"

component FlowAgent
  action open(url: string)
    nav = push(nav, url)
  action back
    nav = back(nav)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0 navigationPresentation=(e.name == "sheet" ? "modal" : "auto")
          if e.name == "home"
            box display="block" position="relative" width=300 height=160
              text width=300 height=160 testId="prose"
                text "Words that flow around a square, " testId="run-a"
                text "and more words after them." testId="run-b"
              box position="absolute" left=0 top=0 width=60 height=60 wrap-flow="both"
            button press=open("/detail") testId="open-detail"
              text "Detail"
            button press=open("/sheet") testId="open-sheet"
              text "Sheet"
          else
            button press=back testId=\`back-\${e.name}\`
              text "Back"
`);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'flow-agent-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path.includes('.') ? path.slice(1) : 'index.html'));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
    // The runs are off the page once the flow paints its fragments, which it loads after first paint.
    const flowed = () => browser.carrier.evaluate('!!document.querySelector(\'[data-testid="prose"] > [data-flow-fragment]\') && !document.querySelector(\'[data-testid="run-a"]\')?.isConnected');
    for (let at = Date.now(); !await flowed(); await new Promise((r) => setTimeout(r, 50))) if (Date.now() - at > 20000) throw new Error('the paragraph was never flowed');
    const runs = async () => {
      const { nodes } = await browser.tree();
      return ['run-a', 'run-b'].map((id) => nodes.find((n) => n.props.testId === id)).map((n) => [!!n.inactive, !!n.hidden]);
    };
    expect(await runs()).toEqual([[false, false], [false, false]]);
    await browser.tap('open-detail');
    await browser.clock('settle');
    expect(await flowed()).toBe(true);
    expect(await runs()).toEqual([[true, true], [true, true]]);
    await browser.tap('back-detail');
    await browser.clock('settle');
    await browser.tap('open-sheet');
    await browser.clock('settle');
    expect(await flowed()).toBe(true);
    expect(await runs()).toEqual([[true, false], [true, false]]);
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 60000);

for (const render of ['none', 'js']) for (const literal of [false, true]) {
  test(`${render} navigation preserves ${literal ? 'literal' : 'bound'} inert on its tabs`, async () => {
    if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
    const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/inert-tabs-test-'));
    let browser, server;
    try {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Inert Tabs', app: { id: 'test.inert-tabs', name: 'Inert Tabs' }, host: { web: {} } }));
      writeFileSync(resolve(dir, 'app.contract'), `routes nav
  tab a "/" render=build
  tab b "/b"

component InertTabs
  state covered = false
  action cover
    covered = not covered
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=\`\${top(nav).id}\` width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 position="relative"
        for tab in nav.tabs key=tab.name
          column role="tabpanel" id=\`panel-\${tab.name}\` position="absolute" inset=0
            for entry in tab.stack key=entry.id
              column navigationKey=\`\${entry.id}\` position="absolute" inset=0
                text entry.name
      row role="tablist" appearance="none" inert=${literal ? 'true' : 'covered'} testId="tabs"
        button role="tab" aria-controls="panel-a" aria-selected=(nav.tab == "a") press=pick("a") testId="tab-a"
          text "A"
        button role="tab" aria-controls="panel-b" aria-selected=(nav.tab == "b") press=pick("b") testId="tab-b"
          text "B"
      button press=cover testId="cover"
        text "Toggle cover"
`);
      const dist = resolve(dir, 'dist');
      const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'inert-tabs-test', '--out', dist, '--render', render], {
        cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
      });
      const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
      expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
      server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
        const path = new URL(req.url).pathname;
        const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
        return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
      } });
      browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
      if (render === 'js') expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
      await browser.tap('cover');
      expect(await browser.carrier.evaluate('document.querySelector("[data-testid=tabs]").inert')).toBe(true);
      const { x, y } = await browser.carrier.evaluate('(()=>{const r=document.querySelector("[data-testid=tab-b]").getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()');
      await browser.carrier.call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
      await browser.carrier.call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 });
      await browser.clock('settle');
      expect((await browser.state()).navigation.url).toBe('/');
      await expect(browser.tap('tab-b')).rejects.toThrow(/hidden or inert/);
      await browser.tap('cover');
      expect(await browser.carrier.evaluate('document.querySelector("[data-testid=tabs]").inert')).toBe(literal);
      if (!literal) {
        await browser.tap('tab-b');
        expect((await browser.state()).navigation.url).toBe('/b');
      }
    } finally {
      await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
    }
  }, 30000);
}
