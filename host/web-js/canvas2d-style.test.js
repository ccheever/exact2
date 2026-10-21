// LLP 1056 D4 on the JS target: a canvas draws in its node's colour and direction (`currentColor`, `direction =
// "inherit"`), so a change that changes them draws it again, cause "style", as the runner's canvases do: a commit's
// bound colour, its `setScheme` (a host command, applied after the commit's settle), the modes it resolves, the
// window's scheme, a development page's held scheme (the root's own style), a mode switch outside a commit and a
// colour transition's end. A change that leaves them as they were draws nothing, and a commit that changes a canvas's
// arguments and its colour draws it once, with both causes (D4: causes coalesce).
import { test, expect, mock } from "bun:test";

const draws = [], after = [];
mock.module("./canvas2d-glue.js", () => ({}));
mock.module("./draw.js", () => ({ drawer: { draw: (q) => { draws.push(q); return { lists: [], frame: false }; }, retire() {} } }));

test("whatever changes a canvas's colour draws it again, cause style", async () => {
  // The page's globals, stood in for the test and put back after it.
  const kept = ["getComputedStyle", "matchMedia", "MutationObserver", "document", "exact"].map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  try { await run(); } finally { for (const [name, d] of kept) d ? Object.defineProperty(globalThis, name, d) : delete globalThis[name]; }
});

async function run() {
  let ink = "rgb(52, 199, 89)", scheme = null;
  const watched = [];
  globalThis.getComputedStyle = () => ({ color: ink, direction: "ltr" });
  globalThis.matchMedia = () => ({ addEventListener: (type, f) => { scheme = f; } });
  globalThis.MutationObserver = class { constructor(f) { this.f = f; } observe(target, options) { watched.push([this.f, options]); } };
  // An attribute written on the root (`below`: on an element inside it), as the page's observers would see it.
  const write = (name, below = false) => { for (const [f, o] of watched) if (o.attributes && o.attributeFilter.includes(name) && (o.subtree || !below)) f(); };
  // A commit, as rt.js runs one: its tree update hands a surface its new arguments (`args`, rt.js `c2`), its settle runs
  // what follows it, then its host commands apply (`command`), then its microtasks run.
  let canvases, c;
  const commit = async (command = () => {}, args) => {
    if (args) { c.args = args; Promise.resolve(canvases).then((m) => m.args(c)); }
    for (const f of [...after]) f(); command(); await new Promise((done) => setTimeout(done));
  };
  let ended = null;
  globalThis.document = { documentElement: {}, addEventListener: (type, f) => { if (type === 'transitionend') ended = f; } };
  globalThis.exact = { canvas2dGlue: () => ({ op() {} }) };
  const { engine } = await import("./canvas2d.js");
  const el = { isConnected: true, dataset: {} };
  canvases = engine({ data: { logic: null, ready() {} }, clock: { now: 0 }, inflight: { n: 0 }, journal: [], After: after, views: new Map(), viewId: () => 7 });
  await Promise.resolve();
  c = { e: el, name: "spark", args: [], types: [], names: [], mounted: 0 };
  canvases.args(c);
  globalThis.exact.wasm.exact_canvas_geometry(7, 10, 10, 1);
  const seen = () => draws.map((q) => [q.frame[2], q.color[1]]);
  expect(seen()).toEqual([[1, 199]]);
  // A commit binds an ancestor's colour.
  ink = "rgb(0, 100, 0)";
  await commit();
  // The app's `setScheme`, which writes the root's `color-scheme` after the commit's settle.
  await commit(() => { ink = "rgb(0, 110, 0)"; });
  // The modes a commit resolves, in a microtask modes.js queued from its own `After` (a nested boundary's colour row).
  after.unshift(() => queueMicrotask(() => { ink = "rgb(0, 120, 0)"; }));
  await commit();
  after.shift();
  // The window's scheme.
  ink = "rgb(48, 209, 88)";
  scheme();
  // A development page's held scheme, on the root's own style.
  ink = "rgb(0, 130, 0)";
  write("style");
  // A mode switch outside a commit, which writes its tuple on the root and its boundaries.
  ink = "rgb(0, 150, 0)";
  write("data-x-modes", true);
  // Changes that leave the canvas's colour as it was, and a style written below the root, which is a commit's.
  write("data-x-modes", true);
  write("style");
  await commit();
  // A colour transition on an ancestor: its start is the old colour, its end the new.
  ink = "rgb(0, 160, 0)";
  ended({ propertyName: "background-color" });
  ended({ propertyName: "color" });
  // A commit that changes the canvas's arguments and its scheme.
  await commit(() => { ink = "rgb(0, 170, 0)"; }, [1]);
  expect(seen()).toEqual([[1, 199], [64, 100], [64, 110], [64, 120], [64, 209], [64, 130], [64, 150], [64, 160], [66, 170]]);
}
