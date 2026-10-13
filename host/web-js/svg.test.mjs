// Build exact-web-js once before running this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from 'playwright-core';
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
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
