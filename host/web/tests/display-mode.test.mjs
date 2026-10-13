// LLP 1039 D7: a compiled app reads the browser's display-mode, including
// transitions, through the same media reporter as the wasm target.
import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
test('display mode agrees with Chrome and media changes reanswer the resource and agent state', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-display-mode-'));
  let server, browser;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Display mode', app: { id: 'com.example.displaymode', name: 'Display mode' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), 'shape Viewport\n  displayMode: string\n  colorGamut: string\n  dynamicRange: string\n  horizontalViewportSegments: number\n  verticalViewportSegments: number\ncomponent App\n  resource viewport = exactViewport() as shape Viewport\n  view\n    text viewport.displayMode testId="mode"\n');
    const dist = resolve(dir, 'dist');
    const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'display-mode', '--out', dist, '--render', 'js'], {
      cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir },
    });
    expect(build.status, build.stderr + build.stdout).toBe(0);
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: req => {
      const path = new URL(req.url).pathname;
      if (path === '/navigation.js') return new Response(Bun.file(resolve(ROOT, 'host/web/navigation.js')));
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.addInitScript(() => {
      const real = matchMedia.bind(window), lists = [];
      window.matchMedia = query => {
        const list = real(query), mode = /^\(display-mode: (.+)\)$/.exec(query)?.[1];
        if (mode || query.startsWith("(color-gamut:") || query === "(dynamic-range: high)") {
          const live = real(query);
          Object.defineProperty(list, 'matches', { get: () => {
            if (mode) return window.testDisplayMode == null ? live.matches : window.testDisplayMode === mode;
            if (query === '(dynamic-range: high)') return window.testHDR ?? live.matches;
            if (window.testGamut == null) return live.matches;
            return query === '(color-gamut: p3)' ? window.testGamut !== 'srgb' : window.testGamut === 'rec2020';
          } });
          lists.push(list);
        }
        return list;
      };
      window.changeDisplayMode = mode => {
        window.testDisplayMode = mode;
        for (const list of lists) list.dispatchEvent(new Event('change'));
      };
    });
    await page.goto(`http://127.0.0.1:${server.port}/?agent`);
    await page.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const agrees = async mode => {
      await page.waitForFunction(async expected => {
        const state = await exact.agentSettled({ op: 'state' });
        return state.resources.viewport.displayMode === expected;
      }, mode);
      const result = await page.evaluate(async expected => {
        const state = await exact.agentSettled({ op: 'state' });
        return [matchMedia(`(display-mode: ${expected})`).matches, state.resources.viewport.displayMode, state.device.displayMode];
      }, mode);
      expect(result).toEqual([true, mode, mode]);
      const packed = await page.evaluate(async () => {
        const { preferences, foldBits, fold } = await import('/navigation.js');
        const bits = (preferences() | foldBits()) >>> 0, f = fold();
        const state = await exact.agentSettled({ op: 'state' });
        const mode = ['standalone', 'browser', 'fullscreen', 'minimal-ui', 'window-controls-overlay', 'picture-in-picture', 'tabbed'][(bits >>> 29) & 7];
        const gamut = matchMedia('(color-gamut: rec2020)').matches ? 'rec2020' : matchMedia('(color-gamut: p3)').matches ? 'p3' : 'srgb';
        const hdr = matchMedia('(dynamic-range: high)').matches ? 'high' : 'standard';
        return {
          actual: [(bits >>> 9) & 255, (bits >>> 17) & 255, (bits >>> 8) & 1, mode, ['srgb', 'p3', 'rec2020', 'rec2020'][(bits >>> 26) & 3], bits & (1 << 28) ? 'high' : 'standard'],
          expected: [f.cols, f.rows, f.posture === 'folded' ? 1 : 0, state.resources.viewport.displayMode, gamut, hdr],
          resource: [state.resources.viewport.horizontalViewportSegments, state.resources.viewport.verticalViewportSegments, state.resources.viewport.colorGamut, state.resources.viewport.dynamicRange],
          expectedResource: [f.cols, f.rows, gamut, hdr],
        };
      });
      expect(packed.actual).toEqual(packed.expected);
      expect(packed.resource).toEqual(packed.expectedResource);
    };
    await agrees('browser');
    await page.evaluate(() => {
      const button = document.createElement('button');
      button.id = 'enter-fullscreen'; button.textContent = 'Enter fullscreen';
      button.onclick = () => document.documentElement.requestFullscreen();
      document.body.append(button);
    });
    await page.locator('#enter-fullscreen').click();
    expect(await page.evaluate(() => !!document.fullscreenElement)).toBe(true);
    // Chrome on macOS may keep display-mode: browser for DOM fullscreen.
    // This fact follows its media query, not document.fullscreenElement.
    const current = () => page.evaluate(() => ['browser', 'fullscreen', 'standalone', 'minimal-ui', 'window-controls-overlay', 'picture-in-picture', 'tabbed'].find(v => matchMedia(`(display-mode: ${v})`).matches));
    await agrees(await current());
    await page.evaluate(() => document.exitFullscreen());
    await agrees(await current());
    // Exercise change delivery even on a browser whose real window mode
    // stays browser, and modes that require an installed web application.
    for (const [i, mode] of ['fullscreen', 'window-controls-overlay', 'standalone', 'minimal-ui', 'picture-in-picture', 'tabbed', 'browser'].entries()) {
      await page.evaluate(async ({ i, mode }) => {
        const { preferFold } = await import('/navigation.js');
        preferFold({ cols: 4, rows: 2, posture: 'folded' });
        window.testGamut = ['srgb', 'p3', 'rec2020'][i % 3];
        window.testHDR = i % 2 === 0;
        changeDisplayMode(mode);
      }, { i, mode });
      await agrees(mode);
    }
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 180000);
