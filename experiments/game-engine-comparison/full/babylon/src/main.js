import { Vector3 } from "@babylonjs/core";
import { createWorld } from "./world.js";

const TICK_RATE = 60;
const DT = 1 / TICK_RATE;
const SAVE_KEY = "lanterns-babylon-save-v1";
const agentMode = new URLSearchParams(location.search).get("agent") === "1";
const $ = (id) => document.getElementById(id);
const ui = {
  loading: $("loading"), title: $("title"), hud: $("hud"), modal: $("modal"),
  count: $("count"), time: $("time"), pause: $("pause"), sound: $("sound"),
  toast: $("toast"), sign: $("sign-copy"), touch: $("touch"),
  modalKicker: $("modal-kicker"), modalTitle: $("modal-title"), modalCopy: $("modal-copy"),
  modalMain: $("modal-main"), save: $("save"), restart: $("restart"), resume: $("resume"),
};

const level = await fetch("./level.json").then((r) => r.json());
const world = await createWorld($("world"), level);
let phase = "title";
let ticks = 0;
let events = [];
let soundEnabled = true;
let audioContext = null;
let toastTimer = null;
let input = { moveX: 0, moveZ: 0, jump: false, act: false };
let jumpEdge = false;
let actEdge = false;
const keys = new Set();
const touchHeld = new Set();

function event(type, extra = {}) {
  events.push({ type, tick: ticks, ...extra });
  if (events.length > 96) events = events.slice(-96);
}

function tone(frequency, duration = 0.1, delay = 0) {
  if (!soundEnabled) return;
  audioContext ||= new AudioContext();
  if (audioContext.state === "suspended") audioContext.resume();
  const oscillator = audioContext.createOscillator();
  const gain = audioContext.createGain();
  oscillator.type = "sine";
  oscillator.frequency.value = frequency;
  gain.gain.setValueAtTime(0.0001, audioContext.currentTime + delay);
  gain.gain.exponentialRampToValueAtTime(0.12, audioContext.currentTime + delay + 0.012);
  gain.gain.exponentialRampToValueAtTime(0.0001, audioContext.currentTime + delay + duration);
  oscillator.connect(gain).connect(audioContext.destination);
  oscillator.start(audioContext.currentTime + delay);
  oscillator.stop(audioContext.currentTime + delay + duration + 0.02);
}

function notify(message) {
  ui.toast.textContent = message;
  ui.toast.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => ui.toast.classList.remove("show"), 1500);
}

function setInput(next) {
  const normalized = {
    moveX: Math.max(-1, Math.min(1, Number(next.moveX) || 0)),
    moveZ: Math.max(-1, Math.min(1, Number(next.moveZ) || 0)),
    jump: next.jump === true,
    act: next.act === true,
  };
  if (normalized.jump && !input.jump) jumpEdge = true;
  if (normalized.act && !input.act) actEdge = true;
  input = normalized;
}

function clearInput() {
  keys.clear();
  touchHeld.clear();
  setInput({});
  jumpEdge = false;
  actEdge = false;
}

function bodyState(body) {
  const v = world.velocity(body);
  return { vx: v.x, vy: v.y, vz: v.z };
}

function rounded(value) {
  return Number(value.toFixed(6));
}

function state() {
  const p = world.feet();
  const pv = bodyState(world.player.body);
  const cv = bodyState(world.crate.body);
  const c = world.crateMesh.position;
  const count = world.lanterns.filter((l) => l.lit).length;
  return {
    phase,
    ticks,
    elapsed: rounded(ticks / TICK_RATE),
    remaining: rounded(Math.max(0, level.duration - ticks / TICK_RATE)),
    player: {
      x: rounded(p.x), y: rounded(p.y), z: rounded(p.z),
      vx: rounded(pv.vx), vy: rounded(pv.vy), vz: rounded(pv.vz),
      grounded: world.grounded(), animation: world.getAnimation(),
    },
    crate: {
      x: rounded(c.x), y: rounded(c.y), z: rounded(c.z),
      vx: rounded(cv.vx), vy: rounded(cv.vy), vz: rounded(cv.vz),
    },
    lanterns: world.lanterns.map((l) => ({ id: l.id, x: l.position[0], y: l.position[1], z: l.position[2], lit: l.lit })),
    count,
    total: world.lanterns.length,
    events: events.map((e) => ({ ...e })),
    assetReady: true,
  };
}

function formatTime(seconds) {
  const whole = Math.ceil(Math.max(0, seconds));
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

function syncUI() {
  const s = state();
  ui.count.textContent = `${s.count} / ${s.total}`;
  ui.time.textContent = formatTime(s.remaining);
  ui.hud.hidden = phase === "title";
  ui.touch.hidden = phase !== "playing";
  ui.title.hidden = phase !== "title";
  ui.pause.textContent = phase === "paused" ? "Resume" : "Pause";
  ui.resume.hidden = !localStorage.getItem(SAVE_KEY);
  ui.modal.hidden = phase === "playing" || phase === "title";
  ui.save.hidden = phase !== "paused";
  ui.restart.hidden = phase === "paused";
  if (phase === "paused") {
    ui.modalKicker.textContent = "THE SUN WAITS";
    ui.modalTitle.textContent = "Paused";
    ui.modalCopy.textContent = `${s.count} lanterns lit with ${formatTime(s.remaining)} left.`;
    ui.modalMain.textContent = "Return to island";
  } else if (phase === "won") {
    ui.modalKicker.textContent = "ALL TWELVE BURN BRIGHT";
    ui.modalTitle.textContent = "Dusk held back";
    ui.modalCopy.textContent = `The fox lit the island with ${formatTime(s.remaining)} to spare.`;
    ui.modalMain.textContent = "Run again";
  } else if (phase === "lost") {
    ui.modalKicker.textContent = "NIGHT REACHED THE SHORE";
    ui.modalTitle.textContent = "The light faded";
    ui.modalCopy.textContent = `${s.count} of ${s.total} lanterns were lit.`;
    ui.modalMain.textContent = "Try again";
  }
  const signDistance = Math.hypot(s.player.x - level.sign.position[0], s.player.z - level.sign.position[2]);
  ui.sign.classList.toggle("show", phase === "playing" && signDistance < 4.2);
}

function teleportDefaults() {
  world.teleport(
    world.player,
    world.playerMesh,
    [level.spawn[0], level.spawn[1] + level.player.height / 2, level.spawn[2]],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0, 1],
  );
  world.teleport(world.crate, world.crateMesh, level.crate.position, [0, 0, 0], [0, 0, 0], [0, 0, 0, 1]);
}

function fresh(nextPhase = "playing") {
  ticks = 0;
  events = [];
  phase = nextPhase;
  clearInput();
  teleportDefaults();
  for (const lantern of world.lanterns) lantern.setLit(false);
  world.setAnimation("Survey");
  syncUI();
  world.render();
  return state();
}

function collectNearest() {
  const p = world.feet();
  let candidate = null;
  let distance = Infinity;
  for (const lantern of world.lanterns) {
    if (lantern.lit) continue;
    const d = Vector3.Distance(p, new Vector3(...lantern.position));
    if (d <= 1.5 && d < distance) {
      candidate = lantern;
      distance = d;
    }
  }
  if (!candidate) return;
  candidate.setLit(true);
  event("lantern-lit", { id: candidate.id });
  tone(660, 0.18);
  tone(990, 0.22, 0.08);
  notify(`${candidate.id.replace("lantern-", "Lantern ")} lit`);
  if (world.lanterns.every((l) => l.lit)) {
    phase = "won";
    clearInput();
    event("win");
    tone(523, 0.5);
    tone(784, 0.7, 0.16);
  }
}

function tick() {
  if (phase !== "playing") return;
  ticks++;
  let x = input.moveX;
  let z = input.moveZ;
  const magnitude = Math.hypot(x, z);
  if (magnitude > 1) { x /= magnitude; z /= magnitude; }
  const speed = level.player.speed;
  const velocity = world.velocity(world.player.body);
  const wasGrounded = world.grounded();
  velocity.x = x * speed;
  velocity.z = z * speed;
  if (jumpEdge && wasGrounded) {
    velocity.y = level.player.jump;
    event("jump");
    tone(280, 0.08);
  }
  jumpEdge = false;
  world.player.body.setLinearVelocity(velocity);
  world.player.body.setAngularVelocity(Vector3.Zero());
  world.stepPhysics();

  if (world.feet().y < -6) {
    world.teleport(
      world.player,
      world.playerMesh,
      [level.spawn[0], level.spawn[1] + level.player.height / 2, level.spawn[2]],
      [0, 0, 0],
    );
    event("fall-reset");
    notify("The fox returned to shore");
  }
  if (actEdge) collectNearest();
  actEdge = false;

  const groundedNow = world.grounded();
  const amount = Math.hypot(x, z);
  const animation = amount < 0.05 ? "Survey" : amount < 0.62 ? "Walk" : "Run";
  world.setAnimation(groundedNow ? animation : "Run", x, z);
  if (phase === "playing" && ticks >= level.duration * TICK_RATE) {
    phase = "lost";
    clearInput();
    event("lose");
    tone(160, 0.8);
  }
  syncUI();
}

function save() {
  const s = state();
  const payload = {
    version: 1,
    phase: s.phase,
    ticks: s.ticks,
    player: { ...s.player },
    crate: { ...s.crate, rotation: [world.crateMesh.rotationQuaternion.x, world.crateMesh.rotationQuaternion.y, world.crateMesh.rotationQuaternion.z, world.crateMesh.rotationQuaternion.w] },
    lit: s.lanterns.filter((l) => l.lit).map((l) => l.id),
    events: s.events,
  };
  localStorage.setItem(SAVE_KEY, JSON.stringify(payload));
  event("save");
  syncUI();
  notify("Game saved");
  return { saved: true };
}

function load() {
  const raw = localStorage.getItem(SAVE_KEY);
  if (!raw) return state();
  const saved = JSON.parse(raw);
  if (saved.version !== 1) throw new Error("Unsupported Lanterns save version");
  ticks = Math.max(0, Math.min(level.duration * TICK_RATE, Number(saved.ticks) || 0));
  phase = ["playing", "paused", "won", "lost"].includes(saved.phase) ? saved.phase : "playing";
  events = Array.isArray(saved.events) ? saved.events.slice(-95) : [];
  world.teleport(
    world.player,
    world.playerMesh,
    [saved.player.x, saved.player.y + level.player.height / 2, saved.player.z],
    [saved.player.vx, saved.player.vy, saved.player.vz],
  );
  world.teleport(
    world.crate,
    world.crateMesh,
    [saved.crate.x, saved.crate.y, saved.crate.z],
    [saved.crate.vx, saved.crate.vy, saved.crate.vz],
    [0, 0, 0],
    saved.crate.rotation,
  );
  const lit = new Set(saved.lit);
  for (const lantern of world.lanterns) lantern.setLit(lit.has(lantern.id));
  clearInput();
  event("load");
  syncUI();
  world.render();
  return state();
}

function pause() {
  if (phase === "playing") phase = "paused";
  else if (phase === "paused") phase = "playing";
  clearInput();
  syncUI();
  return state();
}

function keyboardInput() {
  setInput({
    moveX: (keys.has("KeyD") || keys.has("ArrowRight") || touchHeld.has("right") ? 1 : 0) - (keys.has("KeyA") || keys.has("ArrowLeft") || touchHeld.has("left") ? 1 : 0),
    moveZ: (keys.has("KeyS") || keys.has("ArrowDown") || touchHeld.has("down") ? 1 : 0) - (keys.has("KeyW") || keys.has("ArrowUp") || touchHeld.has("up") ? 1 : 0),
    jump: keys.has("Space") || touchHeld.has("jump"),
    act: keys.has("KeyE") || touchHeld.has("act"),
  });
}

addEventListener("keydown", (e) => {
  if (["Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.code)) e.preventDefault();
  if (e.code === "Escape") { pause(); return; }
  keys.add(e.code);
  keyboardInput();
});
addEventListener("keyup", (e) => { keys.delete(e.code); keyboardInput(); });
addEventListener("blur", clearInput);
addEventListener("resize", world.resize);
for (const button of document.querySelectorAll("[data-input]")) {
  const name = button.dataset.input;
  const down = (e) => { e.preventDefault(); touchHeld.add(name); keyboardInput(); };
  const up = (e) => { e.preventDefault(); touchHeld.delete(name); keyboardInput(); };
  button.addEventListener("pointerdown", down);
  button.addEventListener("pointerup", up);
  button.addEventListener("pointercancel", up);
  button.addEventListener("pointerleave", up);
}

$("start").onclick = () => { tone(440); fresh("playing"); };
ui.resume.onclick = () => { tone(440); load(); };
ui.pause.onclick = pause;
ui.modalMain.onclick = () => phase === "paused" ? pause() : fresh("playing");
ui.restart.onclick = () => fresh("playing");
ui.save.onclick = save;
ui.sound.onclick = () => {
  soundEnabled = !soundEnabled;
  ui.sound.textContent = soundEnabled ? "Sound on" : "Sound off";
  if (soundEnabled) tone(520);
};

window.lanterns = {
  async command(request = {}) {
    switch (request.op) {
      case "ready": return { ready: true, engine: "Babylon.js", version: "9.27.0" };
      case "start": return fresh("playing");
      case "reset": return fresh("title");
      case "input": setInput(request); return state();
      case "step": {
        const amount = Math.max(0, Math.floor(Number(request.ticks) || 0));
        for (let i = 0; i < amount; i++) tick();
        world.render();
        return state();
      }
      case "state": return state();
      case "pause": return pause();
      case "save": return save();
      case "load": return load();
      default: throw new Error(`Unknown lanterns operation: ${request.op}`);
    }
  },
};

ui.loading.hidden = true;
ui.title.hidden = false;
syncUI();
world.render();

let previous = performance.now();
let accumulator = 0;
function frame(now) {
  const delta = Math.min(0.1, (now - previous) / 1000);
  previous = now;
  if (!agentMode && phase === "playing") {
    accumulator += delta;
    while (accumulator >= DT) { tick(); accumulator -= DT; }
  }
  world.render();
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
