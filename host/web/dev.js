// The dev loop's page side (host code, served only by dev.mjs, never in
// dist/): a plan ready on the server → fetch it → restart the app from it.
// A reload is a restart (LLP 1004 D5): the page reboots from initial state.
const es = new EventSource("/__dev");
let overlay = null;
let seen = 0;
let queued = 0;
let reloads = Promise.resolve();
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
  const fetchMs = performance.now() - t;
  const bootMs = await globalThis.exact.reload(bytes);
  // The new plan's first frame is in the DOM: report now (synchronously — a
  // headless page may never get an animation frame), then the paint.
  navigator.sendBeacon(`/__dev/reloaded?seq=${m.seq}&dom=${Date.now()}&fetch=${fetchMs.toFixed(1)}&boot=${bootMs.toFixed(1)}`);
  requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?seq=${m.seq}&paint=${Date.now()}`));
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
  reloads = reloads
    .then(() => reload(m))
    .then(() => { seen = m.seq; })
    .catch((error) => {
      queued = seen;
      const message = String(error);
      show(message);
      console.error("exact dev:", message);
    });
};
