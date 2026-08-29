// The web host's glue: apply batches, forward events, tick the clock.
//
// @ref LLP 1007 §3. This is host code, not app code: it knows nothing about
// the app. The app is the wasm (runner + kernel + data crate + baked plan).
// Nothing here runs per frame; layout and motion are the browser's.

const root = document.getElementById("exact-root");
const views = new Map(); // view id -> element
const animations = new Map(); // "view/property" -> Animation (a spring in flight)
let wasm = null;
let memory = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const t0 = performance.now();
const now = () => performance.now() - t0;

function readOut(len) {
  const ptr = wasm.exact_out();
  return decoder.decode(new Uint8Array(memory.buffer, ptr, len));
}

function writeIn(text) {
  const bytes = encoder.encode(text);
  const ptr = wasm.exact_in(bytes.length);
  new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
  return bytes.length;
}

function applyProps(el, set, clear) {
  for (const name of clear || []) {
    if (name === "text") el.textContent = "";
    else if (name === "value") el.value = "";
    else if (name === "checked") el.checked = false;
    else el.removeAttribute(name);
  }
  for (const [name, value] of Object.entries(set || {})) {
    if (name === "text") {
      if (el.childElementCount === 0) el.textContent = value;
    } else if (name === "value") {
      if (el.value !== value) el.value = value;
    } else if (name === "checked") {
      el.checked = value === "true";
    } else if (name === "disabled") {
      if (value === "true") el.setAttribute("disabled", ""); else el.removeAttribute("disabled");
    } else {
      el.setAttribute(name, value);
    }
  }
}

function attach(el, id, handlers) {
  el.dataset.view = String(id);
  for (const kind of handlers) {
    if (kind === "press") {
      el.addEventListener("click", (e) => { e.stopPropagation(); send(wasm.exact_dispatch(id, 0, 0, now())); });
    } else if (kind === "change") {
      el.addEventListener("input", () => { const n = writeIn(el.value); send(wasm.exact_dispatch(id, 1, n, now())); });
    }
  }
}

function apply(batch) {
  if (batch.error) console.error("exact:", batch.error);
  for (const op of batch.ops) {
    switch (op.op) {
      case "create": {
        const el = document.createElement(op.tag);
        applyProps(el, op.props, []);
        el.style.cssText = op.css;
        attach(el, op.id, op.handlers);
        views.set(op.id, el);
        break;
      }
      case "props": applyProps(views.get(op.id), op.set, op.clear); break;
      case "style": views.get(op.id).style.cssText = op.css; break;
      case "children": {
        const el = views.get(op.id);
        const want = op.ids.map((i) => views.get(i)).filter(Boolean);
        // Reorder in place: keyed rows keep their elements (and their state).
        let cursor = el.firstElementChild;
        for (const child of want) {
          if (child === cursor) { cursor = cursor.nextElementSibling; continue; }
          el.insertBefore(child, cursor);
        }
        while (cursor) { const next = cursor.nextElementSibling; cursor.remove(); cursor = next; }
        break;
      }
      case "animate": {
        // A spring: frames from the engine, played by the browser with linear
        // interpolation (LLP 1002 D2). Replaces the spring on that property.
        const key = op.id + "/" + op.property;
        animations.get(key)?.cancel();
        animations.delete(key);
        if (!op.values.length) break;
        const css = (v) => op.property === "translate" ? `${v[0]}px ${v[1]}px` : op.property === "rotate" ? `${v}deg` : String(v);
        const anim = views.get(op.id).animate(op.values.map((v) => ({ [op.property]: css(v) })), { delay: op.delay, duration: op.duration, easing: "linear" });
        animations.set(key, anim);
        anim.finished.then(() => { if (animations.get(key) === anim) animations.delete(key); }, () => {});
        break;
      }
      case "destroy": { const el = views.get(op.id); if (el) el.remove(); views.delete(op.id); break; }
      case "roots": {
        root.replaceChildren(...op.ids.map((i) => views.get(i)).filter(Boolean));
        break;
      }
    }
  }
  return batch.timers;
}

function send(len) {
  return apply(JSON.parse(readOut(len)));
}

let ticker = null;

// Boot the app — from the plan baked into the wasm, or from `bytes` (the
// dev loop's restart, LLP 1004 D5: a reload is a restart from initial
// state). Returns the milliseconds from call to first frame in the DOM.
function boot(bytes) {
  const t = performance.now();
  if (ticker) clearInterval(ticker);
  ticker = null;
  for (const a of animations.values()) a.cancel();
  animations.clear();
  views.clear();
  root.replaceChildren();
  let timers;
  if (bytes) {
    const ptr = wasm.exact_in(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    timers = send(wasm.exact_boot_plan(bytes.length));
  } else {
    timers = send(wasm.exact_boot());
  }
  if (timers) ticker = setInterval(() => send(wasm.exact_advance(now())), 250);
  return performance.now() - t;
}

globalThis.exact = { reload: (bytes) => (wasm ? boot(bytes) : NaN) };

async function main() {
  const url = new URL("./app.wasm", import.meta.url);
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
  wasm = instance.exports;
  memory = wasm.memory;
  boot(null);
  // The first frame is in the DOM: stamp the time from script start, so a
  // headless run can read it. A second stamp lands when it is painted.
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  requestAnimationFrame(() => {
    root.dataset.paintMs = (performance.now() - t0).toFixed(1);
  });
}

main().catch((e) => { console.error(e); root.dataset.error = String(e); });
