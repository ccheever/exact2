import * as THREE from 'three';
import { DT, initial, key, step, MOVE_KEYS } from './simulation.js';
const params = new URLSearchParams(location.search);
const manual = params.has('proof');
let state = initial(Number(params.get('seed') ?? 20260917)), accumulator = 0;
const $ = id => document.getElementById(id);
const scene = new THREE.Scene();
scene.background = new THREE.Color('#7caaa2');
scene.fog = new THREE.Fog('#7caaa2', 40, 85);
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.PCFSoftShadowMap;
$('viewport').append(renderer.domElement);
const camera = new THREE.PerspectiveCamera(52, innerWidth / innerHeight, 0.1, 120);
const sun = new THREE.DirectionalLight('#fff3d4', 3);
sun.position.set(-12, 25, 8); sun.castShadow = true;
sun.shadow.mapSize.set(2048, 2048);
Object.assign(sun.shadow.camera, { left: -28, right: 28, top: 28, bottom: -28, far: 80 });
sun.shadow.normalBias = 0.035;
scene.add(sun);
const material = (color, extra = {}) => new THREE.MeshStandardMaterial({ color, roughness: 0.85, emissive: color, emissiveIntensity: 0.16, ...extra });
function mesh(geometry, mat, x, y, z) {
  const m = new THREE.Mesh(geometry, mat); m.position.set(x, y, z); m.castShadow = true; m.receiveShadow = true; scene.add(m); return m;
}
const ground = mesh(new THREE.PlaneGeometry(40, 40), material('#527b65'), 0, 0, 0);
ground.rotation.x = -Math.PI / 2; ground.castShadow = false;
const grid = new THREE.GridHelper(40, 20, '#87a77e', '#648871'); grid.position.y = .008; scene.add(grid);
const player = mesh(new THREE.CapsuleGeometry(.4, 1, 8, 16), material('#f6eed4'), 0, .9, 0);
const crateMeshes = Array.from({ length: 6 }, () => mesh(new THREE.BoxGeometry(1, 1, 1), material('#ac7950'), 0, .5, 0));
const beaconMeshes = state.beacons.map(b => {
  mesh(new THREE.CylinderGeometry(.72, .85, .12, 32), material('#3e5d51'), b.x, .06, b.z);
  const ring = mesh(new THREE.RingGeometry(1.44, 1.5, 64), material('#cfcd92', { side: THREE.DoubleSide }), b.x, .016, b.z);
  ring.rotation.x = -Math.PI / 2; ring.castShadow = false;
  const ball = mesh(new THREE.SphereGeometry(.5, 32, 20), material('#718f83', { emissive: '#ffbe45', emissiveIntensity: 0 }), b.x, 1, b.z);
  // A transparent sphere gives the emissive core a visible halo without postprocessing.
  const halo = mesh(new THREE.SphereGeometry(.72, 24, 16), new THREE.MeshBasicMaterial({ color: '#ffd176', transparent: true, opacity: 0, depthWrite: false }), b.x, 1, b.z);
  halo.castShadow = false; halo.receiveShadow = false;
  return { ball, halo };
});
function sync() {
  const active = state.mode !== 'title';
  $('title').hidden = active; $('hud').hidden = !active; $('help').hidden = !active;
  $('win').hidden = state.mode !== 'won'; $('paused').hidden = !state.paused;
  $('pause').hidden = state.mode === 'won'; $('pause').textContent = state.paused ? 'Resume' : 'Pause';
  $('count').textContent = `Beacons ${state.beacons.filter(b => b.lit).length} / 3`;
  const nearby = state.beacons.find(b => !b.lit && Math.hypot(state.player.x - b.x, state.player.z - b.z) <= 1.5);
  $('prompt').textContent = nearby ? 'Press E to light this beacon' : 'Find the three beacons';
  player.position.set(state.player.x, state.player.y + .9, state.player.z);
  crateMeshes.forEach((m, i) => m.position.set(state.crates[i].x, .5, state.crates[i].z));
  beaconMeshes.forEach(({ ball, halo }, i) => {
    const g = state.beacons[i].glow;
    ball.material.color.setRGB(.24 + .76 * g, .4 + .36 * g, .32 + .04 * g);
    ball.material.emissiveIntensity = 2 * g; halo.material.opacity = .16 * g;
  });
  const c = state.camera; camera.position.set(c.x, c.y, c.z); camera.lookAt(c.tx, c.ty, c.tz);
  renderer.render(scene, camera);
}
function play() { state = initial(state.seed); state.mode = 'playing'; accumulator = 0; sync(); $('game').focus(); }
$('play').onclick = play; $('again').onclick = play;
$('pause').onclick = () => {
  state.paused = !state.paused; state.keys = []; state.jump = false; state.interact = false;
  accumulator = 0; sync(); if (!state.paused) $('game').focus();
};
const codes = [...MOVE_KEYS, 'Space', 'KeyE'];
addEventListener('keydown', e => {
  if (!codes.includes(e.code) || e.target instanceof HTMLButtonElement) return;
  e.preventDefault(); key(state, e.code, true);
});
addEventListener('keyup', e => { if (codes.includes(e.code)) { e.preventDefault(); key(state, e.code, false); } });
addEventListener('blur', () => { state.keys = []; state.jump = false; state.interact = false; });
addEventListener('resize', () => { renderer.setSize(innerWidth, innerHeight); camera.aspect = innerWidth / innerHeight; camera.updateProjectionMatrix(); sync(); });
renderer.setSize(innerWidth, innerHeight);
let previous = performance.now();
function frame(now) {
  const elapsed = (now - previous) / 1000; previous = now;
  if (!manual && state.mode === 'playing' && !state.paused) {
    accumulator += Math.min(elapsed, .25);
    while (accumulator >= DT) { step(state); accumulator -= DT; }
  }
  sync(); requestAnimationFrame(frame);
}
// Save every causal simulation value, including camera, PRNG, held input and pending edges.
window.beacons = {
  revision: THREE.REVISION,
  state: () => structuredClone(state),
  save: () => JSON.stringify({ state, accumulator }),
  restore: json => {
    const saved = JSON.parse(json);
    if (saved.state.version !== 1) throw new Error('Unsupported Beacons save');
    state = saved.state; accumulator = saved.accumulator; sync(); $('game').focus();
  },
  advance: ticks => {
    if (!manual || !Number.isInteger(ticks) || ticks < 0) throw new Error('Explicit integer ticks require ?proof');
    for (let i = 0; i < ticks; i++) step(state);
    sync(); return structuredClone(state);
  },
  rendered: () => ({ emissive: beaconMeshes.map(b => b.ball.material.emissiveIntensity), calls: renderer.info.render.calls }),
};
sync(); $('play').focus(); requestAnimationFrame(frame);
