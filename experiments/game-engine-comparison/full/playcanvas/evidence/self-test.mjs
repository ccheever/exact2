import { chromium } from "/tmp/exact-engine-comparison-20260917/browser2d/node_modules/playwright/index.mjs";
import { writeFileSync } from "node:fs";

const executablePath = "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const browser = await chromium.launch({ executablePath, headless: true, args: ["--no-sandbox", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on("pageerror", (error) => errors.push(String(error)));
page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
const startedAt = performance.now();
await page.goto("http://127.0.0.1:4173/?agent=1", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.lanterns, null, { timeout: 20000 }).catch((error) => {
  console.error("adapter wait failed", String(error), errors);
  throw error;
});
while (!(await page.evaluate(() => window.lanterns.command({ op: "ready" }))).ready) await new Promise((resolve) => setTimeout(resolve, 25));
const startupMs = performance.now() - startedAt;
const ready = await page.evaluate(() => window.lanterns.command({ op: "ready" }));
const initial = await page.evaluate(() => window.lanterns.command({ op: "start" }));
await page.screenshot({ path: "evidence/playcanvas-start.png" });
await page.evaluate(() => window.lanterns.command({ op: "input", moveZ: -1 }));
const moved = await page.evaluate(() => window.lanterns.command({ op: "step", ticks: 60 }));
await page.evaluate(() => window.lanterns.command({ op: "input", jump: true }));
const jumped = await page.evaluate(() => window.lanterns.command({ op: "step", ticks: 12 }));
await page.evaluate(() => window.lanterns.command({ op: "input" }));
const savedResult = await page.evaluate(() => window.lanterns.command({ op: "save" }));
await page.reload({ waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.lanterns, null, { timeout: 30000 });
while (!(await page.evaluate(() => window.lanterns.command({ op: "ready" }))).ready) await new Promise((resolve) => setTimeout(resolve, 25));
const loaded = await page.evaluate(() => window.lanterns.command({ op: "load" }));
const result = {
  host: process.env.HOSTNAME,
  engine: ready,
  startupMs,
  checks: {
    initial: initial.phase === "playing" && initial.ticks === 0 && initial.total === 12,
    movement: moved.player.z < initial.player.z - 4 && moved.ticks === 60,
    jump: jumped.player.y > 0.2 && jumped.events.some((event) => event.type === "jump"),
    persistedReload: savedResult.saved && loaded.ticks === jumped.ticks && Math.abs(loaded.player.z - jumped.player.z) < 0.01,
    noBrowserErrors: errors.length === 0
  },
  samples: { initial, moved, jumped, loaded },
  errors
};
result.passed = Object.values(result.checks).every(Boolean);
writeFileSync("evidence/results.json", JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
if (!result.passed) process.exit(1);
