// The three.js twin of game/bench (its README defines the scenes): the same cubes,
// the same camera, the same measurement, reported to the runner that opened the page.
//   bench.html?scene=cubes&n=10000&mode=instanced&renderer=webgl&seconds=10
// Modes of `cubes`: `meshes` (a Mesh each — what a first three.js project does),
// `instanced` (one InstancedMesh, matrices set from JS every frame).
const q = new URLSearchParams(location.search);
const n = Number(q.get('n') ?? 10000), mode = q.get('mode') ?? 'instanced';
const seconds = Number(q.get('seconds') ?? 10), warmup = Number(q.get('warmup') ?? 2);
const which = q.get('renderer') ?? 'webgl';
const THREE = await import(which === 'webgpu' ? 'three/webgpu' : 'three');

const renderer = which === 'webgpu' ? new THREE.WebGPURenderer({ antialias: true }) : new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(2);
renderer.setSize(1280, 720, false);
document.body.append(renderer.domElement);
if (renderer.init) await renderer.init();

const frac = (x) => x - Math.floor(x);
const side = Math.ceil(Math.cbrt(n)), half = (side - 1) * 0.5, radius = side * 2 * 0.9 + 6;
const scene = new THREE.Scene();
scene.background = new THREE.Color(0.05, 0.06, 0.09);
scene.add(new THREE.AmbientLight(new THREE.Color(0.35, 0.38, 0.45), Math.PI));
const sun = new THREE.DirectionalLight(0xffffff, Math.PI);
sun.position.set(0.38, 0.77, 0.51);
scene.add(sun);
const camera = new THREE.PerspectiveCamera(60, 1280 / 720, 0.1, radius * 4);

const positions = [], axes = [], speeds = [];
for (let i = 0; i < n; i++) {
  const x = i % side, y = Math.floor(i / side) % side, z = Math.floor(i / (side * side));
  positions.push(new THREE.Vector3((x - half) * 2, (y - half) * 2, (z - half) * 2));
  const a = new THREE.Vector3(frac(i * 0.3719) - 0.5, frac(i * 0.7331) - 0.5, frac(i * 0.1913) - 0.5);
  axes.push(a.length() > 1e-4 ? a.normalize() : new THREE.Vector3(0, 1, 0));
  speeds.push(0.5 + (i % 7) * 0.25);
}
const geometry = new THREE.BoxGeometry(1, 1, 1);
const colorOf = (i) => new THREE.Color().setHSL(frac(i * 0.61803), 0.6, 0.6);
let meshes = null, instanced = null;
if (mode === 'meshes') {
  meshes = positions.map((p, i) => {
    const m = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: colorOf(i), roughness: 0.6 }));
    m.position.copy(p);
    scene.add(m);
    return m;
  });
} else {
  instanced = new THREE.InstancedMesh(geometry, new THREE.MeshStandardMaterial({ roughness: 0.6 }), n);
  instanced.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  instanced.frustumCulled = false;
  for (let i = 0; i < n; i++) instanced.setColorAt(i, colorOf(i));
  scene.add(instanced);
}

const frames = [], script = [], quat = new THREE.Quaternion(), mat = new THREE.Matrix4(), one = new THREE.Vector3(1, 1, 1);
let last = performance.now(), elapsed = 0, done = false;
function frame() {
  if (done) return;
  const now = performance.now(), dt = now - last;
  last = now;
  elapsed += dt / 1000;
  const t = elapsed, ang = t * Math.PI * 2 / 20;
  camera.position.set(Math.cos(ang) * radius, radius * 0.35, Math.sin(ang) * radius);
  camera.lookAt(0, 0, 0);
  if (meshes) for (let i = 0; i < n; i++) meshes[i].quaternion.setFromAxisAngle(axes[i], t * speeds[i]);
  else {
    for (let i = 0; i < n; i++) {
      quat.setFromAxisAngle(axes[i], t * speeds[i]);
      instanced.setMatrixAt(i, mat.compose(positions[i], quat, one));
    }
    instanced.instanceMatrix.needsUpdate = true;
  }
  const scripted = performance.now() - now;
  renderer.render(scene, camera);
  if (elapsed >= warmup) { frames.push(dt); script.push(scripted); }
  if (elapsed >= warmup + seconds) { done = true; report(); return; }
  requestAnimationFrame(frame);
}
function report() {
  const sorted = [...frames].sort((a, b) => a - b), pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] ?? 0;
  const avg = (a) => a.reduce((s, v) => s + v, 0) / Math.max(a.length, 1), r2 = (x) => Math.round(x * 100) / 100;
  const out = {
    engine: 'three', version: THREE.REVISION, renderer: which, scene: 'cubes', n, mode,
    pixels: [renderer.domElement.width, renderer.domElement.height], frames: frames.length,
    fps_avg: Math.round(10000 / avg(frames)) / 10, ms_p50: r2(pct(0.5)), ms_p95: r2(pct(0.95)), ms_p99: r2(pct(0.99)),
    ms_max: r2(sorted[sorted.length - 1] ?? 0), script_ms_avg: r2(avg(script)), draws: renderer.info?.render?.calls ?? renderer.info?.render?.drawCalls ?? null,
  };
  document.title = 'BENCH ' + JSON.stringify(out);
  navigator.sendBeacon('/__bench', JSON.stringify(out));
}
requestAnimationFrame(frame);
