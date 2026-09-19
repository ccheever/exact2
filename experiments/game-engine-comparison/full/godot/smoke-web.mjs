import { chromium } from "playwright";

const url = process.env.LANTERNS_URL ?? "http://127.0.0.1:4173/?agent=1";
const executablePath = process.env.CHROMIUM ??
  "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const browser = await chromium.launch({ headless: true, executablePath });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("console", message => console.log("BROWSER", message.type(), message.text()));
await page.goto(url);
await page.waitForFunction(() => window.lanterns);
console.log("STAGE adapter");
const ready = await page.evaluate(() => window.lanterns.command({ op: "ready" }));
console.log("STAGE ready");
if (!ready.ready || ready.engine !== "Godot") throw new Error(JSON.stringify(ready));
await page.evaluate(() => window.lanterns.command({ op: "start" }));
console.log("STAGE start");
await page.evaluate(() => window.lanterns.command({ op: "input", moveX: 0, moveZ: -1 }));
console.log("STAGE input");
const moved = await page.evaluate(() => window.lanterns.command({ op: "step", ticks: 60 }));
console.log("STAGE step");
if (moved.ticks !== 60 || moved.player.z >= 10 || moved.player.animation !== "Run") {
  throw new Error(`movement smoke failed: ${JSON.stringify(moved.player)}`);
}
await page.evaluate(() => window.lanterns.command({ op: "save" }));
console.log("STAGE save");
await page.screenshot({ path: "evidence/web-gameplay.png" });
await page.reload();
await page.waitForFunction(() => window.lanterns);
const loaded = await page.evaluate(() => window.lanterns.command({ op: "load" }));
if (loaded.ticks !== 60 || Math.abs(loaded.player.z - moved.player.z) > 0.15) throw new Error("save/reload mismatch");
console.log(JSON.stringify({ pass: true, ready, ticks: loaded.ticks, player: loaded.player }));
await browser.close();
