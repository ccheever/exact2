import { chromium } from "@playwright/test";
import { writeFile } from "node:fs/promises";

const url = process.env.BASE_URL || "http://127.0.0.1:34541/";
const executablePath = process.env.CHROMIUM_PATH || "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const started = performance.now();
const browser = await chromium.launch({
  headless: true,
  executablePath,
  args: ["--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => {
  if (message.type() === "error") errors.push(message.text());
});

const command = (request) => page.evaluate((value) => window.lanterns.command(value), request);
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const evidence = { engine: "Babylon.js 9.27.0", physics: "Havok 1.3.10", url, checks: [] };
try {
  await page.goto(`${url}?agent=1`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.lanterns?.command, null, { timeout: 30000 });
  const ready = await command({ op: "ready" });
  assert(ready.ready && ready.version === "9.27.0", "adapter did not become ready");
  evidence.checks.push("assets and adapter ready");
  await page.screenshot({ path: "evidence/title.png" });

  let value = await command({ op: "start" });
  assert(value.phase === "playing" && value.count === 0, "fresh start failed");
  await command({ op: "input", moveZ: -1 });
  value = await command({ op: "step", ticks: 60 });
  assert(value.player.z < 8 && value.player.z > 6.5, `fixed movement ended at z=${value.player.z}`);
  assert(value.player.animation.toLowerCase().includes("run"), `active clip is ${value.player.animation}`);
  await command({ op: "input", act: true });
  value = await command({ op: "step", ticks: 1 });
  assert(value.count === 1 && value.lanterns.find((l) => l.id === "lantern-11").lit, "nearest lantern was not lit");
  evidence.checks.push("fixed-step movement, Run clip, and press-edge collection");

  await command({ op: "input" });
  await command({ op: "step", ticks: 1 });
  await command({ op: "input", jump: true });
  value = await command({ op: "step", ticks: 5 });
  assert(value.player.y > 0.15 && value.events.some((e) => e.type === "jump"), "jump did not use physics velocity");
  evidence.checks.push("jump and gravity through Havok");
  await page.screenshot({ path: "evidence/gameplay.png" });

  const beforePause = value.ticks;
  await command({ op: "pause" });
  value = await command({ op: "step", ticks: 120 });
  assert(value.phase === "paused" && value.ticks === beforePause, "pause advanced simulation");
  await page.screenshot({ path: "evidence/paused.png" });
  await command({ op: "pause" });
  evidence.checks.push("pause freezes the simulation clock");

  await command({ op: "reset" });
  await command({ op: "start" });
  await command({ op: "input", moveZ: -1 });
  await command({ op: "step", ticks: 54 });
  await command({ op: "input", moveX: 1 });
  value = await command({ op: "step", ticks: 100 });
  assert(value.crate.x > 6.35, `dynamic crate did not move: x=${value.crate.x}`);
  evidence.checks.push("player collision pushes the dynamic crate");

  const saved = await command({ op: "save" });
  assert(saved.saved, "save did not confirm persistence");
  const snapshot = await command({ op: "state" });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.lanterns?.command, null, { timeout: 30000 });
  value = await command({ op: "load" });
  assert(value.ticks === snapshot.ticks, "reload changed saved ticks");
  assert(Math.abs(value.player.x - snapshot.player.x) < 0.01, "reload changed player position");
  assert(Math.abs(value.crate.x - snapshot.crate.x) < 0.01, "reload changed crate position");
  value = await command({ op: "step", ticks: 1 });
  assert(Math.abs(value.player.x - snapshot.player.x) < 0.1, "restored player body was not moved in Havok");
  assert(Math.abs(value.crate.x - snapshot.crate.x) < 0.1, "restored crate body was not moved in Havok");
  evidence.checks.push("localStorage restore after page process reload");

  await command({ op: "start" });
  await command({ op: "input", moveZ: -1 });
  const batch = await command({ op: "step", ticks: 60 });
  await command({ op: "start" });
  await command({ op: "input", moveZ: -1 });
  for (let i = 0; i < 60; i++) value = await command({ op: "step", ticks: 1 });
  assert(Math.abs(batch.player.z - value.player.z) < 0.00001, `batched ticks differ: ${batch.player.z} vs ${value.player.z}`);
  evidence.checks.push("one 60-tick batch equals sixty one-tick steps");

  await command({ op: "reset" });
  await command({ op: "start" });
  value = await command({ op: "step", ticks: 180 * 60 });
  assert(value.phase === "lost" && value.events.some((e) => e.type === "lose"), "countdown did not reach lose state");
  evidence.checks.push("three-minute terminal lose state");
  assert(errors.length === 0, `browser errors: ${errors.join(" | ")}`);
  evidence.passed = true;
} catch (error) {
  evidence.passed = false;
  evidence.failure = error.stack;
  evidence.browserErrors = errors;
  process.exitCode = 1;
} finally {
  evidence.elapsedMs = Math.round(performance.now() - started);
  evidence.recordedAt = new Date().toISOString();
  await writeFile("evidence/self-test.json", JSON.stringify(evidence, null, 2));
  await browser.close();
}

console.log(JSON.stringify(evidence, null, 2));
