// The web host's glue: apply batches, forward events, tick the clock.
//
// @ref LLP 1007 §3. This is host code, not app code: it knows nothing about
// the app. The app is the wasm (runner + kernel + data crate + baked plan).
// Nothing here runs per frame; layout and motion are the browser's.

const root = document.getElementById("exact-root");
const views = new Map(); // view id -> element
let wasm = null;
let memory = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const t0 = performance.now();

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
      el.addEventListener("click", (e) => { e.stopPropagation(); send(wasm.exact_dispatch(id, 0, 0)); });
    } else if (kind === "change") {
      el.addEventListener("input", () => { const n = writeIn(el.value); send(wasm.exact_dispatch(id, 1, n)); });
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

async function main() {
  const url = new URL("./app.wasm", import.meta.url);
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
  wasm = instance.exports;
  memory = wasm.memory;
  const timers = send(wasm.exact_boot());
  // The first frame is in the DOM: stamp the time from script start, so a
  // headless run can read it. A second stamp lands when it is painted.
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  requestAnimationFrame(() => {
    root.dataset.paintMs = (performance.now() - t0).toFixed(1);
  });
  if (timers) {
    setInterval(() => send(wasm.exact_advance(performance.now() - t0)), 250);
  }
}

main().catch((e) => { console.error(e); root.dataset.error = String(e); });
