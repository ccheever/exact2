export const DT = 1 / 120;
export const BEACONS = [[8, 0], [-6, 7], [3, -9]];
export const MOVE_KEYS = ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ArrowUp', 'ArrowLeft', 'ArrowDown', 'ArrowRight'];
export function initial(seed = 20260917) {
  let rng = seed >>> 0;
  const random = () => { rng = (Math.imul(1664525, rng) + 1013904223) >>> 0; return rng / 4294967296; };
  const crates = Array.from({ length: 6 }, () => ({ x: random() * 32 - 16, z: random() * 32 - 16 }));
  return { version: 1, seed, rng, crates, mode: 'title', paused: false, tick: 0,
    player: { x: 0, y: 0, z: 0, vx: 0, vy: 0, vz: 0, grounded: true },
    camera: { x: 10, y: 12, z: 16, tx: 0, ty: 0.7, tz: 0 },
    beacons: BEACONS.map(([x, z]) => ({ x, z, lit: false, age: 0, glow: 0 })),
    keys: [], jump: false, interact: false };
}
export function key(s, code, down) {
  if (s.mode !== 'playing' || s.paused) return;
  if (down && !s.keys.includes(code)) {
    s.keys.push(code); s.keys.sort();
    if (code === 'Space') s.jump = true;
    if (code === 'KeyE') s.interact = true;
  } else if (!down) s.keys = s.keys.filter(k => k !== code);
}
export function step(s) {
  if (s.mode !== 'playing' || s.paused) return;
  const p = s.player, held = (...codes) => codes.some(c => s.keys.includes(c));
  let x = +held('KeyD', 'ArrowRight') - +held('KeyA', 'ArrowLeft');
  let z = +held('KeyS', 'ArrowDown') - +held('KeyW', 'ArrowUp');
  const length = Math.hypot(x, z) || 1;
  x /= length; z /= length;
  const blend = 1 - Math.exp(-10 * DT);
  p.vx += (4 * x - p.vx) * blend;
  p.vz += (4 * z - p.vz) * blend;
  p.x = Math.max(-19.6, Math.min(19.6, p.x + p.vx * DT));
  p.z = Math.max(-19.6, Math.min(19.6, p.z + p.vz * DT));
  if (s.jump && p.grounded) { p.vy = Math.sqrt(2 * 12 * 1.2); p.grounded = false; }
  if (!p.grounded) {
    p.y += p.vy * DT - 6 * DT * DT;
    p.vy -= 12 * DT;
    if (p.y <= 0) { p.y = 0; p.vy = 0; p.grounded = true; }
  }
  if (s.interact && p.grounded) {
    const b = s.beacons.find(b => !b.lit && Math.hypot(p.x - b.x, p.z - b.z) <= 1.5);
    if (b) b.lit = true;
  }
  s.jump = false; s.interact = false;
  for (const b of s.beacons) if (b.lit) {
    b.age = Math.min(60, b.age + 1);
    const t = b.age / 60; b.glow = t * t * (3 - 2 * t);
  }
  const c = s.camera, follow = 1 - Math.exp(-5 * DT);
  for (const [field, goal] of Object.entries({ x: p.x + 10, y: p.y + 12, z: p.z + 16, tx: p.x, ty: p.y + 0.7, tz: p.z }))
    c[field] += (goal - c[field]) * follow;
  s.tick++;
  // Let the last beacon finish easing before presenting the win screen.
  if (s.beacons.every(b => b.glow === 1)) { s.mode = 'won'; s.keys = []; }
}
