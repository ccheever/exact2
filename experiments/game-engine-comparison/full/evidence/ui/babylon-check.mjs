import { chromium } from "/tmp/exact-full-games-20260917/babylon/node_modules/playwright/index.mjs";
import { mkdirSync, writeFileSync } from "node:fs";

const output = "/tmp/babylon-ui-evidence";
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({
  executablePath: "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome",
  headless: true,
  args: ["--no-sandbox", "--enable-unsafe-swiftshader"]
});
const errors = [];
const checks = {};

async function command(page, request) {
  return page.evaluate((value) => window.lanterns.command(value), request);
}
async function ready(page) {
  await page.waitForFunction(() => window.lanterns, null, { timeout: 30000 });
  for (;;) {
    if ((await command(page, { op: "ready" })).ready) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
}
function watch(page, surface) {
  page.on("pageerror", (error) => errors.push(`${surface}: ${error}`));
  page.on("console", (message) => { if (message.type() === "error") errors.push(`${surface}: ${message.text()}`); });
}

const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 } });
watch(desktop, "desktop");
await desktop.goto("http://127.0.0.1:39977/?agent=1");
await ready(desktop);
checks.desktopTitle = await desktop.locator("#title").isVisible();
await desktop.locator("#start").click();
checks.desktopStartButton = (await command(desktop, { op: "state" })).phase === "playing";
const beforeKeyboard = await command(desktop, { op: "state" });
await desktop.keyboard.down("w");
await command(desktop, { op: "step", ticks: 36 });
await desktop.keyboard.up("w");
const afterKeyboard = await command(desktop, { op: "state" });
checks.desktopKeyboardMove = afterKeyboard.player.z < beforeKeyboard.player.z - 2;
await desktop.keyboard.press("Space");
await command(desktop, { op: "step", ticks: 10 });
const afterJump = await command(desktop, { op: "state" });
checks.desktopKeyboardJump = afterJump.player.y > .3 && afterJump.events.some((event) => event.type === "jump");
const sound = desktop.locator("#sound");
await sound.click();
const mutedText = await sound.textContent();
await sound.click();
checks.desktopSoundToggle = mutedText?.trim() === "Sound off" && (await sound.textContent())?.trim() === "Sound on";
await desktop.locator("#pause").click();
const paused = await command(desktop, { op: "state" });
checks.desktopPauseButton = paused.phase === "paused" && await desktop.locator("#modal").isVisible();
await desktop.locator("#save").click();
const saved = await command(desktop, { op: "state" });
checks.desktopSaveButton = saved.phase === "paused" && (await desktop.locator("#toast").textContent())?.includes("saved") && await desktop.evaluate(() => !!localStorage.getItem("lanterns-babylon-save-v1"));
await desktop.reload();
await ready(desktop);
checks.desktopResumeSaveVisible = await desktop.locator("#resume").isVisible();
await desktop.locator("#resume").click();
const loaded = await command(desktop, { op: "state" });
checks.desktopLoadButton = loaded.phase === "paused" && loaded.ticks === saved.ticks && Math.abs(loaded.player.z - saved.player.z) < .01 && loaded.events.some((event) => event.type === "load");
await desktop.locator("#modal-main").click();
checks.desktopResumeButton = (await command(desktop, { op: "state" })).phase === "playing";
const desktopRendered = await command(desktop, { op: "state" });
const desktopWholeSeconds = Math.ceil(desktopRendered.remaining);
const desktopExpectedTime = `${Math.floor(desktopWholeSeconds / 60)}:${String(desktopWholeSeconds % 60).padStart(2, "0")}`;
checks.desktopTimerDisplay = (await desktop.locator("#time").textContent())?.trim() === desktopExpectedTime;
await desktop.waitForTimeout(100);
await desktop.screenshot({ path: `${output}/babylon-desktop.png` });

const mobileContext = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
const mobile = await mobileContext.newPage();
watch(mobile, "mobile");
await mobile.goto("http://127.0.0.1:39977/?agent=1");
await ready(mobile);
await mobile.locator("#start").click();
const up = mobile.locator('[data-input="up"]');
const jump = mobile.locator('[data-input="jump"]');
checks.mobileControlsVisible = await up.isVisible() && await jump.isVisible();
const cdp = await mobileContext.newCDPSession(mobile);
async function touchStart(locator, id) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("touch control has no bounding box");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: box.x + box.width / 2, y: box.y + box.height / 2, id, radiusX: 4, radiusY: 4, force: 1 }] });
}
async function touchEnd() {
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
}
const beforeTouch = await command(mobile, { op: "state" });
await touchStart(up, 20);
await command(mobile, { op: "step", ticks: 36 });
await touchEnd();
const afterTouch = await command(mobile, { op: "state" });
checks.mobileDirectionalTouch = afterTouch.player.z < beforeTouch.player.z - 2;
await touchStart(jump, 21);
await command(mobile, { op: "step", ticks: 10 });
await touchEnd();
const afterTouchJump = await command(mobile, { op: "state" });
checks.mobileJumpTouch = afterTouchJump.player.y > .3 && afterTouchJump.events.some((event) => event.type === "jump");
const mobileWholeSeconds = Math.ceil(afterTouchJump.remaining);
const mobileExpectedTime = `${Math.floor(mobileWholeSeconds / 60)}:${String(mobileWholeSeconds % 60).padStart(2, "0")}`;
checks.mobileTimerDisplay = (await mobile.locator("#time").textContent())?.trim() === mobileExpectedTime;
await mobile.waitForTimeout(100);
await mobile.screenshot({ path: `${output}/babylon-mobile.png` });

const report = {
  engine: "Babylon.js",
  url: "http://127.0.0.1:39977/?agent=1",
  method: "Real DOM clicks, Playwright keyboard events, and Chrome touch input; adapter used only for state reads and fixed-tick advancement.",
  checks,
  errors,
  desktop: { beforeKeyboard, afterKeyboard, afterJump, saved, loaded },
  mobile: { beforeTouch, afterTouch, afterTouchJump },
  observedFailures: []
};
report.passed = Object.values(checks).every(Boolean) && errors.length === 0;
writeFileSync(`${output}/babylon-ui.json`, JSON.stringify(report, null, 2));
console.log(JSON.stringify({ passed: report.passed, checks, errors }, null, 2));
await browser.close();
if (!report.passed) process.exitCode = 1;
