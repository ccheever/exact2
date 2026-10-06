// Driver fetch faults on the web (LLP 1103): the session's table, consulted
// where each web target would send a request — the wasm host's `request`
// (http-body.js) and the JS target's `fetchWith` (admission.js) count a
// match and fail it as a refused connection; `fetchEarly` (module-glue.js)
// only declines to start one early, so a request is counted once. The table
// is the page's, on `globalThis`, because the builds copy these modules; it
// is armed by the agent's `failFetch` launch parameter and its `faults`
// operation, and a production bake has neither.
const AGENT_ADMITTED = true; // false in a production bake (host/web/build.mjs, host/web-js/build.mjs)

/** A table from its launch lines (the runner's `Faults::parse`): `<prefix>`, `<prefix>\t<times>`, or a reload's `<prefix>\t<times>\t<left>\t<hits>\t<armed>`. */
export function parseFaults(spec) {
  const count = field => {
    if (field === undefined || field === '-') return null;
    if (!/^\d+$/.test(field)) throw new Error(`fail fetch: \`${field}\` is not a count`);
    return Number(field);
  };
  const entries = [];
  for (const line of String(spec).split('\n')) {
    if (!line.trim()) continue;
    const [prefix, times, left, hits, armed] = line.split('\t');
    if (!prefix) throw new Error('fail fetch: each line names a non-empty prefix');
    const n = count(times);
    if (n === 0) throw new Error('fail fetch: `times` is a positive integer');
    const at = entries.findIndex(f => f.prefix === prefix);
    if (at >= 0) entries.splice(at, 1);
    entries.push({ prefix, times: n, left: left === undefined ? n : count(left), hits: count(hits) ?? 0, armed: armed !== '0' });
  }
  return entries;
}

function launchTable() {
  if (!AGENT_ADMITTED) return [];
  const params = new URL(globalThis.performance?.getEntriesByType?.('navigation')[0]?.name ?? globalThis.location?.href ?? 'http://x/').searchParams;
  if (!params.has('agent')) return [];
  try { return parseFaults(params.get('failFetch') ?? ''); } catch { return []; }
}

const state = () => globalThis.__exactFaults ??= { entries: launchTable(), log: null, pending: [] };
const live = f => f.armed && f.left !== 0;
const longest = url => state().entries.filter(f => live(f) && String(url).startsWith(f.prefix)).sort((a, b) => b.prefix.length - a.prefix.length)[0];

/** Where an injected failure's journal line goes: each host's own journal; lines from before it was set follow. */
export function setFaultLog(log) { const s = state(); s.log = log; for (const line of s.pending.splice(0)) log(line); }
const say = line => { const s = state(), log = s.log ?? globalThis.__exactFaultLog; if (log) log(line); else s.pending.push(line); };

/** Whether a fetch of `url` fails, counting it (the longest live prefix decides); the journal says so. */
export function takeFault(url) {
  if (!state().entries.length) return false;
  const f = longest(url);
  if (!f) return false;
  f.hits += 1;
  if (f.left !== null) f.left -= 1;
  say(faultMessage(url));
  return true;
}

/** Whether a fetch of `url` would fail, without counting it: `fetchEarly` starts no GET a fault will fail. */
export const faultMatches = url => state().entries.length > 0 && longest(url) !== undefined;

export const faultMessage = url => `fetch failed (driver fault): ${url}`;

/** The table as `state.faults` shows it. */
export const faultsJson = () => state().entries.map(({ prefix, times, left, hits, armed }) => ({ prefix, times, left, hits, armed }));

/** The agent's `faults` operation: `{fail, times?}` arms, `{pass}` stops failing; the reply is the table. */
export function faultOp(req) {
  const { entries } = state();
  if (req.fail !== undefined) {
    if (typeof req.fail !== 'string' || !req.fail) return { error: 'fail fetch needs a URL prefix' };
    if (req.times !== undefined && !(Number.isInteger(req.times) && req.times >= 1 && req.times <= 0xffffffff)) return { error: 'fail fetch: `times` is a positive integer' };
    const at = entries.findIndex(f => f.prefix === req.fail);
    if (at >= 0) entries.splice(at, 1);
    const times = req.times ?? null;
    entries.push({ prefix: req.fail, times, left: times, hits: 0, armed: true });
  } else if (req.pass !== undefined) {
    const f = entries.find(f => f.prefix === req.pass);
    if (!f) return { error: `pass fetch "${req.pass}": no fault was armed for it` };
    f.armed = false;
  }
  return { faults: faultsJson() };
}

// The wasm host's glue loads this file after paint for the agent's `faults` (glue.js `loadAfterPaint`).
if (globalThis.exact) globalThis.exact.faults = { faultOp };
