import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs expose`.
for (const appearance of ['system', 'light', 'dark']) {
  test(`Expose text remains readable with ${appearance} appearance`, async () => {
    const session = await open({ host: 'web', app: 'expose', failFetch: 'https://api.open-meteo.com' });
    try {
      await session.clock('+1000');
      await session.clock('data');
      await session.tap('lock-unlock');
      await session.tap('app-settings');
      await session.tap('row-display');
      const selection = await session.type('appearance', appearance);
      expect(selection.error).toBeUndefined();
      const failures = [];
      const check = async (screen) => {
        await session.tree(screen);
        const result = await checkOp(session, [appearance === 'system' ? 'both' : appearance]);
        expect(result.check.map(report => report.scheme))
          .toEqual(appearance === 'system' ? ['light', 'dark'] : [appearance]);
        for (const report of result.check) {
          failures.push(...report.findings.filter(f => f.kind === 'contrast')
            .map(f => ({ screen, scheme: report.scheme, ...f })));
        }
      };
      await session.tap('status-time');
      await check('lock-screen');
      await session.tap('lock-unlock');
      await check('home-screen');
      await session.tap('app-messages');
      await check('inbox-screen');
      await session.tap('thread-nico');
      await check('thread-screen');
      await session.tap('thread-back');
      await session.tap('thread-expose');
      await check('thread-screen');
      await session.tap('home-bar');
      await session.tap('app-reader');
      await check('reader-screen');
      await session.tap('article-loud-clock');
      await check('article-screen');
      await session.tap('home-bar');
      await session.tap('app-settings');
      await check('settings-screen');
      expect(failures).toEqual([]);
    } finally {
      await session.close();
    }
  }, 30000);
}
