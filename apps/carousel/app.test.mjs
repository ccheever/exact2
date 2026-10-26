import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs carousel`.
test('strip and feed chrome follow the colour scheme while content cards keep their ink', async () => {
  const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
  const session = await open({ host: 'web', app: 'carousel', size: [420, 900],
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    await session.clock('settle');
    const failures = [];
    const require = (condition, detail) => { if (!condition) failures.push(detail); };
    const readPaint = page => {
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
      const root = node('carousel');
      const caption = page === 'strip' ? root.lastElementChild : node('post-1').lastElementChild;
      return {
        page: paint(root), caption: paint(caption),
        buttons: Object.fromEntries(['far', 'feed-page', 'feed-far', 'feed-top', 'eager']
          .map(id => [id, paint(node(id), node(id).firstElementChild)])),
        cards: Array.from({ length: 6 }, (_, n) => {
          const card = node(page === 'strip' ? `card-${n}` : `card-0-${n}`);
          return paint(card, card.firstElementChild);
        }),
        divider: page === 'feed'
          ? over(rgba(getComputedStyle(node('post-1')).borderBottomColor), background(node('post-1')))
          : null,
      };
    };
    for (const page of ['strip', 'feed']) {
      if (page === 'feed') await session.tap('feed-page');
      expect((await session.state()).slots.page).toBe(page);
      const paints = [];
      for (const scheme of ['light', 'dark']) {
        await session.prefer({ 'prefers-color-scheme': scheme });
        await session.clock('settle');
        const paint = await session.carrier.evaluate(`(${readPaint.toString()})(${JSON.stringify(page)})`);
        paints.push(paint);
        for (const [name, surface] of Object.entries({ page: paint.page, ...paint.buttons })) {
          require(scheme === 'dark' ? Math.max(...surface.fill) < 65 : Math.min(...surface.fill) > 200,
            { page, scheme, name, expected: `${scheme} surface`, surface });
          require(ratio(surface.ink, surface.fill) >= 4.5,
            { page, scheme, name, expected: 'readable ink', surface });
        }
        // The contrast checker allows 3:1 for secondary labels; primary ink needs 4.5:1.
        require(ratio(paint.caption.ink, paint.caption.fill) >= 3,
          { page, scheme, expected: 'readable caption', caption: paint.caption });
        for (const card of paint.cards) {
          expect(card.ink).toEqual([255, 255, 255]);
          expect(ratio(card.ink, card.fill)).toBeGreaterThanOrEqual(4.5);
        }
        const launch = scheme === 'light' ? manifest.background_color : manifest.background_color_dark;
        const surface = '#' + paint.page.fill.map(n => n.toString(16).padStart(2, '0')).join('');
        require(launch === surface, { page, scheme, expected: 'launch matches page', launch, surface });
        const checked = await checkOp(session, [scheme]);
        expect(checked.check.map(report => report.scheme)).toEqual([scheme]);
        failures.push(...checked.check[0].findings.filter(f => f.kind === 'contrast')
          .map(f => ({ page, scheme, ...f })));
      }
      for (const property of ['page', 'caption', 'buttons', ...(page === 'feed' ? ['divider'] : [])]) {
        require(JSON.stringify(paints[0][property]) !== JSON.stringify(paints[1][property]),
          { page, expected: `${property} changes with scheme`, paint: paints[0][property] });
      }
      require(JSON.stringify(paints[0].caption.ink) !== JSON.stringify(paints[1].caption.ink),
        { page, expected: 'caption ink changes with scheme', paints });
      expect(paints[0].cards).toEqual(paints[1].cards);
    }
    expect(failures).toEqual([]);
  } finally {
    await session.close();
  }
}, 30000);
