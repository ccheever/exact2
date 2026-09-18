// Diagnostic for the observed async predicate bug; not a repository check.
import { chromium } from 'playwright';
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM || '/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', headless: true });
try {
  const page = await browser.newPage();
  await page.goto('about:blank');
  await page.evaluate(() => { window.probeReady = false; setTimeout(() => { window.probeReady = true; }, 1200); });
  const started = performance.now();
  await page.waitForFunction(async () => window.probeReady === true);
  console.log(JSON.stringify({ elapsedMs: performance.now() - started, value: await page.evaluate(() => window.probeReady) }));
} finally { await browser.close(); }
