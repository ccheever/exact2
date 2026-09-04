// The dev loop's page side (host code, served only by dev.mjs, never in
// dist/): a plan ready on the server → fetch it → restart the app from it.
// A reload is a restart (LLP 1004 D5): the page reboots from initial state.
// An asset changed (LLP 1030 D10, the asset row) → what referenced it is
// refreshed in place: an image re-fetches, a shader is handed to the GPU
// module (which validates it against the interface it binds), a deck page
// reloads its frame, a font restarts the app from the current plan so the
// faces re-register — carrying state, as every `{seq}` does.
const es = new EventSource("/__dev");
let overlay = null;
let seen = 0;
let queued = 0;
let reloads = Promise.resolve();
let lastPlan = null;
function show(message) {
  if (!message) { overlay?.remove(); overlay = null; return; }
  overlay ??= document.body.appendChild(Object.assign(document.createElement("pre"), { style: "position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647" }));
  overlay.textContent = message;
}
async function reload(m) {
  show(null);
  if (!globalThis.exact) throw new Error("the web host did not start");
  const t = performance.now();
  const response = await fetch(`/app.plan?seq=${m.seq}`, { cache: "no-store" });
  if (!response.ok) throw new Error(`the plan fetch failed: ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  lastPlan = bytes;
  const fetchMs = performance.now() - t;
  const bootMs = await globalThis.exact.reload(bytes);
  // The new plan's first frame is in the DOM: report now (synchronously — a
  // headless page may never get an animation frame), then the paint.
  navigator.sendBeacon(`/__dev/reloaded?seq=${m.seq}&dom=${Date.now()}&fetch=${fetchMs.toFixed(1)}&boot=${bootMs.toFixed(1)}`);
  requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?seq=${m.seq}&paint=${Date.now()}`));
}
// One changed asset: the page's element that shows it is refreshed by name.
async function refresh(asset) {
  const name = asset.name;
  const bust = `?v=${(asset.sha256 ?? "").slice(0, 12)}`;
  if (name.startsWith("shaders/") && name.endsWith(".wgsl")) {
    const r = await fetch(`/${name}${bust}`, { cache: "no-store" });
    if (!r.ok) throw new Error(`${name}: HTTP ${r.status}`);
    const text = await r.text();
    if (!globalThis.exact.gpu?.shader) return "shader (no GPU module on this page)";
    if (!globalThis.exact.gpu.shader(name.slice("shaders/".length, -".wgsl".length), text)) throw new Error(`${name}: the module refused it (the reason is on the console) — a bindings, layout, input, or output change needs a rebuild`);
    return "shader (swapped in)";
  }
  if (/\.(ttf|otf|woff2?)$/i.test(name)) {
    // A face's bytes changed: the app restarts from its current plan so the
    // faces re-register from the new bytes, state carried.
    const bytes = lastPlan ?? new Uint8Array(await (await fetch("/app.plan", { cache: "no-store" })).arrayBuffer());
    lastPlan = bytes;
    await globalThis.exact.reload(bytes);
    return "font";
  }
  if (name.startsWith("deck/")) {
    for (const frame of document.querySelectorAll("iframe")) {
      const src = frame.getAttribute("src") ?? "";
      if (src.startsWith("/deck/") || src.startsWith("deck/") || src.startsWith("./deck/")) frame.contentWindow?.location.reload();
    }
    return "deck";
  }
  // An image: every element showing it re-fetches by a busted URL, and the
  // refresh is done when the browser has decoded the new bytes — so the log
  // line says the pixels are on screen, not that a request was started.
  const loads = [];
  for (const img of document.querySelectorAll("img")) {
    const src = img.getAttribute("src") ?? "";
    if (src.split("?")[0].replace(/^\.?\//, "") !== name) continue;
    img.setAttribute("src", `${src.split("?")[0]}${bust}`);
    loads.push(img.decode().catch(() => { throw new Error(`${name}: the browser could not decode the new bytes`); }));
  }
  await Promise.all(loads);
  return loads.length ? `image (${loads.length})` : "unreferenced";
}
es.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.error) { show(m.error); console.error("exact dev:", m.error); return; }
  // A rebuilt wasm is a new program: the page reloads (no state carried).
  if (m.rebuilt) { location.reload(); return; }
  // A fresh page already has the plan baked into its wasm. Skip that one
  // hello when its digest agrees; a stale build, a missed edit, or an SSE
  // reconnect with a newer revision falls through and applies app.plan.
  if (m.hello && seen === 0 && m.digest && m.digest === m.baked) {
    seen = queued = m.seq;
    return;
  }
  // Hello is also a revision: applying it closes both startup races (a stale
  // baked plan and an edit between page fetch and SSE subscribe). On an SSE
  // reconnect, the already-seen revision is left alone, preserving state.
  if (!Number.isInteger(m.seq) || m.seq <= queued) return;
  queued = m.seq;
  const step = m.assets
    ? () => Promise.all(m.assets.filter((a) => !a.removed).map(refresh)).then((kinds) => { console.log("exact dev: assets", m.assets.map((a, i) => `${a.name} → ${kinds[i]}`).join(", ")); })
    : () => reload(m);
  reloads = reloads
    .then(step)
    .then(() => { seen = m.seq; })
    .catch((error) => {
      queued = seen;
      const message = String(error);
      show(message);
      console.error("exact dev:", message);
    });
};
