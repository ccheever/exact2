import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp } from '../../scripts/agent-check.mjs';
import { startNet } from '../../host/web-js/conformance/net.mjs';

// Build first with `bun host/web/build.mjs weatherlight --render js`.
for (const offline of [false, true]) {
  test(`Weatherlight contrast in both schemes with ${offline ? 'failed' : 'loaded'} weather`, async () => {
    const net = await startNet();
    let session;
    try {
      session = await open({ host: 'web', app: 'weatherlight', size: [1180, 1000],
        env: { ...net.env, EXACT_AGENT_EPHEMERAL_PORT: '1' },
        ...(offline ? { failFetch: 'https://api.open-meteo.com' } : {}) });
      const failures = [];
      const check = async screen => {
        const result = await checkOp(session, ['both']);
        expect(result.check.map(report => report.scheme)).toEqual(['light', 'dark']);
        for (const report of result.check) {
          failures.push(...report.findings.filter(f => f.kind === 'contrast')
            .map(f => ({ screen, scheme: report.scheme, ...f })));
        }
      };
      expect((await session.state()).resources.forecast.ready).toBe(false);
      await check('empty forecast');
      await session.tap('change-city');
      expect((await session.state()).slots.searching).toBe(true);
      await session.tree('search-panel');
      await check('search');
      await session.tap('close-search');
      await session.clock('+1000');
      await session.clock('settle');
      expect((await session.state()).resources.forecast.ready).toBe(!offline);
      if (offline) {
        expect((await session.state()).resources.forecast.message).not.toBe('');
        await session.tree('network-notice');
        await check('network notice');
      } else {
        expect((await session.state()).resources.outlook.hours).toHaveLength(24);
        await check('loaded forecast');
        const hour = (await session.state()).resources.outlook.hours[0].id;
        await session.tap(`hour-${hour}`);
        await session.clock('settle');
        expect((await session.state()).slots.selected).toBe(hour);
        await session.tree('back-now');
        await check('selected hour');
      }
      await session.prefer({ 'permission.geolocation': 'denied' });
      await session.clock('settle');
      expect((await session.state()).resources.location.state).toBe('denied');
      await session.tree('location-denied');
      await check('location denied');
      expect(net.unanswered).toEqual([]);
      expect(failures).toEqual([]);
    } finally {
      await session?.close();
      await net.close();
    }
  }, 30000);
}
