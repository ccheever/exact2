// The GPU modules' router (host code, LLP 1009 D6): what glue.js injects in
// place of gpu-glue.js when the app declares GPU modules (`gpu.modules` in
// app.json, baked as `compat.inputs.gpuModules`). It is `exact.gpu` and sends
// each canvas to the artifact that owns its surface name — a declared
// module's `gpu/<name>.js`, else the primary `gpu.js` — loading an artifact
// only when one of its canvases mounts, after a rendering opportunity (D4, per
// artifact). Each artifact is its own gpu-glue.js instance, imported as
// `gpu-glue.js?artifact=<stem>`: its own device, surfaces, recovery and frame
// loop. Everything addressed to a view goes to that view's artifact; the rest
// fans out, and replies that list worlds are joined.
const exact = globalThis.exact;
const owners = new Map(Object.entries(exact.compat.inputs.gpuModules)
  .flatMap(([module, names]) => names.map(name => [name, `gpu/${module}`])));
const stemOf = name => owners.get(name) ?? "gpu";
const hosts = new Map(); // stem -> a registered gpu-glue.js instance
const loading = new Map(); // stem -> its import, from the moment it was needed
const queued = new Map(); // stem -> view -> [name, values], until its instance registers
const views = new Map(); // view -> stem
const afterPaint = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
function need(stem, deferred = true) {
  if (!loading.has(stem)) loading.set(stem, (deferred ? afterPaint() : Promise.resolve())
    .then(() => import(`./gpu-glue.js?artifact=${stem}`))
    .catch(error => { queued.delete(stem); exact.devError?.(String(error)); console.error(`exact gpu: ${stem}:`, error); }));
  return loading.get(stem);
}
const each = (method, ...args) => { for (const host of hosts.values()) host[method]?.(...args); };
const hostOf = view => hosts.get(views.get(view));
const posts = [];

const router = {
  register(stem, host) {
    hosts.set(stem, host);
    for (const [view, [name, values]] of queued.get(stem) ?? []) host.surface(view, name, values);
    queued.delete(stem);
    for (const [name, text, at] of posts.splice(0)) router.post(name, text, at);
  },
  // postMessage(name, text): to the artifact owning the surface, in order, held until it loads.
  post(name, text, at) {
    const host = hosts.get(stemOf(name));
    if (host) host.post(name, text, at); else posts.push([name, text, at]);
  },
  surface(view, name, values, deferred = true) {
    const stem = stemOf(name);
    if (views.has(view) && views.get(view) !== stem) this.destroy(view);
    views.set(view, stem);
    const host = hosts.get(stem);
    if (host) return host.surface(view, name, values);
    if (!queued.has(stem)) queued.set(stem, new Map());
    queued.get(stem).set(view, [name, values]);
    need(stem, deferred);
  },
  destroy(view) {
    const stem = views.get(view);
    views.delete(view);
    queued.get(stem)?.delete(view);
    hosts.get(stem)?.destroy(view);
  },
  async surfaceWork(name, mode, bytes, active) {
    const stem = stemOf(name);
    if (![...views.values()].includes(stem)) throw Object.assign(new Error(`surface ${name}: expected one live surface, found 0`), {kind:2});
    await need(stem);
    const host = hosts.get(stem);
    if (!host) throw Object.assign(new Error(`surface ${name}: module unavailable`), {kind:3});
    return host.surfaceWork(name, mode, bytes, active);
  },
  async settled() {
    await Promise.all(loading.values());
    return (await Promise.all([...hosts.values()].map(host => host.settled()))).flat();
  },
  decorate(request, reply) {
    for (const host of hosts.values()) reply = host.decorate(request, reply);
    return reply;
  },
  handle(request, ask, tagged) { return hostOf(request.id)?.handle(request, ask, tagged) ?? { error: `view ${request.id} has no world` }; },
  agent: (view, request) => hostOf(view)?.agent(view, request) ?? null,
  wantsInput: view => hostOf(view)?.wantsInput(view) === true,
  answers: request => request.entity !== undefined || request.world === true || request.contact !== undefined,
  clock(settle) {
    const parts = [...hosts.values()].map(host => host.clock(settle));
    const world = parts.flatMap(part => part.reply.world ?? []);
    const at = parts.map(part => part.settleAt).filter(Number.isFinite);
    return { pending: parts.some(part => part.pending), settleAt: at.length ? Math.max(...at) : undefined, reply: world.length ? { world } : {} };
  },
  async prepareShaders(assets) {
    await Promise.all(loading.values());
    const commits = await Promise.all([...hosts.values()].map(host => host.prepareShaders(assets)));
    return () => { for (const commit of commits) commit(); };
  },
  placementHidden: el => [...hosts.values()].some(host => host.placementHidden(el)),
  reset(carry) { each("reset", carry); queued.clear(); views.clear(); },
  // Dev only (dev.js): a module's rebuilt artifact; one not loaded yet reads
  // its version from `exact.gpuVersions` when it loads.
  swap(version, module) { return hosts.get(module ? `gpu/${module}` : "gpu")?.swap(version) ?? { ms: 0, errors: [] }; },
  // A lost device is one artifact's; the others answer "healthy" and keep theirs.
  deviceLost: () => each("deviceLost"),
  finishRestart: () => each("finishRestart"),
  drainRecords: () => each("drainRecords"),
  layout: () => each("layout"),
  schedule: () => each("schedule"),
  beforeStyle: el => each("beforeStyle", el),
  afterStyle: el => each("afterStyle", el),
};
exact.gpu = router;
// The canvases glue.js queued painted before this script arrived: their
// artifacts load now.
for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) router.surface(s.id, s.name, s.values, false);
exact.pendingSurfaces = [];
for (const p of exact.pendingPosts ?? []) if (p.generation === exact.generation) router.post(p.name, p.text, p.at);
exact.pendingPosts = [];
