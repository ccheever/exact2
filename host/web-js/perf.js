// `perf` on the JS target (LLP 1079 D1–D2): each plan site's work in a
// development build, in the runner's units where the page has them. The
// emitter wraps each dynamic binding of a development build in `pf(site, f)`
// (`evaluated`, `unchanged`); the document's own mutation records give
// `authored`, `created` and `retired`. Counting starts when the page installs
// it, after boot, as the runner's does when its host says to measure. `seq`
// is the committed epoch: a refused action is no transaction. A production
// build references none of it. `inherited` and `moved` are absent: the
// browser cascades and lays out.
import { eq, owner, After, Before, journal, clock, data, Resources, Mutations } from "./rt.js";
import { sourceTypes } from "./names.js";
import { watch } from "./seam.js";

const Sites = [];
const at = site => Sites[site] ??= { created: 0, retired: 0, evaluated: 0, unchanged: 0, authored: 0 };

/** A binding's expression, counted. One expression can feed several
 * writes (a shorthand's longhands, a paint fact): each effect that runs it
 * is an evaluation, unchanged when it equals what that effect last got. */
export function pf(site, f) {
  // [effect, its last value, …]: one to three effects run a binding.
  const last = [];
  return () => {
    const v = f(), o = owner(), i = last.indexOf(o);
    if (Observer) { const w = at(site); w.evaluated++; if (i >= 0 && eq(last[i + 1], v)) w.unchanged++; }
    if (i >= 0) last[i + 1] = v; else if (o) last.push(o, v);
    return v;
  };
}

const siteOf = el => el?.dataset?.site != null ? Number(el.dataset.site) : null;
// Every site element under (and at) `el`.
const sited = el => el.nodeType !== 1 ? [] : [...(el.matches('[data-site]') ? [el] : []), ...el.querySelectorAll('[data-site]')];
// The site a site was first realized under: `perf <target>` reaches a site
// whose instances have all gone through it (the runner reads the plan).
const Up = new Map();
const Live = new WeakSet();
let Observer = null;

function created(el) {
  if (Live.has(el)) return;
  Live.add(el);
  const s = siteOf(el);
  at(s).created++;
  if (!Up.has(s)) Up.set(s, siteOf(el.parentElement?.closest('[data-site]')));
}

/** Mutation records. `commit`: they are a commit's (taken at its `After`),
 * so the elements they name were authored; records made between commits (an
 * exit animation's end, the motion piece's frames), delivered to the
 * observer or taken at a `Before`, count only what was built and dropped. */
function count(records, commit) {
  if (!records.length) return;
  const authored = new Set(), removed = new Set(), added = new Set();
  for (const r of records) {
    if (r.type === 'childList') {
      for (const n of r.removedNodes) removed.add(n);
      for (const n of r.addedNodes) added.add(n);
    }
    const el = r.target.nodeType === 1 ? r.target : r.target.parentElement;
    const own = el?.closest('[data-site]');
    if (commit && own?.isConnected) authored.add(own);
  }
  const retire = el => { Live.delete(el); at(siteOf(el)).retired++; };
  for (const n of removed) if (!n.isConnected) for (const el of sited(n)) if (Live.has(el)) retire(el);
  const fresh = new Set();
  for (const n of added) for (const el of sited(n)) {
    // A child added under an added parent is reached through both.
    if (Live.has(el) || fresh.has(el)) continue;
    fresh.add(el);
    created(el);
    // Built and dropped between two takes: churn the runner counts too.
    if (!n.isConnected) retire(el);
  }
  // An element built in this commit is created, not authored (a receipt's `touched` excludes `created`).
  for (const el of authored) if (!fresh.has(el)) at(siteOf(el)).authored++;
}
const take = commit => { if (Observer) count(Observer.takeRecords(), commit); };

/** Start counting the document's structure (agent.js, once): elements
 * already there count as created now, as the runner's do when it is told to measure. */
export function install(root) {
  if (Observer) return;
  for (const el of sited(root)) created(el);
  Observer = new MutationObserver(records => count(records, false));
  Observer.observe(root, { subtree: true, childList: true, attributes: true, characterData: true });
  Before.push(() => take(false));
  After.push(() => take(true));
}

/** The reply to `{"op":"perf"[,"target":…]}`, for the element `target`
 * (the page's root without one): the runner's shape (perf.rs). */
export function reply(target, tags) {
  take(false);
  const WALK = 20000, SITES = 500, live = new Map();
  let walked = 0, truncated = false;
  for (const el of sited(target)) {
    if (walked === WALK) { truncated = true; break; }
    walked++;
    const s = siteOf(el);
    live.set(s, (live.get(s) ?? 0) + 1);
  }
  // A site whose instances have all gone still has totals: under the
  // target's site (by the site it was first realized under), or anywhere
  // for a read of the whole page.
  const top = siteOf(target), whole = target.id === 'exact-root';
  for (const s of Sites.keys()) {
    if (!Sites[s] || live.has(s)) continue;
    if (whole) { live.set(s, 0); continue; }
    if (top != null) for (let u = s; u != null; u = Up.get(u)) if (u === top) { live.set(s, 0); break; }
  }
  const all = [...live.keys()].sort((a, b) => a - b);
  if (all.length > SITES) { truncated = true; all.length = SITES; }
  const sites = all.map(site => {
    const w = at(site);
    // A hatched site's row names its hatch's calls and time (LLP 1075.003.000.001 §3.3).
    return { site, instances: live.get(site), live: w.created - w.retired, ...w, ...(globalThis.exact?.hatchPerf?.site(site) ?? {}) };
  });
  return { ...tags, seq: clock.epoch, plan: globalThis.exact?.plan ?? null, sites, walked, truncated };
}

/** A development page's data answers, watched from before the app starts
 * (main.js): a big one crossing the seam is said (seam.js). A reply is parsed
 * for the resource or mutation holding its ticket. */
export function seam() {
  const holder = args => (Resources.find(r => r.ticket?.args === args) ?? Mutations.find(m => m.ticket?.args === args))?.name;
  watch(data, { types: sourceTypes, say: line => journal.push(`t=${clock.now} ${line}`), owner: holder });
}

/** A development page (main.js, outside the agent too): its structure is
 * counted, and outside the agent its frames are sampled (D3) and ⌥⇧T posts a
 * trace (D5). Under the agent's clock no frame is presented: `perf frames`
 * is virtual (agent.js). */
export async function develop(exact) {
  const root = document.getElementById('exact-root');
  install(root);
  if (clock.agent) return;
  const tags = () => ({ clock: clock.now, epoch: clock.epoch, incarnation: 1 });
  let began = null;
  // Loaded, not imported: the sampler announces itself on `exact` for the wasm glue, and an import would keep that in every bundle.
  const { createFrameSampler } = await import("./frames.js");
  const sampler = createFrameSampler({
    origin: () => clock.start ?? 0, target: 'js', covers: ['input', 'scroll', 'commits', 'animations', 'canvas'],
    log: line => journal.push(`t=${clock.now} ${line}`),
    // With the hatches (LLP 1075.003.000.001 §3.3), read in this same turn, so the sections agree.
    gather: async () => ({ journal: { from: journal.start, next: journal.start + journal.length, lines: journal.slice() }, perf: reply(root, tags()),
      ...(exact.hatchState ? { hatches: { state: exact.hatchState(), perf: exact.hatchPerf.reply(tags()) } } : {}) }),
  });
  // A commit's transactions are known once it returns: the epoch moves after its `After`.
  let mark = clock.epoch;
  Before.push(() => { began = performance.now(); });
  After.push(() => {
    const ms = began == null ? 0 : performance.now() - began;
    began = null;
    queueMicrotask(() => { const e = clock.epoch; sampler.batch(e > mark ? [mark + 1, e] : null, ms); mark = e; });
  });
  exact.frames = sampler;
}
