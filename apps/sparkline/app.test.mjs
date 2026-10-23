import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs sparkline`.
test('market chrome follows the scheme and coin labels remain readable', async () => {
  const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
  const session = await open({ host: 'web', app: 'sparkline', size: [420, 900],
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    await session.tap('freeze');
    expect((await session.state()).slots.frozen).toBe(true);
    const readPaint = () => {
      const node = id => {
        const element = document.querySelector(`[data-testid="${id}"]`);
        if (!element) throw new Error(`Missing ${id}`);
        return element;
      };
      const rgba = value => value.match(/[\d.]+/g).map(Number);
      const over = (colour, behind) => colour.slice(0, 3).map((channel, i) =>
        channel * (colour[3] ?? 1) + behind[i] * (1 - (colour[3] ?? 1)));
      const background = element => element
        ? over(rgba(getComputedStyle(element).backgroundColor), background(element.parentElement))
        : [255, 255, 255];
      const paint = (element, label = element) => {
        const fill = background(element);
        return { fill, ink: over(rgba(getComputedStyle(label).color), fill) };
      };
      const root = node('sparkline');
      return {
        page: paint(root),
        buttons: ['tick', 'freeze'].map(id => paint(node(id), node(id).firstElementChild)),
        divider: over(rgba(getComputedStyle(root.firstElementChild).borderBottomColor), background(root)),
        coins: Array.from({ length: 6 }, (_, i) => {
          const row = node(`row-c${i}`);
          const pill = row.lastElementChild.lastElementChild;
          return {
            avatar: paint(row.firstElementChild, row.firstElementChild.firstElementChild),
            ticker: paint(row.children[1].lastElementChild),
            pill: paint(pill), up: pill.textContent.startsWith('+'),
            chart: paint(node(`chart-c${i}`)),
            points: node(`line-c${i}`).getAttribute('points'),
          };
        }),
      };
    };
    const failures = [];
    const require = (condition, detail) => { if (!condition) failures.push(detail); };
    const paints = [];
    for (const scheme of ['light', 'dark']) {
      await session.prefer({ 'prefers-color-scheme': scheme });
      await session.clock('settle');
      const paint = await session.carrier.evaluate(`(${readPaint.toString()})()`);
      paints.push(paint);
      for (const [name, surface] of Object.entries({ page: paint.page, ...paint.buttons })) {
        require(scheme === 'dark' ? Math.max(...surface.fill) < 65 : Math.min(...surface.fill) > 200,
          { scheme, name, expected: `${scheme} surface`, surface });
        require(ratio(surface.ink, surface.fill) >= 4.5,
          { scheme, name, expected: 'readable ink', surface });
      }
      expect(new Set(paint.coins.map(c => c.up)).size).toBe(2);
      expect(new Set(paint.coins.map(c => JSON.stringify(c.avatar.fill))).size).toBe(6);
      for (const [i, coin] of paint.coins.entries()) {
        for (const [name, minimum] of [['avatar', 4.5], ['pill', 4.5], ['ticker', 3], ['chart', 3]]) {
          require(ratio(coin[name].ink, coin[name].fill) >= minimum,
            { scheme, coin: i, name, expected: `contrast >= ${minimum}`, paint: coin[name] });
        }
      }
      const launch = scheme === 'light' ? manifest.background_color : manifest.background_color_dark;
      const surface = '#' + paint.page.fill.map(n => n.toString(16).padStart(2, '0')).join('');
      require(launch === surface, { scheme, expected: 'launch matches page', launch, surface });
      const checked = await checkOp(session, [scheme]);
      expect(checked.check.map(report => report.scheme)).toEqual([scheme]);
      failures.push(...checked.check[0].findings.filter(f => f.kind === 'contrast')
        .map(f => ({ scheme, ...f })));
    }
    for (const property of ['page', 'buttons', 'divider']) {
      require(JSON.stringify(paints[0][property]) !== JSON.stringify(paints[1][property]),
        { expected: `${property} changes with scheme` });
    }
    require(JSON.stringify(paints[0].coins[0].ticker.ink) !== JSON.stringify(paints[1].coins[0].ticker.ink),
      { expected: 'ticker ink changes with scheme' });
    expect(paints[0].coins.map(c => c.avatar)).toEqual(paints[1].coins.map(c => c.avatar));
    await session.tap('tick');
    await session.clock('settle');
    expect((await session.state()).slots.version).toBe(1);
    const ticked = await session.carrier.evaluate(`(${readPaint.toString()})()`);
    expect(ticked.coins[0].points).not.toBe(paints[1].coins[0].points);
    expect(ticked.coins[1].points).toBe(paints[1].coins[1].points);
    await session.tap('freeze');
    expect((await session.state()).slots.frozen).toBe(false);
    expect(failures).toEqual([]);
  } finally {
    await session.close();
  }
}, 30000);
