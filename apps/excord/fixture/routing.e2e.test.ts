// Excord's channel follows its address: the address changes the test language cannot make (the browser's
// Forward and Back landing in the middle of a step), and typed addresses at a standing clock, driven on the REAL app: its web
// build (host/web/dist, built for apps/excord: `bun host/web/build.mjs excord-web`) in Chromium, its
// routing.contract's derives and tasks running in the page as they ship. Nothing is modelled: each step is
// the agent's own page operation (`exact.agentSettled`, as `agent.mjs web` sends it), or the browser's own
// history (`page.goBack`/`goForward`, which reach the app through its root's `navigate`, as a person's Back
// and Forward do). The page's clock stands still between operations, as in a test file, so two steps taken
// without a clock step between are both taken before the first one resolved.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { startFixture } from './server.ts';

const ROOT = resolve(import.meta.dir, '../../..');
const DIST = resolve(ROOT, 'host/web/dist');
const EPOCH = Date.parse('2026-10-22T18:00:00Z');
const GENERAL = '1191168914235588608', HELP = '1191168914235588609', RANDOM = '1191168914235588610';
const DM = '1191168914239782912';
// A web build of Excord to drive, else skipped (Excord's unit run has none).
const built = existsSync(resolve(DIST, '.exact-build.json'))
  && readFileSync(resolve(DIST, '.exact-build.json'), 'utf8').includes('com.exact.excord');

let fixture: ReturnType<typeof startFixture>, server: Server, browser: any, base = '', pages = 0, fixtureOrigin = '';
beforeAll(async () => {
  if (!built) return;
  const { serveBuildTree } = await import(resolve(ROOT, 'host/web/serve.mjs'));
  const { bootedGeneration, leaseGeneration } = await import(resolve(ROOT, 'host/web/generations.mjs'));
  // The fixture listens where the build was baked to reach it: this checkout's fixture port (main d7e95a567d),
  // as `bun scripts/agent-launch.mjs --fixture-port --app excord` prints it, or its override.
  const { backendOriginSelection } = await import(resolve(ROOT, 'js/bake/src/typescript.mjs'));
  const selected = backendOriginSelection(resolve(ROOT, 'apps/excord'));
  if (!selected) throw new Error('Excord has no selected fixture origin');
  fixtureOrigin = selected.origin;
  fixture = startFixture({ port: Number(new URL(selected.origin).port), world: 'transcript', resetOnIdentify: true });
  const pinned = await bootedGeneration(DIST);
  leaseGeneration(DIST, pinned);
  server = createServer((req, res) => serveBuildTree(DIST, req, res, { pinned }));
  await new Promise<void>(ok => server.listen(0, '127.0.0.1', ok));
  base = `http://127.0.0.1:${(server.address() as any).port}`;
  const { chromium } = createRequire(resolve(ROOT, 'scripts/agent.mjs'))('playwright-core');
  browser = await chromium.launch({ headless: true, executablePath: chromium.executablePath() });
});
afterAll(async () => {
  await browser?.close();
  server?.close();
  await fixture?.close();
});

// A page of the app at `path`, its agent adapter loaded (`?agent`), and its steps.
async function app(path = '/', viewport = { width: 420, height: 900 }, { scrollend = true } = {}) {
  const context = await browser.newContext({ viewport });
  // A host that never delivers `scrollend` (Windows' painted presenter): the page's listeners for it are
  // never added, so the app sees every scroll and no scroll's end.
  if (!scrollend) await context.addInitScript(() => {
    const add = EventTarget.prototype.addEventListener;
    EventTarget.prototype.addEventListener = function (type: string, ...rest: any[]) {
      if (type !== 'scrollend') return add.call(this, type, ...rest);
    } as typeof add;
  });
  // Otherwise the page's scrollend listeners can be held (`holdEnds()`) and let go (`releaseEnds()`), as iOS holds
  // scrollend while a finger tracks and a Mac while a trackpad gesture is live: what they deliver later, in order.
  else await context.addInitScript(() => {
    const g = globalThis as any, add = EventTarget.prototype.addEventListener;
    g.heldEnds = []; g.endsSeen = 0;
    EventTarget.prototype.addEventListener = function (type: string, listener: any, ...rest: any[]) {
      if (type !== 'scrollend' || typeof listener !== 'function') return add.call(this, type, listener, ...rest);
      return add.call(this, type, function (this: any, e: Event) {
        if (g.holding) g.heldEnds.push(() => listener.call(this, e)); else { g.endsSeen++; listener.call(this, e); }
      }, ...rest);
    } as typeof add;
    g.holdEnds = () => { g.holding = true; };
    g.releaseEnds = () => { g.holding = false; for (const f of g.heldEnds.splice(0)) { g.endsSeen++; f(); } };
  });
  const page = await context.newPage();
  // A scratch store of its own (`storage`, as `agent.mjs --storage`): the gateway keeps its identify record there.
  await page.goto(`${base}${path}?agent&storage=excord-routing-${process.pid}-${++pages}&epoch=${EPOCH}`);
  await page.waitForFunction(() => typeof (globalThis as any).exact?.agentSettled === 'function', null, { timeout: 30_000 });
  const op = (req: any) => page.evaluate((r: any) => (globalThis as any).exact.agentSettled(r).then(JSON.stringify), req).then(JSON.parse);
  let clock = 0;
  const step = async (req: any) => {
    const r = await op(req);
    if (r?.error) throw new Error(`${JSON.stringify(req)}: ${r.error}`);
    if (typeof r?.clock === 'number') clock = r.clock;
    return r;
  };
  const bag = async () => { const s = await step({ op: 'state' }); return { ...s.resources, ...s.derives, ...s.slots }; };
  return {
    page, bag, step,
    close: () => context.close(),
    // `clock +<ms> [real]`: real time passing with the clock a step at a time, as `agent.mjs` does it (the
    // gateway's READY and the data module's clock run on it); else the clock alone.
    plus: async (ms: number, real = false) => {
      const end = clock + ms;
      if (!real) return step({ op: 'clock', to: end });
      const from = clock, t0 = performance.now();
      for (;;) {
        const to = Math.max(clock, Math.min(end, from + (performance.now() - t0)));
        const r = await step({ op: 'clock', to, step: true });
        if (to >= end) return r;
        await Bun.sleep(Math.min(20, end - to));
      }
    },
    // `clock settle`, as agent.mjs runs it: the page settles, then a task an `after(1, …)` armed a commit later
    // has its clock moved to and the page settles again, up to 16 rounds (agent-inspect.mjs `nextCommitTasks`).
    settle: async () => {
      let r;
      for (let round = 0; round < 16; round++) {
        r = await step({ op: 'clock', settle: true });
        const tasks = Object.values((await step({ op: 'state' })).tasks ?? {}) as number[];
        const soon = tasks.filter(at => Number.isFinite(at) && at > clock && at <= clock + 1).sort((x, y) => x - y);
        if (!soon.length) return r;
        await step({ op: 'clock', to: soon[soon.length - 1] });
      }
      return r;
    },
    data: () => step({ op: 'clock', data: true }),
    // The clock a millisecond at a time (each tick's replies landed), with what is open and kept read after each:
    // a request that must never open shows here even if a later one replaces it.
    ticks: async (n: number) => {
      const seen: { open: string, onScreen: string, server: string }[] = [];
      for (let i = 0; i < n; i++) {
        await step({ op: 'clock', to: clock + 1 });
        await step({ op: 'clock', data: true });
        const b = { ...(await step({ op: 'state' })).slots };
        seen.push({ open: b.open, onScreen: b.onScreen, server: b.server });
      }
      return seen;
    },
    // A location typed into the navigation root: its `navigate`, as the agent's `type <root> "<url>"` delivers it.
    go: async (url: string) => {
      const tree = await step({ op: 'tree' });
      const root = tree.nodes.find((n: any) => n.handlers?.includes('navigate'));
      if (!root) throw new Error('no navigation root');
      const r = await step({ op: 'type', id: root.id, text: url });
      await step({ op: 'clock', land: true });
      return r;
    },
    back: async () => { const r = await step({ op: 'tap', back: true }); await step({ op: 'clock', land: true }); return r; },
    // `tap <target>`: a testId or a view's text; of several views (a route's screen under another), the one
    // that takes the tap.
    tap: async (target: string) => {
      const tree = await step({ op: 'tree' });
      // Views under a route not on top are `inactive` in the tree.
      const ids = tree.nodes.filter((n: any) => !n.inactive && (n.props?.testId === target || n.props?.text === target)).map((n: any) => n.id).reverse();
      let last = `no view matches ${target}`;
      for (const id of ids) {
        // The page refuses a hidden or inert view; any other the carrier clicks, as agent.mjs's does.
        const r = await op({ op: 'tap', id });
        if (r?.error) { last = r.error; continue; }
        const el = (await page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), id)).asElement();
        if (!el || !await el.isVisible()) { last = `view ${id} is not visible`; continue; }
        await el.click();
        // The end of an input: what it settled lands, the clock unmoved (agent.mjs's `clock land` after an input).
        await step({ op: 'clock', land: true });
        return { tapped: id };
      }
      throw new Error(`tap ${target}: ${last}`);
    },
  };
}

// The launch: READY on real time, then #general with the fixture's links message in it.
async function generalWithLinks(a: Awaited<ReturnType<typeof app>>) {
  await a.plus(1500, true);
  await a.settle();
  await a.tap('channel-general');
  await a.settle();
  await a.tap('fixture');
  await a.tap('inject-links');
  await a.data();
  await a.settle();
}

test.skipIf(!built)('the browser\'s Forward to a channel\'s address opens it; Back returns', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.tap('#help');
    await a.data();
    await a.settle();
    expect((await a.bag()).open).toBe(HELP);
    await a.page.goBack();
    await a.data();
    await a.settle();
    expect((await a.bag()).open).toBe(GENERAL);
    await a.page.goForward();
    await a.data();
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.onScreen]).toEqual([HELP, 'help', HELP]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('a newer address wins: two addresses before the first opens, only the second opens', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    // Two addresses typed at a standing clock: #help is asked, then #random; #help never opens.
    await a.go(`/c/${HELP}`);
    await a.go(`/c/${RANDOM}`);
    const asked = await a.bag();
    expect([asked.open, asked.wantId]).toEqual([GENERAL, RANDOM]);
    // Tick by tick, #help is never open and never kept.
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === HELP || t.onScreen === HELP)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.onScreen, s.pendingOpen]).toEqual([RANDOM, 'random', RANDOM, '']);
  } finally { await a.close(); }
}, 180_000);

test.skipIf(!built)('a request cancelled after its server was selected puts the server back', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    const before = (await a.bag()).server;
    // The direct message's address: its server (the direct messages) is selected a tick later.
    await a.go(`/c/${DM}`);
    let ticks = 0;
    while ((await a.bag()).wantServer !== true && ticks++ < 5) await a.plus(1);
    const selected = await a.bag();
    expect([selected.server, selected.open]).toEqual(['@me', GENERAL]);
    // Back to #general's address (the channel open) before the direct message opens.
    await a.go(`/c/${GENERAL}`);
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === DM || t.onScreen === DM)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.server, s.open, s.chan.name, s.onScreen, s.pendingOpen]).toEqual([before, GENERAL, 'general', GENERAL, '']);
    expect(s.dmsShown).toBe(false);
  } finally { await a.close(); }
}, 180_000);

test.skipIf(!built)('a link into the channel already open cancels a request still out', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.go(`/c/${HELP}`);
    // A link back into #general (open) before #help opens: newer, so #help never opens.
    await a.tap('#general');
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === HELP || t.onScreen === HELP)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.pendingOpen, s.gw.channel]).toEqual([GENERAL, 'general', '', GENERAL]);
    // Its address is #general's again, and a relaunch comes back to it.
    expect(new URL(a.page.url()).pathname).toBe(`/c/${GENERAL}`);
  } finally { await a.close(); }
}, 180_000);

test.skipIf(!built)('an address no route takes leaves the open channel where it is, still live', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.go('/nope');
    await a.data();
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.reading, s.gw.channel, s.onScreen]).toEqual([GENERAL, 'general', true, GENERAL, GENERAL]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('an address no route takes is no navigation: a request already out still opens, under its address', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.go(`/c/${HELP}`);
    await a.go('/nope');
    await a.data();
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.onScreen, new URL(a.page.url()).pathname]).toEqual([HELP, 'help', HELP, `/c/${HELP}`]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('an address no route takes on the list leaves the channel left closed', async () => {
  const a = await app();
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.back();
    await a.settle();
    expect((await a.bag()).reading).toBe(false);
    await a.go('/nope');
    await a.data();
    await a.settle();
    const s = await a.bag();
    expect([s.reading, new URL(a.page.url()).pathname]).toEqual([false, '/']);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('an encoded address for the open channel, over a request, returns to its canonical visit', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.go(`/c/${HELP}`);
    // #general's address with every digit percent-encoded: the router decodes it to #general.
    await a.go(`/c/${[...GENERAL].map(d => `%3${d}`).join('')}`);
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === HELP || t.onScreen === HELP)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.pendingOpen, new URL(a.page.url()).pathname]).toEqual([GENERAL, 'general', '', `/c/${GENERAL}`]);
    // One visit of #general, the canonical one: Back reaches the list, never #help.
    await a.back();
    await a.data();
    await a.settle();
    const t = await a.bag();
    expect([t.reading, t.open === HELP]).toEqual([false, false]);
  } finally { await a.close(); }
}, 120_000);

// The picture is a route over its channel (`/c/:channel/photo/:photo`, viewer.contract): Back closes it alone, a
// Forward to it is refused (a picture opens from its thumbnail only), and an address to another channel closes
// it with its channel, so a Back from there returns to the channel, not the picture.
test.skipIf(!built)('a picture is a route over its channel: Back closes it alone, an address to another channel closes it', async () => {
  const a = await app();
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('open-1562887009075210000-1562887009075399990');
    await a.settle();
    expect((await a.bag()).viewing).not.toBe('');
    await a.page.goBack();
    await a.data();
    await a.settle();
    let s = await a.bag();
    expect([s.open, s.viewing, s.reading]).toEqual([GENERAL, '', true]);
    await a.page.goForward();
    await a.data();
    await a.settle();
    s = await a.bag();
    expect([s.open, s.viewing, s.reading]).toEqual([GENERAL, '', true]);
    // An address to #help while #general's picture is open.
    await a.tap('open-1562887009075210000-1562887009075399990');
    await a.settle();
    expect((await a.bag()).viewing).not.toBe('');
    await a.go(`/c/${HELP}`);
    await a.data();
    await a.settle();
    s = await a.bag();
    expect([s.open, s.viewing, s.covered]).toEqual([HELP, '', false]);
    await a.page.goBack();
    await a.data();
    await a.settle();
    s = await a.bag();
    expect([s.open, s.viewing, s.reading]).toEqual([GENERAL, '', true]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('the address of the list closes the open channel when narrow, keeps it beside the list when wide', async () => {
  for (const [viewport, beside] of [[{ width: 420, height: 900 }, false], [{ width: 1100, height: 760 }, true]] as const) {
    const a = await app('/', viewport);
    try {
      await a.plus(1500, true);
      await a.settle();
      await a.tap('channel-general');
      await a.data();
      await a.settle();
      // `openChannel` stamps `now`: wide, the list's address leaves it as it was, so the channel was never
      // closed and opened again (which would drop a reply, an edit, attachments and the unread bar).
      const opened = (await a.bag()).now;
      await a.go('/');
      await a.data();
      await a.settle();
      const s = await a.bag();
      expect([viewport.width, s.wide, s.reading, s.open]).toEqual([viewport.width, beside, beside, GENERAL]);
      if (beside) expect(s.now).toBe(opened);
    } finally { await a.close(); }
  }
}, 240_000);

// The split marks the open channel's row (main 916dd42c22): the row pressed while its visit keeps that address,
// else the row the app marks `aria-current`. An address replaces the channel's visit beside the list, so the
// mark follows the channel the address opened, and Back's.
test.skipIf(!built)('wide, the marked row follows the channel an address or Back opens', async () => {
  const a = await app('/', { width: 1100, height: 760 });
  const marked = () => a.page.evaluate(() => [...document.querySelectorAll('[data-exact-split-row]')].map(el => {
    const v = (globalThis as any).exact.views; for (const [id, e] of v) if (e === el) return id; return -1; }));
  const markedIds = async () => {
    const ids = await marked(), tree = await a.page.evaluate(() => (globalThis as any).exact.agentSettled({ op: 'tree' }).then(JSON.stringify)).then(JSON.parse);
    return ids.map((id: number) => tree.nodes.find((n: any) => n.id === id)?.props?.testId ?? `view ${id}`);
  };
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    expect(await markedIds()).toEqual(['channel-general']);
    await a.go(`/c/${HELP}`);
    await a.data();
    await a.settle();
    expect([(await a.bag()).open, await markedIds()]).toEqual([HELP, ['channel-help']]);
    await a.page.goBack();
    await a.data();
    await a.settle();
    expect([(await a.bag()).open, await markedIds()]).toEqual([GENERAL, ['channel-general']]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('a link after an address change wins: Forward to #help, then a link to #random first', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    await a.tap('#help');
    await a.data();
    await a.settle();
    await a.page.goBack();
    await a.data();
    await a.settle();
    expect((await a.bag()).open).toBe(GENERAL);
    // Forward asks for #help; the link to #random, taken before it opens, is newer.
    await a.page.goForward();
    await a.tap('#random');
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === HELP || t.onScreen === HELP)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.pendingOpen]).toEqual([RANDOM, 'random', '']);
  } finally { await a.close(); }
}, 180_000);

test.skipIf(!built)('Back landing between a link\'s address write and its open wins: the link never opens', async () => {
  const a = await app();
  try {
    await generalWithLinks(a);
    // The link writes /c/help and asks for it; the browser's Back comes before the page's clock moves.
    await a.tap('#help');
    await a.page.goBack();
    const seen = await a.ticks(12);
    expect(seen.filter(t => t.open === HELP || t.onScreen === HELP)).toEqual([]);
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.onScreen, s.pendingOpen]).toEqual([GENERAL, 'general', GENERAL, '']);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('a typed channel address at launch opens that channel', async () => {
  const a = await app(`/c/${RANDOM}`);
  try {
    await a.plus(1500, true);
    await a.data();
    await a.settle();
    const s = await a.bag();
    expect([s.open, s.chan.name, s.onScreen]).toEqual([RANDOM, 'random', RANDOM]);
  } finally { await a.close(); }
}, 120_000);

// A send from history returns the reader to the end and settles there; on a host with no `scrollend`
// the settling still ends once the list is quiet, so reading back afterwards is away from the end again.
test.skipIf(!built)('with no scrollend, a send from history settles, and reading back is away again', async () => {
  const a = await app('/', undefined, { scrollend: false });
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('mark-read');
    const into = async (key: string) => {
      const tree = await a.step({ op: 'tree' });
      const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
      await a.step({ op: 'tap', id: transcript.id, into: { key } });
      await a.step({ op: 'clock', land: true });
      await a.data();
      await a.settle();
    };
    await into('1562755578936174328');
    expect((await a.bag()).away).toBe(true);
    const tree = await a.step({ op: 'tree' });
    const composer = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'composer');
    // The composer typed into as a person does (the page's own input events).
    const field = (await a.page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), composer.id)).asElement();
    await field.click();
    await a.page.keyboard.type('back to the end');
    await a.step({ op: 'clock', land: true });
    await a.settle();
    expect((await a.bag()).draft).toBe('back to the end');
    await a.tap('send');
    await a.data();
    await a.settle();
    // The list quiet well past the 200 ms the app waits for (`scrollQuiet`).
    await a.plus(1000, true);
    await a.settle();
    let s = await a.bag();
    expect([s.anchor, s.away, s.settling]).toEqual(['', false, false]);
    expect(await atEnd(a, s)).toEqual([true, true]);
    await into('1562755578936174328');
    s = await a.bag();
    expect([s.away, s.settling]).toEqual([true, false]);
  } finally { await a.close(); }
}, 120_000);

// The transcript's real geometry: scrolled to its content's end within a pixel, and the newest row (`message-<key>`,
// its element id) on screen.
async function atEnd(a: Awaited<ReturnType<typeof app>>, s: any) {
  const tree = await a.step({ op: 'tree' });
  const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
  return a.page.evaluate(([list, key]: any) => {
    let port = (globalThis as any).exact.views.get(list);
    while (port && port.scrollHeight <= port.clientHeight) port = port.firstElementChild;
    const r = document.getElementById(`message-${key}`)?.getBoundingClientRect(), p = port?.getBoundingClientRect();
    return [!!port && Math.abs(port.scrollHeight - port.clientHeight - port.scrollTop) <= 1,
      !!r && !!p && r.bottom > p.top && r.top < p.bottom];
  }, [transcript.id, s.lastKey]);
}

// A send from history, then the reader's slow wheel over the list: a step every 50 ms for 750 ms, Return sending
// at the fourth step, the list wheeled on from where the send puts it. The quiet path (`scrollQuiet`, 200 ms) is
// restarted by every scroll report, so it never acts while the list moves.
async function sendThenWheel(a: Awaited<ReturnType<typeof app>>) {
  await a.plus(1500, true);
  await a.settle();
  await a.tap('channel-general');
  await a.data();
  await a.settle();
  await a.tap('mark-read');
  const tree = await a.step({ op: 'tree' });
  const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
  await a.step({ op: 'tap', id: transcript.id, into: { key: '1562755578936174328' } });
  await a.step({ op: 'clock', land: true });
  await a.data();
  await a.settle();
  expect((await a.bag()).away).toBe(true);
  const composer = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'composer');
  const field = (await a.page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), composer.id)).asElement();
  await field.click();
  await a.page.keyboard.type('back to the end');
  await a.step({ op: 'clock', land: true });
  const list = (await a.page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), transcript.id)).asElement();
  const box = await list.boundingBox();
  const cdp = await a.page.context().newCDPSession(a.page);
  const seen: { settling: boolean, away: boolean, gap: number }[] = [];
  for (let i = 0; i < 15; i++) {
    // A wheel step as the agent's carrier sends one (Input.dispatchMouseEvent).
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: box.x + box.width / 2, y: box.y + box.height / 2, deltaX: 0, deltaY: -120 });
    if (i === 3) await a.page.keyboard.press('Enter');
    await a.plus(50, true);
    const s = await a.bag();
    seen.push({ settling: s.settling, away: s.away, gap: s.endGap });
  }
  // From the send's landing at the end, the list only moves on with the wheel: never put back while it moves.
  const landed = seen.findIndex((m, i) => i >= 3 && m.gap === 0);
  const moving = seen.slice(landed + 1);
  expect([landed >= 3, moving.length >= 8]).toEqual([true, true]);
  expect(moving.every((m, i) => i === 0 || m.gap >= moving[i - 1].gap)).toBe(true);
  expect(moving[moving.length - 1].gap).toBeGreaterThan(0);
  return moving;
}

test.skipIf(!built)('with no scrollend, a slow scroll after a send from history is not cut short', async () => {
  const a = await app('/', undefined, { scrollend: false });
  try {
    const moving = await sendThenWheel(a);
    // Settling the whole time it moves; once still, the list goes back to the end and settles there.
    expect(moving.map(m => m.settling)).toEqual(moving.map(() => true));
    await a.plus(1000, true);
    await a.settle();
    const s = await a.bag();
    expect([s.settling, s.away, s.anchor]).toEqual([false, false, '']);
    expect(await atEnd(a, s)).toEqual([true, true]);
  } finally { await a.close(); }
}, 120_000);

test.skipIf(!built)('with scrollend, a send settles at its landing\'s scrollend and the reader\'s scroll after it is theirs', async () => {
  const a = await app();
  try {
    const moving = await sendThenWheel(a);
    // The landing's scrollend found the list at the end: settled, and the wheel after it carries the reader off.
    expect(moving.map(m => m.settling)).toEqual(moving.map(() => false));
    await a.plus(1000, true);
    await a.settle();
    const s = await a.bag();
    expect([s.settling, s.away, s.endGap >= moving[moving.length - 1].gap]).toEqual([false, true, true]);
  } finally { await a.close(); }
}, 120_000);


// Where the host delivers scrollend it alone ends a send's settling, however long it holds it (here, held from the
// send on, as iOS does under a tracking finger): no quiet settles it, through a still stretch of 600 ms (three
// times the quiet) and a slow wheel carrying the list off the end; it settles after the held scrollend arrives.
test.skipIf(!built)('with scrollend, a send from history settles only at its scrollend, however late', async () => {
  const a = await app();
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('mark-read');
    const tree = await a.step({ op: 'tree' });
    const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
    await a.step({ op: 'tap', id: transcript.id, into: { key: '1562755578936174328' } });
    await a.step({ op: 'clock', land: true });
    await a.data();
    await a.settle();
    expect((await a.bag()).away).toBe(true);
    const composer = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'composer');
    const field = (await a.page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), composer.id)).asElement();
    await field.click();
    await a.page.keyboard.type('back to the end');
    await a.step({ op: 'clock', land: true });
    const list = (await a.page.evaluateHandle((v: number) => (globalThis as any).exact.views.get(v), transcript.id)).asElement();
    const box = await list.boundingBox();
    const cdp = await a.page.context().newCDPSession(a.page);
    const seen: { at: string, settling: boolean, away: boolean, gap: number, ends: number }[] = [];
    const look = async (at: string) => {
      const s = await a.bag();
      seen.push({ at, settling: s.settling, away: s.away, gap: s.endGap, ends: await a.page.evaluate(() => (globalThis as any).endsSeen) });
    };
    await a.page.evaluate(() => (globalThis as any).holdEnds());
    await a.page.keyboard.press('Enter');
    await a.data();
    await a.plus(50, true);
    await look('sent');
    await a.plus(600, true);
    await look('still');
    for (let i = 0; i < 8; i++) {
      await cdp.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: box.x + box.width / 2, y: box.y + box.height / 2, deltaX: 0, deltaY: -120 });
      await a.plus(50, true);
      await look(`wheel ${i}`);
    }
    await a.plus(600, true);
    await look('still again');
    const held = seen.length;
    await a.page.evaluate(() => (globalThis as any).releaseEnds());
    for (let i = 0; i < 10; i++) {
      await a.plus(100, true);
      await look(`released ${i}`);
    }
    await a.settle();
    const s = await a.bag();
    // While scrollend is held: settling throughout, never away, nothing delivered; the wheel carried the list off.
    expect(seen.slice(0, held).map(m => [m.at, m.settling, m.away])).toEqual(seen.slice(0, held).map(m => [m.at, true, false]));
    expect(seen[held - 1].gap).toBeGreaterThan(0);
    const before = seen[held - 1].ends;
    // Settled only after the held scrollend arrived, the list back at the end.
    const settled = seen.findIndex(m => !m.settling);
    expect([settled >= held, settled >= 0 && seen[settled].ends > before]).toEqual([true, true]);
    expect([s.settling, s.away]).toEqual([false, false]);
    expect(await atEnd(a, s)).toEqual([true, true]);
  } finally { await a.close(); }
}, 120_000);

// The fixture's `message` injection into #general: its id from the control door's answer, then the clock until
// the app has the row.
async function arrive(a: Awaited<ReturnType<typeof app>>, control: (body: object) => Promise<Response>) {
  const answer = await control({ what: 'message' });
  expect(answer.ok).toBe(true);
  const id = (await answer.json()).dispatches.find((d: any) => d.t === 'MESSAGE_CREATE').d.id as string;
  for (let i = 0; i < 20 && !(await a.bag()).win.rows.some((r: any) => r.id === id); i++) {
    await a.data();
    await a.plus(100, true);
  }
  await a.settle();
  await a.plus(1000, true);
  await a.data();
  await a.settle();
  expect((await a.bag()).win.rows.some((r: any) => r.id === id)).toBe(true);
  return id;
}

// A restore that gives up: the saved place's row was deleted while the reader was away, so the window around the
// place's anchor (here reaching the live end) cannot place it, and the list stays at its end. The reader is at the end,
// live: a message arriving there is followed and acknowledged, as at any end. Web only: the deletion is the fixture's
// own, by its control door, in a channel not on screen.
test.skipIf(!built)('a restore that gives up at the live end leaves the reader live: an arrival is followed and read', async () => {
  const a = await app();
  const control = (body: object) => fetch(`${fixtureOrigin}/control/inject`, { method: 'POST',
    headers: { 'content-type': 'application/json' }, body: JSON.stringify({ channel_id: GENERAL, ...body }) });
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('mark-read');
    const tree = await a.step({ op: 'tree' });
    const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
    await a.step({ op: 'tap', id: transcript.id, into: { key: '1562755578936174328' } });
    await a.step({ op: 'clock', land: true });
    await a.data();
    await a.settle();
    let s = await a.bag();
    const saved = s.topKey, ids = s.win.rows.map((r: any) => r.id);
    expect([s.away, saved, ids.indexOf(saved) >= 0]).toEqual([true, '1562755578936174328', true]);
    await a.back();
    await a.settle();
    // The saved row goes (the fixture deletes the message three above the one it is given).
    expect((await control({ what: 'delete-above', near: ids[ids.indexOf(saved) + 3] })).ok).toBe(true);
    await a.data();
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    s = await a.bag();
    // Given up: no target, the window the live tail's, the list at its end.
    expect([s.target, s.win.hasAfter, s.away, s.win.rows.some((r: any) => r.id === saved)]).toEqual(['', false, false, false]);
    expect(await atEnd(a, s)).toEqual([true, true]);
    // A message arrives at the end: its own row on screen, followed, and read up to it.
    const arrived = await arrive(a, control);
    s = await a.bag();
    expect([s.anchor, s.chan.unread, s.lastKey, s.chan.lastRead]).toEqual(['', 0, arrived, arrived]);
    expect(await atEnd(a, s)).toEqual([true, true]);
  } finally { await a.close(); }
}, 120_000);

// The other branch: a restore that gives up in a window of older history (`hasAfter`), the saved row deleted while the
// reader was away. The reader is in history, not live: the place's anchor stays, and the window's next older page
// (F1's lead) lands with the reader's row where it was.
test.skipIf(!built)('a restore that gives up in older history keeps its anchor, and the next page lands in place', async () => {
  const a = await app();
  const control = (body: object) => fetch(`${fixtureOrigin}/control/inject`, { method: 'POST',
    headers: { 'content-type': 'application/json' }, body: JSON.stringify({ channel_id: GENERAL, ...body }) });
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('mark-read');
    const tree = await a.step({ op: 'tree' });
    const transcript = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
    await a.step({ op: 'tap', id: transcript.id, into: { key: '1562624645306263268' } });
    await a.step({ op: 'clock', land: true });
    await a.settle();
    // A reply's original far back: the window is its own run around it, away from the live end.
    await a.tap('reply-to-1562624645306263268');
    await a.data();
    await a.settle();
    await a.plus(3000);
    await a.data();
    await a.settle();
    let s = await a.bag();
    const saved = s.topKey, anchor = s.anchor, ids = s.win.rows.map((r: any) => r.id);
    expect([s.away, s.win.hasAfter, ids.indexOf(saved) >= 3]).toEqual([true, true, true]);
    await a.back();
    await a.settle();
    expect((await control({ what: 'delete-above', near: ids[ids.indexOf(saved) + 3] })).ok).toBe(true);
    await a.data();
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    s = await a.bag();
    // Given up in older history: no target, the saved row gone, the anchor kept.
    expect([s.target, s.win.hasAfter, s.anchor, s.win.rows.some((r: any) => r.id === saved)]).toEqual(['', true, anchor, false]);
    const first = s.win.firstId, row = s.win.rows[1].key;
    // Reading back to the window's top asks its next older page; it lands, and the reader's row stays put.
    const list = (await a.step({ op: 'tree' })).nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
    await a.step({ op: 'tap', id: list.id, into: { key: row } });
    await a.step({ op: 'clock', land: true });
    await a.data();
    await a.settle();
    await a.plus(3000);
    await a.data();
    await a.settle();
    s = await a.bag();
    expect([s.win.firstId !== first, s.win.hasAfter, s.topKey]).toEqual([true, true, row]);
  } finally { await a.close(); }
}, 120_000);

// A give-up with the reader still in history: the open channel chosen again in the sidebar (wide) after its saved
// row was deleted while it stayed open. Its window reaches the live end, but the list was not remounted and the
// reader is where they were, off the end: the anchor stays, and an arrival is neither followed nor read.
test.skipIf(!built)('a restore given up in the open channel, reader off the end, keeps its anchor: an arrival waits', async () => {
  const a = await app('/', { width: 1100, height: 760 });
  const control = (body: object) => fetch(`${fixtureOrigin}/control/inject`, { method: 'POST',
    headers: { 'content-type': 'application/json' }, body: JSON.stringify({ channel_id: GENERAL, ...body }) });
  try {
    await a.plus(1500, true);
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    await a.tap('mark-read').catch(() => undefined);
    const tree = await a.step({ op: 'tree' });
    const list = tree.nodes.find((n: any) => !n.inactive && n.props?.testId === 'transcript');
    await a.step({ op: 'tap', id: list.id, into: { key: '1562755578936174328' } });
    await a.step({ op: 'clock', land: true });
    await a.data();
    await a.settle();
    // Another channel beside the list, then #general again: its place restored.
    await a.tap('channel-random');
    await a.data();
    await a.settle();
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    let s = await a.bag();
    const saved = '1562755578936174328', anchor = s.anchor, ids = s.win.rows.map((r: any) => r.id);
    expect([s.topKey, s.away, s.target]).toEqual([saved, true, '']);
    // The saved row goes while the channel is open.
    expect((await control({ what: 'delete-above', near: ids[ids.indexOf(saved) + 3] })).ok).toBe(true);
    await a.data();
    await a.settle();
    // #general chosen again in the sidebar: the saved place asked again, its row gone, the restore gives up in place.
    await a.tap('channel-general');
    await a.data();
    await a.settle();
    s = await a.bag();
    expect([s.chan.name, s.target, s.win.hasAfter, s.win.rows.some((r: any) => r.id === saved), s.endGap > 0.5])
      .toEqual(['general', '', false, false, true]);
    expect(s.anchor).toBe(anchor);
    const top = s.topKey, unread = s.chan.unread;
    // An arrival at the end: not followed (the reader's row stays), not read (counted).
    const arrived = await arrive(a, control);
    s = await a.bag();
    expect([s.anchor, s.topKey, s.chan.unread, s.lastKey]).toEqual([anchor, top, unread + 1, arrived]);
  } finally { await a.close(); }
}, 120_000);
