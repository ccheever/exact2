// The native-module fixture (LLP 1024 D8) on the web: the module table's
// names as exports (host/web/native-glue.js). The host defines each roster
// tag and hands this module the element; the fixture renders into its shadow
// root. `exact-fixture` echoes each props object it accepts as a `message`,
// fires all nine events when `emit` changes, refuses `reject=true`, and calls
// back after `destroy` (the host must drop it). Neither box takes pointer
// events of its own, so an agent tap lands on the element — the node.
export const abi = 1;
export const roster = { 'exact-fixture': { snapshot: true }, 'exact-plain': { snapshot: false }, 'exact-screen': { snapshot: false } };

const size = (h, props) => {
  if (h.tag === 'exact-plain') {
    h.box.style.width = props.natural === 'false' ? '0px' : '120px';
    h.box.style.height = props.natural === 'false' ? '0px' : props.expanded === 'true' ? '64px' : '32px';
  }
};

const echo = (props) => 'props:' + JSON.stringify(Object.fromEntries(Object.entries(props).sort(([a], [b]) => (a < b ? -1 : 1))));

export function create(tag, element, json, event) {
  const props = JSON.parse(json);
  if (tag === 'exact-fixture' && props.reject === 'true') throw new Error('reject=true');
  const root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
  const box = document.createElement('div');
  box.style.cssText = 'width:100%;height:100%;pointer-events:none';
  root.replaceChildren(box);
  const h = { tag, box, event, emit: props.emit ?? '0' };
  size(h, props);
  box.style.backgroundColor = props.tint ?? 'gray';
  // The web's form of a native screen (LLP 1075.003 §3.6): its own element.
  if (tag === 'exact-screen') { box.textContent = 'A native screen'; box.style.cssText += ';display:grid;place-items:center;background:#f2f2f7'; }
  if (tag === 'exact-fixture') { event(8, echo(props)); event(7); }
  return h;
}

export function setProps(h, json) {
  const props = JSON.parse(json);
  if (h.tag === 'exact-fixture' && props.reject === 'true') throw new Error('reject=true');
  size(h, props);
  h.box.style.backgroundColor = props.tint ?? 'gray';
  if (h.tag !== 'exact-fixture') return;
  h.event(8, echo(props));
  const next = props.emit ?? '0';
  if (next === h.emit) return;
  h.emit = next;
  // Every event, later and in order, as a background source would.
  if (Number(next) > 0) setTimeout(() => {
    h.event(0); h.event(1, 'changed'); h.event(2, true); h.event(3); h.event(4);
    h.event(5, 'Enter'); h.event(6); h.event(7); h.event(8, 'hello');
  });
}

export function snapshot() {}

export function destroy(h) {
  if (h.tag === 'exact-fixture') setTimeout(() => h.event(8, 'late'), 50);
}

// The hatches on the web (LLP 1075.003.000): the badge takes a context menu of
// its own, an interaction Exact leaves to the app; each row's dot hatch does
// nothing; every call is counted where the smoke reads it. Each also says what
// it did through its diagnostics (LLP 1075.003.000.001 §3.2): a counter per
// moment, and for the badge a line, a span from its mount to its end and a
// snapshot of its last tone.
const calls = (globalThis.exactFixtureHatches ??= {});
const count = (e, moment) => { const key = `${e.hatch}:${moment}`; calls[key] = (calls[key] ?? 0) + 1; };
const shown = new WeakMap();

export function element(e) {
  const moment = e.isNew ? 'built' : 'changed';
  count(e, moment);
  e.diagnostics.count(moment);
  // What a hatch asks of an authored node (§2.5), each queued: `feed` replaces
  // its field's value, `presser` clicks its own button once when armed and on
  // every change while looping.
  if (e.hatch === 'feed' && !e.isNew && e.data.feed) e.input(e.data.feed);
  if (e.hatch === 'presser' && !e.isNew && (e.data.loop !== 'off' || e.data.armed === 'true')) e.click();
  // The frame clock (§2.4): a ticket while the node says to run.
  if (e.hatch === 'clock') {
    const run = e.data.run === 'true';
    if (run && !clockStop) startClock(e);
    if (!run && clockStop) { clockStop(); clockStop = null; }
    if (!e.isNew && e.data.presses === '65') e.click();
  }
  if (e.hatch !== 'badge') return;
  e.diagnostics.log(`${moment}, tone ${e.data.tone}`);
  e.diagnostics.publish('tone', { tone: e.data.tone, moment });
  if (!e.isNew) return;
  shown.set(e, e.diagnostics.begin('shown'));
  e.element.addEventListener('contextmenu', (event) => event.preventDefault());
}

// The frame clock (LLP 1075.003.000.001 §2.4), as the Swift module's: each
// tick counts itself and whether the node's `data-frames`, which a frame task
// advances, is the tick's own number; the first chains `after`s; the third
// presses the node 65 times.
let clockStop = null;
function startClock(e) {
  const { frames, after, diagnostics: d } = globalThis.exact.hatches;
  let ticks = 0, agreed = 0;
  clockStop = frames((frame) => {
    ticks++;
    if (e.data.frames === String(ticks)) agreed++;
    d.count('clock.ticks');
    if (ticks === 1) {
      after(0, () => { d.count('clock.after0'); after(20, () => d.count('clock.after20')); });
      after(30, () => d.count('clock.stopped'))();
    }
    if (ticks === 3) for (let i = 0; i < 65; i++) e.click();
    d.publish('clock', { ticks, agreed, now: frame.now });
  });
}

export function elementEnded(e) {
  count(e, 'ended');
  e.diagnostics.count('ended');
  shown.get(e)?.end();
}

// The container hatches on the web: each counted, as the element hatches are.
const tally = (key) => { calls[key] = (calls[key] ?? 0) + 1; };
export function navigation() { tally('navigation:built'); globalThis.exact.diagnostics.count('navigations'); }
export function route() { tally('route:built'); }
export function routeEnded() { tally('route:ended'); }
export function tabs() { tally('tabs:built'); }

// The app and window scopes (LLP 1075.003.000.001 §2.1): each moment is
// counted and published for the smoke, as the Swift module does.
let scheme = 'light', own = null;
const publishScopes = (moment) => {
  globalThis.exact.diagnostics.count(`scope.${moment}`);
  globalThis.exact.diagnostics.publish('scopes', { scheme, exclusive: own?.exclusive ?? false, hasWindow: !!own?.window, frame: own ? own.frame.slice(2) : [], last: moment });
};
export function app(a) {
  scheme = a.prefersColorScheme;
  globalThis.exact.diagnostics.publish('app', { processOwner: a.processOwner, hasApplication: true, visibilityState: a.visibilityState, onLine: a.onLine });
  publishScopes(a.isNew ? 'app-built' : 'app-changed');
}
export function appEnded() { publishScopes('app-ended'); }
export function window(w) { own = w; publishScopes(w.isNew ? 'window-built' : 'window-changed'); }
export function windowEnded() { publishScopes('window-ended'); }
