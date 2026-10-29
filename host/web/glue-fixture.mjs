// The real glue.js run with host doubles, for the agent tests (agent.test.mjs): its clock, its replies
// and the public object, no browser.
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { focusController, placeReporter, timeReporter, launchLocation } from './navigation.js';
import { backButton, viewBox, layoutBoxes, scrollRange } from './input-glue.js';

// Like http-body/request-refusal.test.mjs, run the actual glue with host doubles.
// Include the real public object and nodeDetail so registration and flow facts
// are covered too. No browser geometry or scheduling is simulated as evidence.
export const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
// A one-line declaration is the whole line (with any state it declares first).
export const declaration = name => (source.match(new RegExp(`^(?:let [^;\\n]+; )?(?:async )?function ${name}\\(.*\\}$`, 'm'))
  ?? source.match(new RegExp(`(?:async )?function ${name}\\([^]*?\\n\\}`)))[0];
export const publicObject = source.match(/globalThis.exact = \{[^]*?\n\};/)[0];
export function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
export function fixture(agentMode = true) {
  const events = [], state = { slots: { backPresses: 3, navigatePresses: 4 } };
  const outline = { roots: [1], nodes: [{ id: 1, type: 'Text' }] };
  const logs = { next: 1, from: 0, lines: ['boot'] };
  const box = { x: 0, y: 0, width: 100, height: 50, left: 0, top: 0, right: 100, bottom: 50 };
  const el = { isConnected: true, localName: 'p', dataset: {}, clientWidth: 100, clientHeight: 50,
    // A real element's client rects: one box, as an unfragmented node has (glue.js reads them for column_fragments, LLP 1093 D12).
    getBoundingClientRect: () => box, getClientRects: () => [box], hasAttribute: () => false };
  const root = { dataset: {}, replaceChildren() { events.push('replace'); } };
  // glue.js: `agentKeepsStore = !agentMode || ?storage`; the fixture's drive names no store.
  const context = vm.createContext({ events, agentMode, agentKeepsStore: !agentMode, root, views: new Map([[1, el]]),
    state, outline, logs, textflow: null, flowLoading: null, flowContexts: [], flowDue: null, flowLanding: false, flowFrames: false, present() {}, lists: new Map(),
    Date: class extends Date { static now() { return 123; } }, performance: { now: () => 10 }, TextEncoder, Uint8Array,
    HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLIFrameElement: class {}, HTMLVideoElement: class {}, HTMLMediaElement: class {},
    foldBits: () => 0 /* no fold posture (LLP 1078 D6) */,
    document: { activeElement: null, body: {}, querySelector: () => null, getElementById: () => null, documentElement: { scrollWidth: 0, clientWidth: 0, scrollHeight: 0, clientHeight: 0 } }, // boot's last plan's theme: none; a page with nothing to scroll
    innerWidth: 300, innerHeight: 200, devicePixelRatio: 1, scrollX: 0, scrollY: 0,
    INHERITED_CSS: {}, getComputedStyle: () => ({}), inertAncestor: () => false,
    viewBox: element => element.getBoundingClientRect(),
    navigation: { observation: () => ({ location: '/' }), reset() {} },
    presence: { live: null, pending: () => null, landed: async () => true },
    ask: req => req.op === 'state' ? state : req.op === 'logs' ? logs : req.op === 'node' ? { id: req.id, type: 'Text' }
      : req.op === 'tags' ? { epoch: 2, incarnation: 1, clock: 0 } : { error: 'unknown op' },
    tree: () => outline, now: () => 0, environment: () => ({}),
    agentClock: 0, imageHold: null /* no animated image loaded (image-glue.js, LLP 1011.000) */, SETTLE_DEADLINE_MS: 20000, REPLACE_SETTLE_MS: 1000, GLIDE_BOUND_MS: 4000, glidesSettled: undefined, scrollsSeen: undefined /* the agent's input helpers, not loaded here (input-glue.js) */, inflight: new Set(), mediaAttaching: new Set(), forgettable: new Map(), waiting: () => [...context.inflight],
    heldAsks: new Map(), holds: ticket => !context.letGoTickets?.has(ticket), // requests the server holds (LLP 1016.000 D9), and whether the runner still holds a ticket
    // The page's own wait (http-body.js) races the deadline; here a request marked `stuck` is what a passed deadline finds.
    waitForInflight: async () => { const all = [...context.inflight]; if (all.some(p => p.stuck)) return false; await Promise.all(all); return true; },
    memory: { buffer: new ArrayBuffer(1024) }, readOut: value => value,
    requestsPending: 0,
    wasm: { exact_in: () => 0, exact_plan: () => 1, exact_out: () => 0,
      exact_requests_pending: () => context.requestsPending,
      exact_plan_fonts: () => '[]', exact_boot: () => '{"ops":[],"timers":true}',
      exact_boot_plan: () => '{"ops":[],"timers":true}', exact_advance: () => '{"ops":[],"clock":16}' },
    devAssets: null, bootAttempt: 0, encoder: new TextEncoder(), location: { pathname: '/', search: '', href: 'https://fixture.invalid/' }, URL,
    focus: focusController({ ready: () => true, elements: () => [], inert: () => false }), loadGpuIfNeeded() {}, afterPaint: fn => context.requestAnimationFrame(() => context.requestAnimationFrame(fn)), writeIn: value => value, send() {}, messageViews: new Set(), markupModule: null, decorationsModule: null, liveTimeModule: null,
    prepareFonts: async () => [], commitFonts() {}, releaseAssets() {}, appRootFontSize() {} /* no app root rule to drop (LLP 1069.000 D3) */, incarnation: 0,
    // Ordinary boot tests model an already-loaded post-paint scheduler.
    timerFactory: () => ({ update() { events.push('ticker'); }, dispose() {} }),
    ticker: null, motion: { reset() {} }, arrange: { reset() {} }, pieces: { pending: () => null }, retiredViews: new WeakSet(), followedScrolls: new Map(),
    pendingScrolls: new Set(), collections: { reset() {} }, messageFrames: new Map(),
    storageRequests: null, controllers: new Set(), grants: [],
    commitTurns: [],
    applyBatch: batch => { events.push('batch'); context.agentClock = batch.clock ?? 0;
      queueMicrotask(() => context.commitTurns.push([...events])); return { timers: batch.timers, batch }; },
    activateData: () => events.push('activate'), setInterval: () => { events.push('ticker'); return 1; },
    requestAnimationFrame: () => events.push('raf'), clearInterval() {},
    ready: Promise.resolve(), moduleReady: Promise.resolve(), inputReady: true, logicInfo: null, activeModule: null,
    page: null, // a built document's boot (LLP 1048.000 D6); these pages have none
    loadStage: () => Promise.resolve(), stageLoaded: () => true, // every stage linked (LLP 1047.000 §9)
    preferences: () => '{}', localAssetURL: source => source, pageFacts: {} /* `globalThis.exact` exposes it (LLP 1139.003 D2) */, sizes: new Set(), mediaAttached: (deadline, set = context.mediaAttaching) => Promise.all(set).then(() => true) /* glue.js's, unbounded */,
    launchLocation: () => launchLocation(context), // the launch path less the drive's facts (feed F16)
  });
  vm.runInContext(`let toldOffset = null; const folded = () => false; /* no folded text here (LLP 1007.001) */ const backButton = ${backButton}; const viewBox = ${viewBox}; const layoutBoxes = ${layoutBoxes}; const scrollRange = ${scrollRange};\n` + ['let gpuLoading', 'const POST_BOUND', 'let frameSampler', 'const followOffset', 'const shownValue'].map(head => source.match(new RegExp(`^${head} = .*$`, 'm'))[0]).join('\n') + '\n' + source.match(/const heldNamed = r => \{.*? : r; \};/)[0] + '\n' + ['nodeDetail', 'agentInputReady', 'agent', 'agentNow', 'agentReply', 'settleGpu', 'gpuPendingReply', 'agentSettled', 'tagged', 'clock', 'startClock', 'mutate', 'boot', 'bootNow'].map(declaration).join('\n') + '\n' + publicObject, context);
  context.reportPlace = placeReporter(new URLSearchParams(agentMode ? 'agent=1' : ''), context);
  context.reportTime = timeReporter(new URLSearchParams(agentMode ? 'agent=1' : ''), context);
  return context;
}

export function requestClock(f) {
  let wall = 0;
  f.performance = { now: () => wall };
  f.setTimeout = (fn, ms) => { queueMicrotask(() => { wall += ms; f.pollRequests?.(); fn(); }); return 1; };
  f.clearTimeout = () => {};
  f.gpuInPlay = () => false;
  f.settleCandidate = () => f.agentClock;
  Object.assign(f.collections, { settle() {}, fireOwedScrolls: async () => null, owedScrolls: () => false });
  vm.runInContext(declaration('waitForInflight'), f);
  return () => wall;
}

// A timer every 300 ms whose action sends a request: the runner keeps one
// request per target, so a jump that fired every due tick at once would
// drop each reply but the last (runner/src/runner/commit.rs `enqueue`).
// Advancing until a request, the runner stops after the tick that sent.
export function timedRequests(f, { stuck = false, parked = false } = {}) {
  let due = 300;
  f.flowDue = due; // the boot batch's `timer_due_ms`
  f.wasm.exact_advance = (to, untilRequest) => {
    let clock = to;
    while (due <= to) {
      const at = due;
      if (parked) f.requestsPending = 1;
      else {
        const p = stuck ? new Promise(() => {}) : new Promise(resolve => setTimeout(() => { f.events.push(`reply ${at}`); resolve(); }, 0));
        p.stuck = stuck; f.inflight.add(p); p.finally(() => f.inflight.delete(p));
      }
      due += 300;
      if (untilRequest) { clock = at; break; }
    }
    f.events.push(`advance ${to}${untilRequest ? ' until a request' : ''} → ${clock}`);
    return JSON.stringify({ ops: [], clock, timers: true, timer_due_ms: due });
  };
  f.applyBatch = batch => { f.agentClock = batch.clock; f.flowDue = batch.timer_due_ms; f.flowLanding = !!batch.landing; return { timers: true, batch }; };
}
