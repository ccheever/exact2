// The dev loop's page side (host code, served only by dev.mjs, never in
// dist/): a plan ready on the server → fetch it → restart the app from it.
// A reload is a restart (LLP 1004 D5): the page reboots from initial state.
const es = new EventSource("/__dev");
let overlay = null;
function show(message) {
  if (!message) { overlay?.remove(); overlay = null; return; }
  overlay ??= document.body.appendChild(Object.assign(document.createElement("pre"), { style: "position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647" }));
  overlay.textContent = message;
}
es.onmessage = async (e) => {
  const m = JSON.parse(e.data);
  if (m.error) { show(m.error); console.error("exact dev:", m.error); return; }
  // A rebuilt wasm is a new program: the page reloads (no state carried).
  if (m.rebuilt) { location.reload(); return; }
  show(null);
  if (!globalThis.exact) return;
  const t = performance.now();
  const bytes = new Uint8Array(await (await fetch(`/app.plan?seq=${m.seq}`, { cache: "no-store" })).arrayBuffer());
  const fetchMs = performance.now() - t;
  const bootMs = await globalThis.exact.reload(bytes);
  // The new plan's first frame is in the DOM: report now (synchronously — a
  // headless page may never get an animation frame), then the paint.
  navigator.sendBeacon(`/__dev/reloaded?seq=${m.seq}&dom=${Date.now()}&fetch=${fetchMs.toFixed(1)}&boot=${bootMs.toFixed(1)}`);
  requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?seq=${m.seq}&paint=${Date.now()}`));
};
