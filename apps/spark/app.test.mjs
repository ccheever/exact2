import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';
import { answer, startNet } from '../../host/web-js/conformance/net.mjs';

// Build first with `bun host/web/build.mjs spark`.
test('the match backdrop covers the page throughout its content entrance', async () => {
  // This source check locks the attribute only: the web omits status-bar
  // properties. The browser checks the backdrop; native ink still needs iOS.
  const declarations = readFileSync(new URL('./app.contract', import.meta.url), 'utf8')
    .split('\n').filter(line => line.includes('status-bar-style='));
  expect(declarations).toHaveLength(1);
  expect(declarations[0]).toContain('testId="match-overlay"');
  expect(declarations[0]).toContain('status-bar-style="auto"');
  const net = await startNet();
  let session;
  try {
    session = await open({ host: 'web', app: 'spark', size: [420, 900],
      env: { ...net.env, EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    await session.clock('settle');
    const overlayMounted = async () => (await session.op({ op: 'tree' })).nodes
      .some(node => node.props?.testId === 'match-overlay');
    for (const scheme of ['light', 'dark']) {
      await session.prefer({ 'prefers-color-scheme': scheme });
      expect(await overlayMounted()).toBe(false);
      await session.tap('btn-like');
      expect((await session.state()).slots.matched).toBe(0);
      // Sample mounting, the delay and the content entrance before settling.
      for (const step of [0, 160, 290, 'settle']) {
        if (step) await session.clock(step === 'settle' ? step : `+${step}`);
        const paint = await session.carrier.evaluate(`(() => {
          const overlay = document.querySelector('[data-testid="match-overlay"]');
          const style = getComputedStyle(overlay);
          const rect = overlay.getBoundingClientRect();
          let opacity = 1;
          for (let node = overlay; node; node = node.parentElement) {
            opacity *= Number(getComputedStyle(node).opacity);
          }
          return { opacity, background: style.backgroundImage,
            coversViewport: rect.top <= 0 && rect.left <= 0 &&
              rect.right >= innerWidth && rect.bottom >= innerHeight };
        })()`);
        expect(paint.opacity).toBe(1);
        expect(paint.background).toContain('linear-gradient(');
        expect(paint.coversViewport).toBe(true);
      }
      const overlay = (await session.layout('match-overlay')).node;
      expect(overlay.style.color_scheme.computed).toBe('dark');
      expect(await overlayMounted()).toBe(true);
      await session.tap('keep-swiping');
      await session.clock('settle');
      expect((await session.state()).slots.matched).toBe(-1);
      expect(await overlayMounted()).toBe(false);
      await session.tap('btn-undo');
      await session.clock('settle');
    }
  } finally {
    try { await session?.close(); } finally { await net.close(); }
  }
}, 30000);

for (const loaded of [true, false]) {
  test(`Spark contrast with ${loaded ? 'fixture portraits' : 'failed portraits'}`, async () => {
    const net = await startNet(undefined, loaded ? {} : {
      'randomuser.me': () => ({ status: 404, body: 'Portrait unavailable' }),
    });
    let session;
    try {
      expect(answer('randomuser.me', 'GET', '/api/portraits/women/1.jpg').status).toBe(200);
      session = await open({ host: 'web', app: 'spark', size: [420, 900],
        env: { ...net.env, EXACT_AGENT_EPHEMERAL_PORT: '1' } });
      const failures = [];
      const check = async (screen, overlay = false) => {
        await session.tree(screen);
        for (const scheme of ['light', 'dark']) {
          await session.prefer({ 'prefers-color-scheme': scheme });
          const observation = await session.op({ op: 'check' });
          expect(observation.scheme).toBe(scheme);
          if (overlay) {
            for (const text of ["It's a match!", 'You and Maya like each other.', 'Keep swiping']) {
              expect(observation.items.filter(item => item.text === text)).toHaveLength(1);
            }
          }
          if (screen.startsWith('stamp-')) {
            const label = { 'stamp-like': 'LIKE', 'stamp-nope': 'NOPE', 'stamp-super': 'SUPER LIKE' }[screen];
            const stamp = observation.items.find(item => item.text === label && item.bg);
            expect(stamp.bg).toHaveLength(3);
            expect(ratio(stamp.fg, stamp.bg)).toBeGreaterThanOrEqual(3);
          }
          if (screen === 'empty-deck') {
            const hint = observation.items.find(item => item.text?.startsWith('Check back later'));
            expect(hint.bg).toHaveLength(3);
            const alpha = hint.fg[3] ?? 1;
            const ink = hint.fg.slice(0, 3).map((channel, i) => channel * alpha + hint.bg[i] * (1 - alpha));
            // A role's lower checker floor must not hide a regression in this hint.
            expect(ratio(ink, hint.bg)).toBeGreaterThanOrEqual(4.5);
          }
        }
        const result = await checkOp(session, ['both']);
        expect(result.check.map(report => report.scheme)).toEqual(['light', 'dark']);
        for (const report of result.check) {
          const contrast = [];
          for (const finding of report.findings.filter(f => f.kind === 'contrast')) {
            if (screen.startsWith('stamp-')) {
              // The checker can sample Maya's white letters as the background of
              // Jordan's age beneath her card. Exclude only text wholly covered by it.
              const covered = await session.carrier.evaluate(`(() => {
                const top = document.querySelector('[data-testid="card-top"]');
                const element = exact.views.get(${finding.id});
                if (!element || top.contains(element)) return false;
                const { left, top: y, right, bottom } = element.getBoundingClientRect();
                return [[left + 1, y + 1], [right - 1, y + 1], [left + 1, bottom - 1], [right - 1, bottom - 1]]
                  .every(([x, y]) => top.contains(document.elementFromPoint(x, y)));
              })()`);
              if (covered) continue;
            }
            contrast.push(finding);
          }
          // This authored dialog is translucent and leaves the deck mounted.
          const own = f => !overlay || ["It's a match!", 'You and Maya like each other.', 'Keep swiping'].includes(f.text);
          failures.push(...contrast.filter(own).map(f => ({ screen, scheme: report.scheme, ...f })));
          if (overlay && contrast.some(f => !own(f))) {
            console.log('Behind match overlay', report.scheme, contrast.filter(f => !own(f)));
          }
        }
      };
      const portrait = async selector => {
        const decoded = await session.carrier.evaluate(`(async () => {
          const image = document.querySelector(${JSON.stringify(selector)});
          if (!image) throw new Error('Missing portrait');
          await image.decode().catch(() => {});
          return image.complete && image.naturalWidth > 0;
        })()`);
        expect(decoded).toBe(loaded);
      };
      await session.clock('settle');
      expect((await session.state()).slots.index).toBe(0);
      await portrait('[data-testid="card-top"] img');
      await check('card-top');
      for (const [stamp, dx, dy] of [['like', 100, 0], ['nope', -100, 0], ['super', 0, -100]]) {
        await session.tap('photo-left', { down: true });
        await session.pointer('move', { dx, dy });
        expect((await session.state()).slots.dragging).toBe(true);
        expect(await session.carrier.evaluate(`Number(getComputedStyle(document.querySelector('[data-testid="stamp-${stamp}"]')).opacity)`))
          .toBeGreaterThan(0.9);
        await check(`stamp-${stamp}`);
        await session.pointer('move', { dx: -dx, dy: -dy, ms: 250 });
        await session.pointer('up');
        await session.clock('settle');
        expect((await session.state()).slots.index).toBe(0);
      }
      await session.tap('btn-like');
      await session.clock('settle');
      expect((await session.state()).slots.matched).toBe(0);
      await portrait('[data-testid="match-photo"]');
      await check('match-overlay', true);
      await session.tap('keep-swiping');
      expect((await session.state()).slots.matched).toBe(-1);
      const count = (await session.state()).derives.count;
      for (let index = 1; index < count; index++) {
        await session.tap('btn-nope');
        await session.clock('settle');
      }
      expect((await session.state()).slots.index).toBe(count);
      await check('empty-deck');
      await session.tap('start-over');
      await session.clock('settle');
      expect((await session.state()).slots.index).toBe(0);
      await session.tree('card-top');
      expect(net.unanswered).toEqual([]);
      expect(failures).toEqual([]);
    } finally {
      try { await session?.close(); } finally { await net.close(); }
    }
  }, 30000);
}
