// The design tool's Inspector on an app whose design declares no mode axes (stage 15 D4 batch 1 #1, theme2's 18:30
// ruling): exact2's library-gate, on the JS dev loop. Its copy sits on the light column, not "the app's design is not
// read yet", and a library component's row that reads a token passed by name through a prop offers that token's edit
// (VerifiedBadge's check: `color=mark`, mark="cards.badge-mark").
import { expect, onTestFinished, test } from 'bun:test';
import { resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { checkoutPort, takePort } from './port-lock.mjs';
import { ROOT, TOOL_ORIGIN, lines, stop, toolTurns } from './design-tool-harness.mjs';
import { suiteBrowser } from './agent-launch.mjs';

toolTurns();

test('an app with no mode axes: a selection is placed on the light column, and a token a prop carries by name offers its edit', async () => {
  // The gate's design server takes its .exact/tamber.lock: in turn with any other run here.
  const giveGate = await takePort(1000 + checkoutPort(resolve(ROOT, 'apps/library-gate')), { label: `design-tool-noaxes on ${ROOT}/apps/library-gate` });
  onTestFinished(giveGate);
  const listener = (await import('node:http')).createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'library-gate', '--port', String(port)], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  const browser = await suiteBrowser('chromium');
  let tool;
  try {
    const base = `http://127.0.0.1:${port}`;
    await lines(dev).until(l => l.includes('design reader ready'), 900_000);
    tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', resolve(ROOT, 'apps/library-gate'), '--via', base, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000);
    const page = await browser.newPage({ viewport: { width: 1400, height: 1200 }, colorScheme: 'light' });
    await page.goto(url);
    await page.locator('[data-testid="name"]').fill('Ada');
    await page.locator('[data-testid="save-name"]').click();
    await page.locator('[data-testid^="build-web-"]').first().waitFor({ timeout: 120_000 });
    await page.locator('[data-testid="inspect-toggle"]').click();
    await page.locator('[data-testid="inspect-result"]').waitFor();
    const frame = page.frames().find(f => f.url().startsWith(base.replace('127.0.0.1', 'localhost')));
    await frame.locator('#exact-inspect').waitFor();
    // The verified badge: its check (the top image, `color=mark`).
    const badge = frame.locator('[data-testid="vb-base"]');
    await badge.scrollIntoViewIfNeeded();
    await badge.click({ force: true });
    await page.locator('[data-testid="selection-source"]').waitFor({ timeout: 60_000 });
    await page.waitForFunction(() => /VerifiedBadge/.test(document.querySelector('[data-testid="selection-source"]')?.innerText ?? ''), null, { timeout: 60_000 });
    // (Placed once the app's design is read, which may come after the selection: waited for, not assumed.)
    await page.waitForFunction(() => document.querySelector('[data-testid="selrow-edit-color"]')?.innerText === 'Edit cards.badge-mark', null, { timeout: 60_000 });
    expect(await page.locator('[data-testid="selection-build"]').innerText()).toMatch(/ · Light$/);
    expect(await page.locator('[data-testid="selection-stale"]').count()).toBe(0);
    expect(await page.locator('[data-testid="selrow-edit-color"]').innerText()).toBe('Edit cards.badge-mark');
    await page.locator('[data-testid="inspect-toggle"]').click();
    await frame.locator('#exact-inspect').waitFor({ state: 'detached' });
  } finally {
    await browser.close();
    if (tool) await stop(tool);
    await stop(dev);
  }
}, 1_200_000);
