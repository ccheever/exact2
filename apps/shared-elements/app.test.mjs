import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs shared-elements`.
test('surfaces adapt, selections remain distinct and photo viewers stay dark', async () => {
  const session = await open({ host: 'web', app: 'shared-elements', size: [420, 900],
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    await session.clock('data');
    const resources = (await session.state()).resources;
    expect(resources.photos).toHaveLength(12);
    expect(resources.rows).toHaveLength(200);
    expect(resources.cards).toHaveLength(3);
    expect(resources.tasks).toHaveLength(5);
    const failures = [];
    const paint = async (id, page = false) => {
      const read = (id, page) => {
        const node = document.querySelector(`[data-testid="${id}"]`);
        if (!node) throw new Error(`Missing ${id}`);
        const element = page ? node.firstElementChild : id === 'zoom-large' ? node.parentElement : node;
        const rgba = value => value.match(/[\d.]+/g).map(Number);
        const background = el => {
          if (!el) return [255, 255, 255];
          const colour = rgba(getComputedStyle(el).backgroundColor);
          const alpha = colour[3] ?? 1, behind = background(el.parentElement);
          return colour.slice(0, 3).map((channel, i) => channel * alpha + behind[i] * (1 - alpha));
        };
        const style = getComputedStyle(element);
        return { fill: background(element), ink: rgba(style.color), scheme: style.colorScheme };
      };
      return session.carrier.evaluate(`(${read.toString()})(${JSON.stringify(id)}, ${page})`);
    };
    const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
    const contrast = ({ ink, fill }) => {
      const alpha = ink[3] ?? 1;
      return ratio(ink.slice(0, 3).map((channel, i) =>
        channel * alpha + fill[i] * (1 - alpha)), fill);
    };
    const check = async (screen, { tab, surface = 'app', viewer = false } = {}) => {
      await session.clock('settle');
      await session.tree(screen);
      if (tab) expect((await session.state()).slots.tab).toBe(tab);
      const paints = [];
      for (const scheme of ['light', 'dark']) {
        await session.prefer({ 'prefers-color-scheme': scheme });
        await session.clock('settle');
        const observation = await session.op({ op: 'check' });
        expect(observation.scheme).toBe(scheme);
        const result = await checkOp(session, [scheme]);
        expect(result.check.map(report => report.scheme)).toEqual([scheme]);
        failures.push(...result.check[0].findings.filter(f => f.kind === 'contrast')
          .map(f => ({ screen, scheme, ...f })));
        const colours = await paint(surface, surface === 'app');
        paints.push(colours);
        expect(contrast(colours)).toBeGreaterThanOrEqual(4.5);
        if (viewer) {
          if (colours.scheme !== 'dark' || Math.max(...colours.fill) > 50 ||
              Math.min(...colours.ink.slice(0, 3)) < 200) {
            failures.push({ screen, scheme, expected: 'dark viewer with light ink', colours });
          }
        } else if (tab) {
          const selected = await paint(`tab-${tab}`);
          expect(contrast(selected)).toBeGreaterThanOrEqual(4.5);
          const other = await paint(`tab-${tab === 'grid' ? 'list' : 'grid'}`);
          if (ratio(selected.fill, other.fill) < 1.5) {
            failures.push({ screen, scheme, expected: 'distinct selected tab', selected, other });
          }
        }
      }
      if (!viewer) {
        for (const property of ['fill', 'ink']) {
          if (same(paints[0][property], paints[1][property])) {
            failures.push({ screen, expected: `${property} changes with scheme`, paints });
          }
        }
      }
      return paints;
    };
    const tap = async id => {
      await session.tap(id);
      await session.clock('settle');
    };
    await check('open-p1', { tab: 'grid' });
    await tap('open-p1');
    expect((await session.state()).slots.open).toBe(1);
    await check('large', { surface: 'viewer', viewer: true });
    await tap('next');
    expect((await session.state()).slots.open).toBe(2);
    await tap('close');
    await session.tree('thumb-p2');

    await tap('tab-list');
    await check('row-p1', { tab: 'list' });
    await tap('row-p1');
    expect((await session.state()).slots.openRow).toBe(1);
    await check('large', { surface: 'viewer', viewer: true });
    await tap('ahead-n');
    expect((await session.state()).slots.openRow).toBe(26);
    await tap('close');
    await session.tree('rowthumb-p26');

    await tap('tab-cards');
    await check('card-c1', { tab: 'cards' });
    const cardSurfaces = [];
    for (const id of ['c1', 'c2', 'c3']) {
      const source = await check(`card-${id}`, { surface: `card-${id}` });
      cardSurfaces.push(source);
      await tap(`card-${id}`);
      expect((await session.state()).slots.card).toBe(id);
      const destination = await check('card-detail', { surface: 'card-detail' });
      expect(destination.map(p => p.fill)).toEqual(source.map(p => p.fill));
      await tap('close-card');
      expect((await session.state()).slots.card).toBe('');
    }

    for (let scheme = 0; scheme < 2; scheme++) {
      expect(new Set(cardSurfaces.map(pair => JSON.stringify(pair[scheme].fill))).size).toBe(3);
    }

    await tap('tab-tasks');
    await check('task-t1', { tab: 'tasks' });
    const unfinished = await check('task-t1', { surface: 'task-t1' });
    await tap('task-t1');
    expect((await session.state()).slots.done).toBe(1);
    const finished = await check('task-t1', { surface: 'task-t1' });
    for (let i = 0; i < 2; i++) expect(finished[i].fill).not.toEqual(unfinished[i].fill);
    await tap('task-t1');
    expect((await session.state()).slots.done).toBe(0);

    await tap('tab-zoom');
    await check('zoom-open-p1', { tab: 'zoom' });
    await tap('zoom-open-p1');
    expect((await session.state()).derives.current.name).toBe('photo');
    await check('zoom-large', { surface: 'zoom-large', viewer: true });
    // Check the close control's ink as well as the surrounding media surface.
    for (const scheme of ['light', 'dark']) {
      await session.prefer({ 'prefers-color-scheme': scheme });
      const close = await paint('zoom-close');
      expect(close.scheme).toBe('dark');
      expect(contrast(close)).toBeGreaterThanOrEqual(4.5);
    }
    await tap('zoom-close');
    expect((await session.state()).derives.current.name).toBe('home');
    await session.tree('zoom-open-p1');
    expect(failures).toEqual([]);
  } finally {
    await session.close();
  }
}, 30000);


test('launch colours and the dark empty web shell match the first frame', async () => {
  const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
  const shell = readFileSync(new URL('../../host/web/dist/index.html', import.meta.url), 'utf8');
  const browser = await suiteBrowser();
  const session = await open({ host: 'web', app: 'shared-elements',
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    const failures = [];
    for (const scheme of ['light', 'dark']) {
      await session.prefer({ 'prefers-color-scheme': scheme });
      const firstFrame = await session.carrier.evaluate(`getComputedStyle(
        document.querySelector('[data-testid="app"]').firstElementChild).backgroundColor`);
      const hex = '#' + firstFrame.match(/\d+/g).map(n => Number(n).toString(16).padStart(2, '0')).join('');
      const launch = scheme === 'light' ? manifest.background_color : manifest.background_color_dark;
      if (launch !== hex) failures.push({ scheme, launch, firstFrame: hex });
      if (scheme === 'dark') {
        // Disable scripts to observe the built shell before any app nodes mount.
        const page = await browser.newPage({ javaScriptEnabled: false, colorScheme: scheme });
        await page.setContent(shell);
        expect(await page.locator('#exact-root').evaluate(root => root.childNodes.length)).toBe(0);
        const empty = await page.evaluate(() => getComputedStyle(document.documentElement).backgroundColor);
        if (empty !== firstFrame) failures.push({ scheme, empty, firstFrame });
      }
    }
    expect(failures).toEqual([]);
  } finally {
    try { await session.close(); } finally { await browser.close(); }
  }
}, 30000);
