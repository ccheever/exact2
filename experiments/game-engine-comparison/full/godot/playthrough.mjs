import { chromium } from "playwright";

const url = process.env.LANTERNS_URL ?? "http://127.0.0.1:4173/?agent=1";
const executablePath = process.env.CHROMIUM ??
  "/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome";
const browser = await chromium.launch({ headless: true, executablePath });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.goto(url);
await page.waitForFunction(() => window.lanterns);
const command = request => page.evaluate(request => window.lanterns.command(request), request);

async function moveTo(x, z, tolerance = 0.65) {
  for (let attempt = 0; attempt < 300; attempt++) {
    const state = await command({ op: "state" });
    const dx = x - state.player.x;
    const dz = z - state.player.z;
    const distance = Math.hypot(dx, dz);
    if (attempt > 0 && attempt % 50 === 0) console.log("MOVE_PROGRESS", x, z, attempt, state.player);
    if (distance <= tolerance) {
      await command({ op: "input" });
      await command({ op: "step", ticks: 1 });
      return await command({ op: "state" });
    }
    const n = Math.max(distance, 0.001);
    const ticks = Math.max(1, Math.min(10, Math.floor(distance / 4.5 * 60)));
    await command({ op: "input", moveX: dx / n, moveZ: dz / n });
    await command({ op: "step", ticks });
  }
  throw new Error(`failed to reach ${x},${z}`);
}

async function act() {
  await command({ op: "input" });
  await command({ op: "input", act: true });
  const state = await command({ op: "step", ticks: 1 });
  await command({ op: "input" });
  return state;
}

await command({ op: "start" });
for (const [x, z, id] of [
  [0, 7, "lantern-11"], [0, 0, "lantern-10"], [5, 2, "lantern-9"],
  [12, 0, "lantern-8"], [12, -8, "lantern-7"], [6, -12, "lantern-6"],
  [0, -10, "lantern-5"], [-6, -12, "lantern-4"], [-12, -8, "lantern-3"],
  [-10, 2, "lantern-2"], [-12, 10, "lantern-1"],
]) {
  await moveTo(x, z);
  const state = await act();
  if (!state.lanterns.find(item => item.id === id).lit) throw new Error(`failed to light ${id}`);
  console.log("LIT", id, state.player.x, state.player.z);
}

// Approach from the west and push the dynamic crate against the ledge.
await moveTo(3.8, 10);
await moveTo(4.5, 8, 0.4);
await command({ op: "input", moveX: 1 });
await command({ op: "step", ticks: 90 });
await command({ op: "input" });
let puzzle = await command({ op: "state" });
console.log("PUZZLE_PUSH", JSON.stringify({ player: puzzle.player, crate: puzzle.crate }));

// Jump onto the crate, then jump from it onto the 2.4-high ledge.
await command({ op: "input", moveX: 1, jump: true });
await command({ op: "step", ticks: 1 });
await command({ op: "input", moveX: 1 });
await command({ op: "step", ticks: 28 });
await command({ op: "input" });
await command({ op: "step", ticks: 18 });
puzzle = await command({ op: "state" });
console.log("PUZZLE_CRATE", JSON.stringify({ player: puzzle.player, crate: puzzle.crate }));
await command({ op: "input", moveX: 1, jump: true });
await command({ op: "step", ticks: 1 });
await command({ op: "input", moveX: 1 });
await command({ op: "step", ticks: 28 });
await command({ op: "input" });
await command({ op: "step", ticks: 18 });
puzzle = await command({ op: "state" });
console.log("PUZZLE_LEDGE", JSON.stringify({ player: puzzle.player, crate: puzzle.crate }));
await moveTo(10, 8, 1.0);
const won = await act();
if (won.phase !== "won" || won.count !== 12) throw new Error(`win route failed: ${JSON.stringify(won.player)} count=${won.count}`);
await page.screenshot({ path: "evidence/web-win.png" });
console.log("WIN_PASS", JSON.stringify({ ticks: won.ticks, player: won.player, crate: won.crate }));

// One batch and sixty single commands advance the same live engine world.
await command({ op: "reset" });
await command({ op: "start" });
await command({ op: "input", moveZ: -1 });
const batch = await command({ op: "step", ticks: 60 });
await command({ op: "reset" });
await command({ op: "start" });
await command({ op: "input", moveZ: -1 });
let singles;
for (let i = 0; i < 60; i++) singles = await command({ op: "step", ticks: 1 });
if (Math.abs(batch.player.z - singles.player.z) > 0.002 ||
    Math.abs(batch.crate.y - singles.crate.y) > 0.002) {
  throw new Error(`step equivalence failed: ${batch.player.z} / ${singles.player.z}`);
}
console.log("STEP_EQ_PASS", batch.player.z, singles.player.z);

await command({ op: "reset" });
await command({ op: "start" });
await command({ op: "input" });
const lost = await command({ op: "step", ticks: 10800 });
if (lost.phase !== "lost" || lost.ticks !== 10800 || lost.remaining !== 0) throw new Error("loss timer failed");
await page.screenshot({ path: "evidence/web-lost.png" });
console.log("LOSE_PASS", JSON.stringify({ ticks: lost.ticks, phase: lost.phase }));
await browser.close();
