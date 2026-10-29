// Build exact-web-js once before running this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { webkit } from 'playwright-core';
import { decodePng } from '../../scripts/png.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('compiled SVG geometry bindings render, change, unset and preserve zero', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const dir = mkdtempSync(resolve(target, 'svg-bindings-'));
  const sheet = readFileSync(resolve(root, 'host/web/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1];
  let server, browser;
  try {
    const compiled = spawnSync(resolve(target, 'debug/exact-web-js'), [
      'js', resolve(import.meta.dir, 'conformance/svg-geometry.contract'), '-o', dir,
    ], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${sheet}</style><link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      // A module that re-exports the host's own reaches it through `../web/`.
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
    const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
    await page.goto(server.url.href);
    const nested = page.getByTestId('nested');
    await nested.waitFor({ state: 'attached' });
    expect(await page.getByTestId('viewport-static').evaluate(e => [e.getBBox().width, e.getBBox().height])).toEqual([80, 60]);
    const hairline = page.getByTestId('hairline');
    const height = element => element.evaluate(e => e.getBoundingClientRect().height);
    expect(await height(hairline)).toBe(1);
    expect(await height(page.getByTestId('hairline-static'))).toBe(1);
    expect(await hairline.getAttribute('height')).toBeNull();
    expect(await hairline.evaluate(e => e.style.height)).toBe('var(--exact-hairline, 1px)');
    await page.getByTestId('hairline-toggle').click();
    expect(await height(hairline)).toBe(10);
    expect(await hairline.getAttribute('height')).toBe('10px');
    await page.getByTestId('hairline-toggle').click();
    expect(await height(hairline)).toBe(1);
    expect(await page.getByTestId('use').evaluate(e => {
      const box = e.getBoundingClientRect();
      return [e.getAttribute('width'), e.getAttribute('height'), box.width, box.height];
    })).toEqual(['25vh', '25vh', 150, 150]);
    const envGeometry = () => page.evaluate(() => ['environment-static', 'environment-bound'].map(id => {
      const e = document.querySelector(`[data-testid="${id}"]`);
      return { attribute: e.getAttribute('height'), height: e.querySelector('rect').getBBox().height };
    }));
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    await page.getByTestId('fallback').click();
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    await page.getByTestId('fallback').click();
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-bound').evaluate(e => e.style.height)).toContain('env(');
    const cdp = await page.context().newCDPSession(page);
    for (const [scale, expected] of [[1.25, 1], [2, 0.5], [3, 1 / 3], [1, 1]]) {
      // CDP needs a viewport resize to invalidate resolution media queries.
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800 + Math.round(scale * 10), height: 600, deviceScaleFactor: scale, mobile: false });
      await page.evaluate(() => new Promise(requestAnimationFrame));
      expect(await hairline.getAttribute('height')).toBeNull();
      expect(await height(hairline)).toBeCloseTo(expected, 1);
      for (const id of ['hairline-static', 'hairline-pattern']) {
        expect(await page.getByTestId(id).getAttribute('height')).toBe('1');
      }
      await page.getByTestId('hairline-toggle').click();
      expect(await hairline.getAttribute('height')).toBe('10px');
      expect(await hairline.evaluate(e => e.style.height)).toBe('');
      await page.getByTestId('hairline-toggle').click();
      await page.evaluate(() => new Promise(requestAnimationFrame));
      expect(await hairline.getAttribute('height')).toBeNull();
      expect(await height(hairline)).toBeCloseTo(expected, 1);
    }
    for (const [width, sharp, ellipseWidth] of [[200, true, 0], [300, true, 0], [400, true, 20], [800, true, 100], [900, true, 120], [1000, false, 140], [800, true, 100]]) {
      await page.setViewportSize({ width, height: 600 });
      expect(await page.getByTestId('calculated-rect').evaluate(e => e.isPointInFill(new DOMPoint(1, 1)))).toBe(sharp);
      expect(await page.getByTestId('calculated-ellipse').evaluate(e => e.getBBox().width)).toBe(ellipseWidth);
    }
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false,
      displayFeature: { orientation: 'vertical', offset: 400, maskLength: 0 } });
    await page.evaluate(() => dispatchEvent(new Event('resize')));
    expect(await envGeometry()).toEqual([
      { attribute: null, height: 400 },
      { attribute: null, height: 400 },
    ]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false });
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    const geometry = () => nested.evaluate(e => ({
      attribute: e.getAttribute('height'),
      height: e.querySelector('rect').getBBox().height,
    }));
    const attributes = async (extent, offset, foreignExtent = extent, useExtent = extent) => {
      for (const id of ['mask', 'pattern', 'filter', 'primitive', 'foreign', 'use']) {
        const size = id === 'foreign' ? foreignExtent : id === 'use' ? useExtent : extent;
        expect(await page.getByTestId(id).evaluate(e =>
          ['x', 'y', 'width', 'height'].map(name => e.getAttribute(name))
        )).toEqual([offset, offset, size, size]);
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
    await attributes('0', '0', '0', '25vh');
    await page.getByTestId('zero').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    await page.getByTestId('auto').click();
    expect(await geometry()).toEqual({ attribute: null, height: 400 });
    await attributes(null, '20', '10px');
    expect(await page.getByTestId('environment-bound').getAttribute('height')).toBe('10px');
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBe('10px');
    expect(await page.getByTestId('use').getAttribute('height')).toBeNull();
    await page.getByTestId('auto').click();
    expect(await geometry()).toEqual({ attribute: '50vh', height: 300 });
    expect(await envGeometry()).toEqual([{ attribute: null, height: 400 }, { attribute: null, height: 400 }]);
    expect(await page.getByTestId('environment-gradient').getAttribute('cx')).toBeNull();
    expect(await page.getByTestId('use').getAttribute('height')).toBe('50vh');
    const comparisons = await page.evaluate(async () => {
      const { svgGeometry } = await import('/svg-transform.js');
      const { sig, W, commit, Sm, Hooks } = await import('/rt.js');
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('width', '200'); svg.setAttribute('height', '400');
      const element = document.createElementNS(svg.namespaceURI, 'foreignObject');
      const control = document.createElementNS(svg.namespaceURI, 'foreignObject');
      for (const e of [element, control]) { e.setAttribute('width', '100'); e.style.fontSize = '20px'; svg.append(e); }
      document.body.append(svg);
      const value = sig('10vh');
      const held = [];
      const previous = Hooks.style;
      Hooks.style = (e, name, text) => { held.push(text); return false; };
      try {
        svgGeometry(element, 'height', value, Sm);
        return ['calc(10vh + 10px)', 'min(10vh, 50px)', 'max(10vh, 50px)', 'clamp(10px, 10vh, 100px)',
          'env(safe-area-inset-top, 3px)', 'env(viewport-segment-height 1 0, 25%)', '-exact-hairline', '2em', '2rem'].map(length => {
          commit(() => W(value, length), 'test');
          const css = length === '-exact-hairline' ? 'var(--exact-hairline, 1px)' : length;
          control.style.height = ''; control.style.height = css;
          const fallback = { length, attribute: element.getAttribute('height'), css: element.style.height,
            control: control.style.height, height: element.getBoundingClientRect().height,
            expected: control.getBoundingClientRect().height, motion: held.at(-1) };
          commit(() => W(value, '10vh'), 'test');
          return { ...fallback, restored: [element.getAttribute('height'), element.style.height] };
        });
      } finally { Hooks.style = previous; svg.remove(); }
    });
    for (const result of comparisons) {
      expect(result.attribute, result.length).toBeNull();
      expect(result.css, result.length).toBe(result.control);
      expect(result.height, result.length).toBeCloseTo(result.expected, 3);
      expect(result.motion, result.length).toBe(result.length === '-exact-hairline' ? 'var(--exact-hairline, 1px)' : result.length);
      expect(result.restored).toEqual(['10vh', '']);
    }
    await browser.close();
    if (existsSync(webkit.executablePath())) {
      browser = await suiteBrowser('webkit');
      const safari = await browser.newPage({ viewport: { width: 800, height: 600 }, deviceScaleFactor: 2 });
      await safari.goto(server.url.href);
      await safari.getByTestId('hairline-pattern').waitFor({ state: 'attached' });
      for (const id of ['hairline-pattern', 'hairline-static']) {
        expect(await safari.getByTestId(id).getAttribute('height')).toBe('1');
      }
      expect(await safari.getByTestId('hairline').getAttribute('height')).toBeNull();
      expect(await height(safari.getByTestId('hairline'))).toBeCloseTo(0.5, 1);
      const pixels = decodePng(await safari.getByTestId('hairline-paint').screenshot());
      let red = 0;
      for (let i = 0; i < pixels.data.length; i += 4) {
        if (Math.abs(pixels.data[i] - 225) <= 8 && Math.abs(pixels.data[i + 1] - 29) <= 8 && Math.abs(pixels.data[i + 2] - 72) <= 8) red++;
      }
      expect(red / (pixels.width * pixels.height)).toBeGreaterThan(0.4);
      expect(red / (pixels.width * pixels.height)).toBeLessThan(0.6);
    }
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
