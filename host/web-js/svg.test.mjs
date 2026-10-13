// Build exact-web-js once before running this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, webkit } from 'playwright-core';
import { decodePng } from '../../scripts/png.mjs';
import { chromium as browserPath } from '../../scripts/agent-launch.mjs';

test('compiled SVG geometry bindings render, change, unset and preserve zero', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const dir = mkdtempSync(resolve(target, 'svg-bindings-'));
  let server, browser;
  try {
    const compiled = spawnSync(resolve(target, 'debug/exact-web-js'), [
      'js', resolve(import.meta.dir, 'conformance/svg-geometry.contract'), '-o', dir,
    ], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      const file = [dir, import.meta.dir, resolve(root, 'host/web')]
        .map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
    await page.goto(server.url.href);
    const nested = page.getByTestId('nested');
    await nested.waitFor({ state: 'attached' });
    const hairline = page.getByTestId('hairline');
    const height = element => element.evaluate(e => e.getBoundingClientRect().height);
    expect(await height(hairline)).toBe(1);
    expect(await height(page.getByTestId('hairline-static'))).toBe(1);
    expect(await hairline.getAttribute('height')).toBe('1');
    await page.getByTestId('hairline-toggle').click();
    expect(await height(hairline)).toBe(10);
    expect(await hairline.getAttribute('height')).toBe('10px');
    await page.getByTestId('hairline-toggle').click();
    expect(await height(hairline)).toBe(1);
    const envGeometry = () => page.evaluate(() => ['environment-static', 'environment-bound'].map(id => {
      const e = document.querySelector(`[data-testid="${id}"]`);
      return { attribute: e.getAttribute('height'), height: e.querySelector('rect').getBBox().height };
    }));
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    await page.getByTestId('fallback').click();
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: 'env(viewport-segment-height 1 0, 25%)', height: 100 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBe('env(viewport-segment-height 1 0, 25%)');
    await page.getByTestId('fallback').click();
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    const cdp = await page.context().newCDPSession(page);
    for (const scale of [1.25, 2, 3, 1]) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: scale, mobile: false });
      // CDP changes the scale without emitting the resize that browser zoom emits.
      await page.evaluate(() => dispatchEvent(new Event('resize')));
      for (const id of ['hairline', 'hairline-static', 'hairline-pattern']) {
        expect(Number(await page.getByTestId(id).getAttribute('height'))).toBeCloseTo(1 / scale, 6);
      }
      for (const id of ['hairline', 'hairline-static']) expect(await height(page.getByTestId(id))).toBeCloseTo(1 / scale, 1);
      await page.getByTestId('hairline-toggle').click();
      expect(await hairline.getAttribute('height')).toBe('10px');
      await page.getByTestId('hairline-toggle').click();
      expect(Number(await hairline.getAttribute('height'))).toBeCloseTo(1 / scale, 6);
    }
    for (const [width, sharp, ellipseWidth] of [[800, true, 0], [900, true, 0], [1000, false, 20], [800, true, 0]]) {
      await page.setViewportSize({ width, height: 600 });
      expect(await page.getByTestId('calculated-rect').evaluate(e => e.isPointInFill(new DOMPoint(1, 1)))).toBe(sharp);
      expect(await page.getByTestId('calculated-ellipse').evaluate(e => e.getBBox().width)).toBe(ellipseWidth);
    }
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false,
      displayFeature: { orientation: 'vertical', offset: 400, maskLength: 0 } });
    await page.evaluate(() => dispatchEvent(new Event('resize')));
    expect(await envGeometry()).toEqual([
      { attribute: 'env(viewport-segment-height 1 0)', height: 600 },
      { attribute: 'calc(env(viewport-segment-height 1 0) + 0px)', height: 600 },
    ]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBe('calc(env(viewport-segment-height 1 0) + 0px)');
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false });
    await page.evaluate(() => exact.fold.prefer({}));
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    const geometry = () => nested.evaluate(e => ({
      attribute: e.getAttribute('height'),
      height: e.querySelector('rect').getBBox().height,
    }));
    const attributes = async (extent, offset) => {
      for (const id of ['mask', 'pattern', 'filter', 'primitive', 'foreign', 'use']) {
        expect(await page.getByTestId(id).evaluate(e =>
          ['x', 'y', 'width', 'height'].map(name => e.getAttribute(name))
        )).toEqual([offset, offset, extent, extent]);
      }
      expect(await page.getByTestId('gradient').evaluate(e =>
        ['cx', 'cy', 'r'].map(name => e.getAttribute(name))
      )).toEqual([extent, extent, extent]);
      expect(await nested.evaluate(e => [e.getAttribute('x'), e.getAttribute('y')]))
        .toEqual([offset, offset]);
    };
    expect(await geometry()).toEqual({ attribute: '25vh', height: 150 });
    await attributes('25vh', '10');
    await page.getByTestId('resize').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    await attributes('50vh', '20');
    await page.getByTestId('omit').click();
    expect(await geometry()).toEqual({ attribute: null, height: 400 });
    await attributes(null, null);
    await page.getByTestId('omit').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    await page.getByTestId('zero').click();
    expect(await geometry()).toEqual({ attribute: '0', height: 0 });
    await attributes('0', '0');
    await page.getByTestId('zero').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    await page.getByTestId('auto').click();
    expect(await geometry()).toEqual({ attribute: null, height: 400 });
    await attributes(null, '20');
    await page.getByTestId('auto').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    await browser.close();
    browser = await webkit.launch({ headless: true });
    const safari = await browser.newPage({ viewport: { width: 800, height: 600 }, deviceScaleFactor: 2 });
    await safari.goto(server.url.href);
    await safari.getByTestId('hairline-pattern').waitFor({ state: 'attached' });
    for (const id of ['hairline-pattern', 'hairline', 'hairline-static']) {
      expect(await safari.getByTestId(id).getAttribute('height')).toBe('0.5');
    }
    const pixels = decodePng(await safari.getByTestId('hairline-paint').screenshot());
    let red = 0;
    for (let i = 0; i < pixels.data.length; i += 4) {
      if (pixels.data[i] === 225 && pixels.data[i + 1] === 29 && pixels.data[i + 2] === 72) red++;
    }
    expect(red / (pixels.width * pixels.height)).toBeGreaterThan(0.4);
    expect(red / (pixels.width * pixels.height)).toBeLessThan(0.6);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
