// The design tool's inspect on the canvas frame, on the wasm dev loop (T2b; split from design-tool-canvas.test.mjs, so a
// lane runs it beside the others).
import { expect, onTestFinished, test } from 'bun:test';
import { mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { checkoutPort, takePort } from './port-lock.mjs';
import { ROOT, TOOL_ORIGIN, lines, stop, toolTurns } from './design-tool-harness.mjs';
import { suiteBrowser } from './agent-launch.mjs';

toolTurns();

test('T2b: inspect from the tool selects a view in the canvas frame, shows its source and rows, and opens its token on the branch that won', async () => {
  // This checkout's apps/theme-fixture (the wasm loop builds the app's crate, so no copy will do), in turn with another
  // run here: its design server takes the app's .exact/tamber.lock, which refuses a second writer.
  const giveFixture = await takePort(1000 + checkoutPort(resolve(ROOT, 'apps/theme-fixture')), { label: `T2b on ${ROOT}/apps/theme-fixture` });
  onTestFinished(giveFixture);
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  // Its web build kept for the next run here (a build fixture, held under the fixture's turn above, never two at once):
  // the dev loop starts on it when its receipt still matches the app's sources (dev.mjs currentWebBuild), and builds
  // the wasm again when not, as it would on a fresh one. Nothing else lives there.
  const dist = resolve(ROOT, 'target', 'design-tool-inspect-web');
  mkdirSync(dist, { recursive: true });
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--wasm', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  // Site isolation on, as desktop Chrome has it: the pinned headless shell (the default browser without CHROME) runs a
  // cross-site frame in its parent's process unless told, so the check below could never pass there.
  const browser = await suiteBrowser('chromium', { args: ['--site-per-process'] });
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    const devSaid = lines(dev);
    await devSaid.until(l => l.includes('plan ready'), 600_000);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', resolve(ROOT, 'apps/theme-fixture'), '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const page = await browser.newPage({ viewport: { width: 1400, height: 1200 } });
    await page.goto(url);
    await page.locator('[data-testid="name"]').fill('Ada');
    await page.locator('[data-testid="save-name"]').click();
    await page.locator('[data-testid^="build-web-"]').first().waitFor({ timeout: 60_000 });
    await page.locator('[data-testid="inspect-toggle"]').click();
    await page.locator('[data-testid="inspect-result"]').waitFor();
    expect(await page.locator('[data-testid="inspect-result"]').innerText()).toMatch(/^Inspecting on 1 build\(s\)/);
    // The frame shows the banner; a click on Swap pins there (through the
    // page's cover, at its place) selects it.
    const frame = page.frames().find(f => f.url().startsWith(base.replace('127.0.0.1', 'localhost')));
    await frame.locator('#exact-inspect').waitFor();
    // Another site than the tool's: the browser runs it out of the tool's
    // process (an iframe target of its own, with site isolation on, above).
    // Its target's URL can trail the frame's first navigation, so it is asked
    // again for a while; a frame in the tool's own process never shows as one.
    const cdp = await page.context().newCDPSession(page), here = base.replace('127.0.0.1', 'localhost');
    let isolated = false;
    for (const end = Date.now() + 10_000; Date.now() < end; await Bun.sleep(200)) {
      if ((await cdp.send('Target.getTargets')).targetInfos.some(t => t.type === 'iframe' && t.url.startsWith(here))) { isolated = true; break; }
    }
    expect(isolated).toBe(true);
    const at = await frame.locator('[data-testid="swap"]').boundingBox();
    await page.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
    const line = readFileSync(resolve(ROOT, 'apps/theme-fixture/app.contract'), 'utf8').split('\n').findIndex(l => l.includes('text "Swap pins"')) + 1;
    await page.locator('[data-testid="selection-source"]').waitFor({ timeout: 60_000 });
    expect(await page.locator('[data-testid="selection-source"]').innerText()).toBe(`app.contract:${line} · App`);
    // Its colour is inherited: a link to the page's token, not its edit;
    // opened, the grid's cell for it, in the column the copy shows.
    expect(await page.locator('[data-testid="selrow-link-color"]').innerText()).toBe('Open text.primary');
    expect(await page.locator('[data-testid="selrow-edit-color"]').count()).toBe(0);
    // The page is light (headless Chrome's default) in Black: the light
    // column, its default branch.
    expect(await page.locator('[data-testid="selection-build"]').innerText()).toMatch(/ · Light$/);
    await page.locator('[data-testid="selrow-link-color"]').click();
    await page.waitForFunction(() => /source site\(s\)/.test(document.querySelector('[data-testid="impact-count"]')?.innerText ?? ''), null, { timeout: 60_000 });
    expect(await page.locator('[data-testid="impact-count"]').innerText()).toMatch(/^text\.primary: read at [1-9]\d* source site\(s\), statically/);
    await page.locator('[data-testid="branch-label"]').waitFor();
    expect(await page.locator('[data-testid="branch-label"]').innerText()).toBe('text.primary · default (Light)');
    // Stop: the frame leaves inspect mode.
    await page.locator('[data-testid="inspect-toggle"]').click();
    await frame.locator('#exact-inspect').waitFor({ state: 'detached' });
  } finally {
    await browser.close();
    if (tool) await stop(tool);
    await stop(dev);
  }
}, 900_000);
