import { chromium } from "playwright";

const executablePath = process.env.CHROMIUM ??
  "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const browser = await chromium.launch({ headless: true, executablePath });
const page = await browser.newPage();
await page.goto(process.env.LANTERNS_URL ?? "http://127.0.0.1:4173/?agent=1");
await page.waitForFunction(() => window.lanterns);
const command = request => page.evaluate(request => window.lanterns.command(request), request);

await command({ op: "start" });
const began = performance.now();
const lost = await command({ op: "step", ticks: 10800 });
const milliseconds = performance.now() - began;
if (lost.phase !== "lost" || lost.ticks !== 10800 || lost.remaining !== 0) {
  throw new Error(`loss mismatch: ${JSON.stringify(lost)}`);
}

await command({ op: "reset" });
await command({ op: "start" });
await command({ op: "input", moveZ: -1 });
const batch = await command({ op: "step", ticks: 60 });
await command({ op: "reset" });
await command({ op: "start" });
await command({ op: "input", moveZ: -1 });
let singles;
for (let i = 0; i < 60; i++) singles = await command({ op: "step", ticks: 1 });
if (batch.ticks !== singles.ticks ||
    Math.abs(batch.player.z - singles.player.z) > 0.002 ||
    Math.abs(batch.crate.y - singles.crate.y) > 0.002) {
  throw new Error(`batch mismatch: ${JSON.stringify({batch, singles})}`);
}
console.log(JSON.stringify({
  pass: true,
  lossMilliseconds: milliseconds,
  lost: { phase: lost.phase, ticks: lost.ticks, remaining: lost.remaining },
  equivalence: { batchZ: batch.player.z, singlesZ: singles.player.z },
}));
await browser.close();
