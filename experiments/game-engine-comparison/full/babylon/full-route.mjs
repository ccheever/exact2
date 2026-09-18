import { chromium } from "@playwright/test";
import { writeFile } from "node:fs/promises";

const url = process.env.BASE_URL || "http://127.0.0.1:39977/";
const executablePath = process.env.CHROMIUM_PATH || "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const browser = await chromium.launch({
  headless: true,
  executablePath,
  args: ["--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const command = (request) => page.evaluate((value) => window.lanterns.command(value), request);
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const proof = { route: [], passed: false, url };

async function moveTo(x, z, tolerance = 0.48, maxTicks = 900) {
  let used = 0;
  while (used < maxTicks) {
    const value = await command({ op: "state" });
    const dx = x - value.player.x;
    const dz = z - value.player.z;
    const distance = Math.hypot(dx, dz);
    if (distance <= tolerance) {
      await command({ op: "input" });
      await command({ op: "step", ticks: 1 });
      proof.route.push({ type: "waypoint", x, z, reached: value.player });
      return value;
    }
    await command({ op: "input", moveX: dx / distance, moveZ: dz / distance });
    await command({ op: "step", ticks: Math.min(4, maxTicks - used) });
    used += 4;
  }
  const value = await command({ op: "state" });
  throw new Error(`waypoint ${x},${z} not reached from ${value.player.x},${value.player.z}`);
}

async function light(id) {
  const before = await command({ op: "state" });
  await command({ op: "input" });
  await command({ op: "step", ticks: 1 });
  await command({ op: "input", act: true });
  const after = await command({ op: "step", ticks: 1 });
  await command({ op: "input" });
  await command({ op: "step", ticks: 1 });
  assert(after.count === before.count + 1, `${id} did not light at ${after.player.x},${after.player.y},${after.player.z}`);
  assert(after.lanterns.find((entry) => entry.id === id)?.lit, `${id} missing from live lantern state`);
  proof.route.push({ type: "lit", id, tick: after.ticks, player: after.player });
}

try {
  await page.goto(`${url}?agent=1`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.lanterns?.command, null, { timeout: 30000 });
  await command({ op: "start" });

  const route = [
    ["lantern-11", 0, 7],
    ["lantern-1", -12, 10],
    ["lantern-2", -10, 2],
    ["lantern-3", -12, -8],
    ["lantern-4", -6, -12],
    ["lantern-5", 0, -10],
    ["lantern-6", 6, -12],
    ["lantern-7", 12, -8],
    ["lantern-8", 12, 0],
    ["lantern-9", 5, 2],
    ["lantern-10", 0, 0],
  ];
  for (const [id, x, z] of route) {
    await moveTo(x, z);
    await light(id);
  }

  await moveTo(4.8, 8, 0.25);
  const crateBefore = (await command({ op: "state" })).crate.x;
  await command({ op: "input", moveX: 1 });
  await command({ op: "step", ticks: 44 });
  await command({ op: "input" });
  let value = await command({ op: "step", ticks: 5 });
  assert(value.crate.x > crateBefore + 0.7, `crate push was too short: ${crateBefore} -> ${value.crate.x}`);
  proof.route.push({ type: "crate-pushed", crate: value.crate, player: value.player });

  await moveTo(value.crate.x - 1.1, value.crate.z - 1.8, 0.25);
  value = await command({ op: "state" });
  await moveTo(value.crate.x - 1.06, value.crate.z, 0.22);
  value = await command({ op: "state" });
  proof.route.push({ type: "west-of-crate", crate: value.crate, player: value.player });

  await command({ op: "input", moveX: 0.3, jump: true });
  await command({ op: "step", ticks: 1 });
  await command({ op: "input", moveX: 0.3 });
  await command({ op: "step", ticks: 42 });
  await command({ op: "input" });
  value = await command({ op: "step", ticks: 10 });
  for (let i = 0; i < 24 && !value.player.grounded; i++) value = await command({ op: "step", ticks: 1 });
  assert(value.player.grounded && value.player.y > 0.7, `fox did not land on crate: y=${value.player.y}`);
  proof.route.push({ type: "on-crate", player: value.player, crate: value.crate });

  await command({ op: "input", moveX: 0.55, jump: true });
  await command({ op: "step", ticks: 1 });
  await command({ op: "input", moveX: 0.55 });
  await command({ op: "step", ticks: 38 });
  await command({ op: "input" });
  value = await command({ op: "step", ticks: 22 });
  assert(value.player.y > 2.15 && value.player.x > 8, `fox did not reach ledge: x=${value.player.x} y=${value.player.y}`);
  proof.route.push({ type: "on-ledge", player: value.player });

  await moveTo(10, 8, 0.55, 180);
  await light("lantern-12");
  value = await command({ op: "state" });
  assert(value.phase === "won" && value.count === 12, `terminal state was ${value.phase} with ${value.count}`);
  assert(value.events.some((entry) => entry.type === "win"), "win event absent");
  await page.screenshot({ path: "evidence/won.png" });
  proof.passed = true;
  proof.final = value;
  assert(errors.length === 0, `page errors: ${errors.join(" | ")}`);
} catch (error) {
  proof.failure = error.stack;
  proof.lastState = await command({ op: "state" }).catch(() => null);
  proof.pageErrors = errors;
  await page.screenshot({ path: "evidence/route-failure.png" }).catch(() => {});
  process.exitCode = 1;
} finally {
  proof.recordedAt = new Date().toISOString();
  await writeFile("evidence/full-route.json", JSON.stringify(proof, null, 2));
  await browser.close();
}

console.log(JSON.stringify({ passed: proof.passed, route: proof.route, failure: proof.failure }, null, 2));
