import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';
import { startNet } from '../../host/web-js/conformance/net.mjs';

test('location request failures remain visible when permission status has not changed', async () => {
  const net = await startNet();
  let session;
  try {
    session = await open({ host: 'web', app: 'weatherlight',
      env: { ...net.env, EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    await session.prefer({ 'permission.geolocation': 'granted' });
    for (const code of ['PERMISSION_DENIED', 'POSITION_UNAVAILABLE']) {
      await session.prefer({ extension: `geolocation.watchPosition=${JSON.stringify({ error: code })}` });
      await session.tap('use-location');
      await session.clock('settle');
      expect((await session.state()).resources.location.state).toBe('granted');
      const message = await session.carrier.evaluate('document.querySelector(\'[data-testid="location-error"]\')?.textContent');
      expect(message).toContain(code === 'PERMISSION_DENIED' ? 'Location access was denied' : "Couldn't get your location");
      await session.tap('city-london');
      await session.clock('settle');
      expect(await session.carrier.evaluate('document.querySelector(\'[data-testid="location-error"]\') === null')).toBe(true);
    }
    await session.prefer({ extension: 'geolocation.watchPosition={"coords":{"latitude":37.7749,"longitude":-122.4194,"accuracy":50}}' });
    await session.tap('use-location');
    await session.clock('settle');
    expect((await session.state()).slots.following).toBe(true);
    expect(await session.carrier.evaluate('document.querySelector(\'[data-testid="location-error"]\') === null')).toBe(true);
  } finally {
    await session?.close();
    await net.close();
  }
}, 30000);

// These controls have flat fills. Include every translucent ancestor when
// comparing a selection with its track; comparing CSS colour strings misses it.
async function backgrounds(session, ids) {
  const read = ids => {
    const painted = element => {
      if (!element) return [255, 255, 255];
      const colour = getComputedStyle(element).backgroundColor.match(/[\d.]+/g).map(Number);
      const alpha = colour[3] ?? 1, behind = painted(element.parentElement);
      return colour.slice(0, 3).map((channel, i) => channel * alpha + behind[i] * (1 - alpha));
    };
    return Object.fromEntries(ids.map(id => {
      const element = document.querySelector(`[data-testid="${id}"]`);
      return [id, { fill: painted(element), behind: painted(element.parentElement) }];
    }));
  };
  return session.carrier.evaluate(`(${read.toString()})(${JSON.stringify(ids)})`);
}

test('selected units and sibling controls stand out in both schemes', async () => {
  const net = await startNet();
  let session;
  try {
    session = await open({ host: 'web', app: 'weatherlight', size: [1180, 1000],
      env: { ...net.env, EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    await session.prefer({ 'permission.geolocation': 'granted',
      extension: 'geolocation.watchPosition={"coords":{"latitude":37.7749,"longitude":-122.4194,"accuracy":50}}' });
    await session.clock('+1000');
    await session.clock('settle');
    expect((await session.state()).resources.forecast.ready).toBe(true);
    const prominence = colour => ratio(colour.fill, colour.behind);
    const failures = [];
    for (const scheme of ['light', 'dark']) {
      await session.prefer({ 'prefers-color-scheme': scheme });
      expect((await session.op({ op: 'check' })).scheme).toBe(scheme);
      for (const units of ['F', 'C']) {
        await session.tap(`unit-${units.toLowerCase()}`);
        await session.clock('settle');
        expect((await session.state()).slots.units).toBe(units);
        const colours = await backgrounds(session, ['unit-f', 'unit-c']);
        const selected = prominence(colours[`unit-${units.toLowerCase()}`]);
        const inactive = prominence(colours[units === 'F' ? 'unit-c' : 'unit-f']);
        if (selected < 1.2 || selected <= inactive + 0.05) failures.push({ scheme, units, selected, inactive });
        const checked = await checkOp(session, [scheme]);
        expect(checked.check[0].scheme).toBe(scheme);
        expect(checked.check[0].findings.filter(f => f.kind === 'contrast')).toEqual([]);
      }
      const cities = ['city-sf', 'city-london', 'city-tokyo', 'city-reykjavik'];
      for (const city of cities) {
        await session.tap(city);
        await session.clock('settle');
        const colours = await backgrounds(session, cities);
        for (const other of cities.filter(id => id !== city)) {
          expect(prominence(colours[city])).toBeGreaterThan(prominence(colours[other]) + 0.05);
        }
      }
      const inactive = (await backgrounds(session, ['use-location']))['use-location'];
      await session.tap('use-location');
      await session.clock('settle');
      expect((await session.state()).slots.following).toBe(true);
      const following = (await backgrounds(session, ['use-location']))['use-location'];
      expect(prominence(following)).toBeGreaterThan(prominence(inactive) + 0.05);
      const hour = `hour-${(await session.state()).resources.outlook.hours[0].id}`;
      for (const selected of [hour, 'hour-now']) {
        await session.tap(selected);
        await session.clock('settle');
        expect((await session.state()).slots.selected).toBe(selected.slice(5));
        const colours = await backgrounds(session, [hour, 'hour-now']);
        expect(prominence(colours[selected])).toBeGreaterThan(
          prominence(colours[selected === hour ? 'hour-now' : hour]) + 0.05);
      }
    }
    expect(net.unanswered).toEqual([]);
    expect(failures).toEqual([]);
  } finally {
    await session?.close();
    await net.close();
  }
}, 30000);

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
