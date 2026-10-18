// Authored tests (LLP 1017 P7), the driver's half: `contract test <file>`
// turns a file's `test` blocks into steps, and this drives them through the
// session the operations use (`agent.mjs`'s `open`). `agent.mjs` re-exports it.
import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { connect } from 'node:net';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { faultRequest as rawFaultRequest } from './agent-prefer.mjs';
import { open } from './agent.mjs';
import { duringOp } from './agent-drag.mjs';
import { driveStore, faultSpecOf, launchFacts } from './agent-launch.mjs';
import { resolveApp, webDist as defaultWebDist } from './app.mjs';
import { serverUrl } from './verify.mjs';
import { backendOriginSelection } from '../js/bake/src/typescript.mjs';
import { devicePreflight, mountsSnapback } from '../snapback4/web/device-build.mjs';

/** The text `expect text` reads (kanban F19, shop F15): the node's own `text`, else a checkbox's `checked` as `true` or `false`, else a control's value (a select's options
 * are its choices, not its text; LLP 1087 wizard trials), else its descendants' in order — the web's `textContent`, a button's
 * label — else a field's value. `nodes` is a `tree` reply's, in preorder. */
export function textOf(nodes, node, live = false) {
  if (node.props.text != null) return node.props.text;
  const at = nodes.indexOf(node), runs = [];
  // A checkbox or switch (only those take `checked`): `true` or `false`.
  if (node.type === 'Control' && node.props.checked != null) return String(node.props.checked);
  if (node.type === 'Control' && node.props.value != null) return node.props.value;
  // `live`: only what an active screen shows (a covered descendant's text is not this node's name).
  for (let i = at + 1; i < nodes.length && nodes[i].depth > node.depth; i++) if (nodes[i].props.text != null && !(live && nodes[i].inactive)) runs.push(nodes[i].props.text);
  return runs.length ? runs.join('') : node.props.value;
}

/** For a target no view carries: the tab panels never opened, whose contents do not exist until their tab is first
 * selected (LLP 1075.003 §3.7), each with the step that selects it. Nothing is opened for the drive. */
export function unopenedHint(nodes) {
  const panels = nodes.filter((n) => n.unopened);
  if (!panels.length) return '';
  const named = panels.map((p) => {
    const tab = nodes.find((n) => n.props.accessibilityRole === 'tab' && p.props.id != null && n.props.accessibilityControls === p.props.id);
    return `${p.props.testId ?? p.props.id ?? `#${p.id}`}${tab ? ` (\`tap ${tab.props.testId ?? tab.id}\`)` : ''}`;
  });
  return `; not yet opened: ${named.join(', ')}, a tab whose views exist once it is first selected`;
}

/** The node a test's target names in a tree: a covered screen's copy only when no active one carries it (shop F16). */
const targetNode = (nodes, target) => { const matches = nodes.filter((n) => n.props.testId === target); return matches.find((m) => !m.inactive) ?? matches[0]; };

/** `expect text <target> == <value>` over a `tree` reply's nodes: null when it holds, else `{ message }` — with
 * `unshown` when the tree's text is right but the shipped app does not show the node: a header node the iOS
 * navigation bar leaves out, which the agent's own chrome still paints (LLP 1116 D2). No clock can change that. */
export function textExpectation(nodes, target, value) {
  const n = targetNode(nodes, target), got = n ? textOf(nodes, n) : undefined;
  const runs = [];
  if (n && n.props.text == null && got !== n.props.value) for (let i = nodes.indexOf(n) + 1; i < nodes.length && nodes[i].depth > n.depth; i++) if (nodes[i].props.text != null) runs.push(nodes[i].props.text);
  const parts = runs.length > 1 ? ` (joined from its descendants: ${runs.map((r) => JSON.stringify(r)).join(' + ')}; to assert one, give that text its own testId)` : '';
  if (got !== value) return { message: `text of "${target}" is ${n ? JSON.stringify(got) : `absent (no view carries that testId${unopenedHint(nodes)})`}, expected ${JSON.stringify(value)}${parts}` };
  return n.unshown ? { message: `text of "${target}" is ${JSON.stringify(got)}, but ${n.unshown}`, unshown: true } : null;
}

/** `expect tree <present|missing> <target>`, as `textExpectation`: a node present in the tree that the shipped app does not show is not present. */
export function treeExpectation(nodes, target, present) {
  const n = targetNode(nodes, target);
  if (!!n !== present) return { message: `expected testId "${target}" ${present ? 'present' : 'absent'}, it was ${n ? 'present' : `absent${unopenedHint(nodes)}`}` };
  return present && n.unshown ? { message: `testId "${target}" is in the tree, but ${n.unshown}`, unshown: true } : null;
}

/** Whether a tree node is a disabled control: every host's tree reports the prop as `true`. `aria-disabled`
 * (`accessibilityDisabled`) reads as disabled for an expectation, but its press still dispatches, so only
 * a truly disabled control has its press refused (`refuseDisabled`). */
const propTrue = (n, key) => n?.props?.[key] === true || n?.props?.[key] === 'true';
const isDisabled = (n) => propTrue(n, 'disabled');
const readsDisabled = (n) => isDisabled(n) || propTrue(n, 'accessibilityDisabled');

/** `expect disabled|enabled <target>` (LLP 1116.010 D3): null when the control the target names is as asked, else `{ message }`. */
export function disabledExpectation(nodes, target, disabled) {
  const n = targetNode(nodes, target), word = disabled ? 'disabled' : 'enabled';
  if (!n) return { message: `expected "${target}" ${word}, no view carries that testId` };
  return readsDisabled(n) === disabled ? null : { message: `expected "${target}" ${word}, it was ${disabled ? 'enabled' : 'disabled'}` };
}

/** A press on a disabled control does nothing, and each host said so its own way (the web's silently, a native one in the words of the view
 * that took the touch), so a step that presses one fails the same on every host (LLP 1116.010 D3): the test of a refusal is `expect disabled`.
 * Context requests ignore disabled on containers and links, as HTML does; other non-press forms below bypass this guard. */
export async function refuseDisabled(s, node, target, opts = {}) {
  if (node.entity !== undefined || ['hover', 'wheel', 'pinch', 'drag', 'drop', 'into', 'history', 'mediaSession', 'gesture'].some((k) => opts[k] !== undefined && opts[k] !== false)) return;
  const read = await s.op({ op: 'tree', target: node.id, shallow: true }).catch(() => null);
  const current = read?.nodes?.find((n) => n.id === node.id);
  const control = ['Pressable', 'TextInput', 'Control'].includes(current?.type) && !(current.type === 'Pressable' && typeof current.props?.href === 'string' && current.props.href.length > 0);
  if (opts.contextmenu === true && current?.type && !control) return;
  if (isDisabled(current)) throw new Error(`${target} is disabled: a press does nothing (\`expect disabled ${JSON.stringify(String(target))}\` asserts it)`);
}

// What a person activates, most direct first: a control, or a view a press or an edit acts on (2); one taking a gesture
// or focus (1); none (0) — a `scroll` or `pointermove` handler only watches.
const PRESSES = new Set(['press', 'change', 'input', 'submit', 'select', 'dblclick']);
const GESTURES = new Set(['contextmenu', 'focus', 'swiperight', 'pan', 'drop']);
const tier = (n) => n.type === 'Control' || n.type === 'TextInput' || (n.handlers ?? []).some((h) => PRESSES.has(h)) ? 2 : (n.handlers ?? []).some((h) => GESTURES.has(h)) ? 1 : 0;
const interactive = (n) => tier(n) > 0;
const name = (n) => n.props.testId ?? (n.props.accessibilityLabel ? `"${n.props.accessibilityLabel}"` : null);

/** A target no testId carries, by the name a person reads (`nodes` a whole `tree` reply's, in preorder): the node whose
 * accessibilityLabel is exactly `target`, else whose text (`textOf`) is: on the active screens first (their text without a
 * covered descendant's), then anywhere. The most directly interactive win; of nested ones the outermost interactive,
 * or the innermost otherwise. More than one left
 * refuses, naming them; none returns null. */
export function nodeNamed(nodes, target) {
  let ends = null; // each node's preorder index and the end of its subtree, computed once and only on a match
  const spans = () => {
    ends = new Map();
    const open = [];
    nodes.forEach((n, i) => {
      while (open.length && open.at(-1).n.depth >= n.depth) { const o = open.pop(); ends.set(o.n, [o.i, i]); }
      open.push({ n, i });
    });
    for (const o of open) ends.set(o.n, [o.i, nodes.length]);
  };
  const passes = [true, false].flatMap((live) => [(n) => n.props.accessibilityLabel, (n) => textOf(nodes, n, live)].map((read) => [live, read]));
  for (const [live, read] of passes) {
    let found = nodes.filter((n) => { if (live && n.inactive) return false; const v = read(n); return v != null && String(v).trim() === target; });
    if (!found.length) continue;
    const top = found.reduce((t, n) => Math.max(t, tier(n)), 0);
    found = found.filter((n) => tier(n) === top);
    if (found.length > 1) {
      if (!ends) spans();
      const kept = [];
      for (const n of found) { // preorder: an ancestor comes before what it holds
        const inside = (m) => { const [i, end] = ends.get(m), [j] = ends.get(n); return j > i && j < end; };
        if (interactive(n)) { if (!kept.length || !inside(kept.at(-1))) kept.push(n); } // kept subtrees are disjoint
        else { while (kept.length && inside(kept.at(-1))) kept.pop(); kept.push(n); }
      }
      found = kept;
    }
    if (found.length === 1) return found[0];
    throw new Error(`${JSON.stringify(target)} names ${found.length} views: ${found.slice(0, 8).map((n) => `${n.id}${n.props.testId ? ` (${n.props.testId})` : ''} ${n.type}`).join(', ')}${found.length > 8 ? ', …' : ''}; target one by its view id`);
  }
  return null;
}

/** A few targets an agent can name instead of a point: interactive views' testIds and labels, else their text. */
export function targetsIn(nodes, limit = 12) {
  const named = [...new Set(nodes.filter((n) => interactive(n) && !n.inactive).map((n) => {
    const text = textOf(nodes, n);
    return name(n) ?? (text != null && String(text).trim() ? JSON.stringify(String(text).trim()) : null);
  }).filter(Boolean))];
  return named.length ? `; targets here: ${named.slice(0, limit).join(', ')}${named.length > limit ? ', …' : ''}` : '';
}

/** Where a native host keeps a drive's scratch stores (host/apple and host/linux `configure_storage`), or
 * null where the driver cannot reach them: an iOS simulator's are in its app container and go with the app. */
export function storeBase(appId, host, env = process.env, home = homedir()) {
  if (host === 'macos' || host === 'mac') return resolve(home, 'Library/Caches/exact', appId, 'agent');
  if (host === 'linux') return resolve(env.XDG_CACHE_HOME?.startsWith('/') ? env.XDG_CACHE_HOME : resolve(home, '.cache'), 'exact', appId, 'agent');
  return null;
}
/** The origins a web build's data module may fetch: its `net.fetch` grants, as host/web-js/build.mjs writes them
 * into the page's admission data (none for a wasm build). */
export function fetchOrigins(dist) {
  let text = ''; try { text = readFileSync(resolve(dist, '.gen/admission-data.js'), 'utf8'); } catch { return []; }
  return [...new Set([...text.matchAll(/"net\.fetch (https?:\/\/[^/"\s]+)[/"]/g)].map((m) => m[1]))];
}
/** The loopback ones: a backend on this machine the tests talk to. */
export const loopbackOrigins = (dist) => fetchOrigins(dist).filter((o) => /^https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?$/.test(o));
/** The source that selected the built backend, with a stale build stated explicitly. */
export function backendOriginSource(app, origin, env=process.env) {
  const dir=typeof app==='string' ? app : app.dir;
  if (!dir) return 'built net.fetch grant';
  const selected=backendOriginSelection(dir,env);
  if (selected?.origin===origin) return selected.source;
  let code=''; try { code=readFileSync(resolve(dir,'app.ts'),'utf8'); } catch { /* no TypeScript source */ }
  const literal=/\bconst\s+origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(code)?.[1]?.replace(/\/+$/,'');
  return !selected && literal===origin ? 'app.ts origin literal' : 'built net.fetch grant (rebuild after changing the backend selection)';
}
const listening = (origin, ms = 1000) => new Promise((done) => {
  const { hostname, port, protocol } = new URL(origin), socket = connect({ host: hostname.replace(/^\[|\]$/g, ''), port: Number(port || (protocol === 'https:' ? 443 : 80)) });
  const end = (ok) => { socket.destroy(); done(ok); };
  socket.setTimeout(ms, () => end(false)).once('connect', () => end(true)).once('error', () => end(false));
});
/** Before a web run, every loopback origin the app fetches is asked whether anything listens (app farm wave 1: four
 * builds' tests failed as data mismatches with no `snapback4 dev` running). An app whose data is a Snapback client
 * (it mounts snapback4/ts) cannot open a partition that never synced, so every test would fail: the run is refused,
 * naming the origin. Any other app is told, and runs (its tests may be of the error path). */
export async function backendPreflight(app, dist, say = console.error) {
  const down = [];
  for (const origin of loopbackOrigins(dist)) if (!(await listening(origin))) down.push(origin);
  if (!down.length) return devicePreflight(app, fetchOrigins(dist));
  const what = `no backend at ${down.map(origin=>`${origin} (source: ${backendOriginSource(app,origin)})`).join(', ')}: nothing listens there, and app.ts fetches it (a net.fetch grant)`;
  if (!mountsSnapback(app)) return say(`note: ${what}; a test that loads from it fails`);
  throw new Error(`${what}. The app's Snapback client cannot open data that has never synced, so every test would fail. Start the server first, from the backend's directory (\`npx snapback4 dev --port ${new URL(down[0]).port || 80}\`, seeded if the tests read seeded rows), then run the tests again.`);
}
/** A `snapback4 dev` server's commit head (`seq` of `GET /changes`), asked as its first declared persona; null for
 * any other backend (no `/dev/status`: `serve`, a server of another kind), one not reached, or a refused ask. The
 * tests of a run share that server and every write before the run (sb4 farm wave 7, E1: six builds read a rerun's
 * or an earlier test's rows as an app bug), so a failed test says how many commits it began with. Advice only. */
export async function backendHead(origin, ms = 1000) {
  const get = async (path, headers) => { const r = await fetch(origin + path, { headers, signal: AbortSignal.timeout(ms) }); return r.ok ? r.json() : null; };
  try {
    const status = await get('/dev/status');
    if (typeof status?.storeId !== 'string') return null;
    const changes = await get('/changes?since=0', { 'x-snapback-persona': status.personas?.[0] ?? 'exact' });
    return Number.isInteger(changes?.seq) ? changes.seq : null;
  } catch { return null; }
}
export async function backendClock(origin, ms = 1000) {
  try {
    const response = await fetch(`${origin}/dev/status`, {signal: AbortSignal.timeout(ms)});
    const status = response.ok ? await response.json() : null;
    return typeof status?.storeId === 'string' && Number.isFinite(status.now) ? status.now : null;
  } catch { return null; }
}
export function backendClockNote(epoch, now, origin) {
  if (!Number.isFinite(epoch) || !Number.isFinite(now) || Math.abs(epoch - now) <= 86_400_000) return null;
  return `drive epoch ${new Date(epoch).toISOString()} differs from the backend clock at ${origin} (${new Date(now).toISOString()}) by more than a day; deadlines, ages and windows use different clocks. Write \`epoch now\` to follow real time, or set both clocks deliberately.`;
}

export function rosterRefusalNote(reset, schema, refusals) {
  if (!reset || reset.seed != null || !refusals?.size || !/^[ \t]+insert[ \t]*<-[ \t]*deny[ \t]*(?:--[^\n]*)?\r?$/m.test(schema)) return null;
  const authorization = /\b(?:E_RULE|E_AUTH|E_PERMISSION|E_FORBIDDEN|FORBIDDEN|DENIED|NOT_(?:OWNER|ADMIN|MANAGER|MEMBER|STAFF|AUTHORIZED|ALLOWED))\b/;
  if (![...refusals.values()].every(why => authorization.test(why))) return null;
  return `possible cause: every privileged operation refused after \`fresh\`: seed roster tables. Authorization refusals: ${[...refusals.values()].map(why => authorization.exec(why)[0]).join(', ')}. Bare \`fresh\` removed the insert <- deny rows; use \`fresh "seed.json"\` with [{"table":"…","row":{…}}], resolved from the test file’s directory.`;
}
// @ref LLP 1086 D13 — `fresh` (the author's ruling of 2026-10-12, via the sb4 app farm): a test file's per-test reset.
/** Before a test opens, each snapback4 dev server the app fetches is emptied through its development door, `POST
 * /dev/reset` (dev only: the app's tables and derived data emptied; a body of `[{table, row}]` seeds it after; it answers
 * `{reset: true, seeded, generation}`). An origin that is no snapback4 dev server (no `/dev/status` with a `storeId`) is
 * left alone. Nothing to reset, an origin that does not answer, or a server older than the door (404) refuses the test
 * with what to do. Returns a line for each server reset. */
export async function resetBackends(origins, rows = null, ms = 10000, app = null) {
  const said = [];
  for (const origin of origins) {
    let status;
    try { const r = await fetch(`${origin}/dev/status`, { signal: AbortSignal.timeout(ms) }); status = r.ok ? await r.json().catch(() => null) : null; }
    catch (e) { throw new Error(`fresh: nothing answered at ${origin}${app ? ` (source: ${backendOriginSource(app,origin)})` : ''} (${e.cause?.code ?? e.message}): start \`snapback4 dev --port ${new URL(origin).port || 80}\` there first`); }
    if (typeof status?.storeId !== 'string') continue;
    const r = await fetch(`${origin}/dev/reset`, { method: 'POST', signal: AbortSignal.timeout(ms), ...(rows ? { headers: { 'content-type': 'application/json' }, body: JSON.stringify(rows) } : {}) });
    const text = await r.text();
    if (r.status === 404) throw new Error(`fresh: the snapback4 dev server at ${origin} has no reset door (POST /dev/reset answered 404: ${text.slice(0, 160)}); it is older than the snapback4 that adds it. Install snapback4@0.4.23 in the backend's directory`);
    let reply = null; try { reply = JSON.parse(text); } catch { /* said below */ }
    if (!r.ok || reply?.reset !== true) throw new Error(`fresh: POST ${origin}/dev/reset answered ${r.status}: ${text.slice(0, 300)}`);
    said.push(`${origin} reset to generation ${reply.generation}${rows ? `, ${reply.seeded} row${reply.seeded === 1 ? '' : 's'} seeded` : ''}`);
  }
  if (!said.length) throw new Error(`fresh: no snapback4 dev server at ${origins.length ? origins.join(', ') : 'any loopback origin the app fetches (its web build\'s net.fetch grants: none)'}; \`fresh\` empties one before each test`);
  return said;
}
/** A `fresh` line's seed file, a path from the test file: a JSON list of `{table, row}`. */
export function freshRows(file, seed) {
  if (seed == null) return null;
  const path = resolve(dirname(resolve(file)), seed);
  let rows; try { rows = JSON.parse(readFileSync(path, 'utf8')); } catch (e) { throw new Error(`fresh ${JSON.stringify(seed)}: ${path}: ${e.code === 'ENOENT' ? 'no such file' : e.message}`); }
  if (!Array.isArray(rows) || rows.some((r) => typeof r?.table !== 'string' || r.row === null || Array.isArray(r.row) || typeof r.row !== 'object')) throw new Error(`fresh ${JSON.stringify(seed)}: ${path} is not a list of {"table": "…", "row": {…}}`);
  return rows;
}
/** The note a failed test gets when the backend held commits as it began: from before the run, from earlier tests. */
export function sharedBackendNote(origin, before, earlier) {
  const n = (k) => `${k} commit${k === 1 ? '' : 's'}`;
  const from = [before ? `${n(before)} from before this run (a seed, or an earlier run on this server)` : '', earlier ? `${n(earlier)} from earlier tests in this run` : ''].filter(Boolean).join(' and ');
  return `the backend at ${origin} held ${from} when this test began: every test and every run on that server sees those rows, so a count, a position or a "taken" refusal can be theirs. Give each test its own rows (a code or name only it uses, asserted newest first or through its own reply), or each test file its own empty server (snapback4/README.md, "Tests share the server")`;
}
const alive = pid => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
/** The stores of authored-test runs that are gone (`<storage>.r<pid>-<tag>.t<n>`, the pid no longer running):
 * a run killed before it removed its own. A live run's are left alone, so concurrent runs never share or
 * empty one another's. */
export function sweepTestStores(base, storage) {
  if (!base) return;
  let names = []; try { names = readdirSync(base); } catch { return; }
  const ours = new RegExp(`^${storage.replace(/[.]/g, '\\.')}\\.r(\\d+)-[0-9a-z]+\\.t\\d+$`);
  for (const name of names) { const m = name.match(ours); if (m && !alive(Number(m[1]))) rmSync(resolve(base, name), { recursive: true, force: true }); }
}
/** `"backend"` follows the app's single net.fetch origin at test runtime. Literal prefixes stay literal. */
export function backendFetchPrefixes(tests, origins) {
  const unique = [...new Set(origins.map(o => o.replace(/\/+$/, '')))];
  for (const t of tests) for (const step of t.steps) {
    if (!['fail-fetch', 'pass-fetch'].includes(step.op)) continue;
    step.prefix = resolveFaultPrefix(step.prefix, unique);
  }
  return tests;
}

/** Each mutation whose reply was refused, and why, from the journal line both runners write when they let its request
 * go (`request N (name) failed and is no longer pending: <it ends unsent | it is not retried>: <why>`). A mutation has no
 * `state.failed`: its slot keeps what it had and its `then` never runs, so this is the only place the reason is said
 * (sb4 farm wave 6, E3: an answer's undeclared field voided a reply, 25 minutes with nothing naming the field). */
export function refusedReplies(lines) {
  const refused = new Map();
  for (const l of Array.isArray(lines) ? lines : []) {
    const m = (typeof l === 'string' ? l : JSON.stringify(l)).match(/request \d+ \(([^)]+)\) failed and is no longer pending: (?:it ends unsent|it is not retried): (.*)$/)
      ?? (typeof l === 'string' ? l : JSON.stringify(l)).match(/send ([^ ]+) failed: (.*); it ends unsent$/);
    if (m) refused.set(m[1], m[2]);
  }
  return refused;
}
/** Assert a mutation's observed refusal, preserving only its matching journal lines for verify. */
export function refusalExpectation(logs, target) {
  const lines=Array.isArray(logs?.lines) ? logs.lines : [];
  for (let i=lines.length-1;i>=0;i--) {
    const why=refusedReplies([lines[i]]).get(target);
    if (why==null) continue;
    const expected=[String(lines[i])];
    // A failed reply's transaction line precedes the request's terminal failure.
    for (let j=/\brequest \d+ \(/.test(String(lines[i])) ? i-1 : -1;j>=0;j--) {
      const line=String(lines[j]).replace(/^t=\d+(?:\.\d+)?\s+/, '');
      if (/^request \d+ /.test(line)) break;
      if (line.startsWith(`refused reply ${target};`) && line.endsWith(`: ${why}`)) { expected.push(String(lines[j])); break; }
    }
    return { expected };
  }
  return { message: `expected mutation ${JSON.stringify(target)} to refuse, but no refused reply was observed; clock data or clock settle lands a send` };
}

const replyNote = (name, why) => `mutation ${name}'s reply was refused: ${why}; its slot keeps what it had and its \`then\` does not run`;
/** The notes a CLI drive writes on stderr beside its replies, each once (LLP 1102 §3.17; app farm round 1, where a
 * shape refusal lived only in `logs` and an early `tree` showed an empty list): a drive with no scratch store whose
 * source was refused storage; a resource that failed, and why; a read taken while the app's data is in flight. Call it
 * after each op with the op's name, and with `end` when the drive ends. The web reads after each input; a native
 * carrier, whose reads could be slow, at the end; both at a read before the clock first moved. Advice only: bounded,
 * and never fails a drive. */
/** The journal line every host writes at a drive's first refused read or write of app storage, when the drive names no store. */
export const STORAGE_REFUSED = /storage refused \(agent\)/;
export const STORAGE_NOTE = 'this drive names no --storage, so the app\'s storage was refused: a data source that reads its files finds none and keeps nothing, so a screen may show an empty store, not the app\'s data; pass --storage <name> (docs/agent-pitfalls.md)';
/** The held device requests a person would see as a system sheet, by capability; and those iOS presents for real
 *  beside the hold under `--touch platform` (Share.swift, DocumentPickers.swift). */
const HELD_SHEETS = { share: 'share sheet', pick: 'photo or file picker', 'open-file': 'document picker', 'open-directory': 'document picker', 'save-file': 'document picker', export: 'export sheet', auth: 'sign-in sheet' };
const SHOWN_SHEETS = ['share', 'open-file', 'open-directory', 'save-file'];
// Native and web canvas journals advance their own cursors on every wire read.
// Advice owns no journal entries: keep its batches until the caller asks for logs.
const peekedWorldLogs = new WeakMap();
export function resetLogs(s) {
  peekedWorldLogs.delete(s);
  s.logCursor = 0;
}
export async function peekLogs(s, since) {
  let sessions = peekedWorldLogs.get(s);
  if (!sessions) peekedWorldLogs.set(s, sessions = new Map());
  const session = s.session, reply = await s.op({ op: 'logs', since });
  // A probe started before a reload belongs to the old page, even if it replies later.
  if (reply.world?.length && peekedWorldLogs.get(s) === sessions)
    sessions.set(session, [...(sessions.get(session) ?? []), ...reply.world]);
  return reply;
}
export function takeWorldLogs(s, world) {
  const sessions = peekedWorldLogs.get(s), previous = sessions?.get(s.session) ?? [];
  if (!previous.length) return world;
  // Equal cursor boundaries can belong to different canvas lifetimes. Keep each read's metadata.
  const batches = [...previous, ...(world ?? [])];
  // Consume only after collecting all rows succeeds, so a failed read can be retried.
  sessions?.delete(s.session);
  return batches;
}
export function driveNotes(s, { host, storage, origins = [] }) {
  const INPUTS = ['tap', 'type', 'clock'], READS = ['tree', 'screenshot', 'layout'];
  const said = new Set(), note = (key, text) => { if (!said.has(key)) { said.add(key); console.error(`note: ${text}`); } };
  const bounded = (p, ms) => Promise.race([p.catch(() => null), new Promise((done) => setTimeout(() => done(null), ms))]);
  let peek = 0, probes = 0, clocked = false, replies = 0, checkedClock = false;
  return async (op) => {
    if (s.carrier?.installNote) note('install', s.carrier.installNote); // a Release build replaced for the drive (devices.mjs `keepInstalledRelease`)
    // A system sheet held for the agent is drawn by no host: a screenshot of it shows the app alone (euxy port).
    if (op === 'screenshot') {
      const ios = ['ios', 'host-ios'].includes(host), real = ios && s.carrier?.touch === 'platform';
      for (const p of (await bounded(s.state(), 3000))?.pending ?? []) {
        const capability = p.device?.capability, kind = HELD_SHEETS[capability], shown = SHOWN_SHEETS.includes(capability);
        if (kind && !(real && shown)) note(`sheet ${p.ticket}`, `the screenshot shows no ${kind}: under the agent it is held (@${p.ticket}), answered by tap @${p.ticket} or type @${p.ticket}${ios && shown ? '; under --touch platform the real one is presented beside the hold, and a screenshot draws it' : ''}`);
      }
    }
    if (!checkedClock) {
      checkedClock = true;
      const clocks = await Promise.all(origins.map(origin => backendClock(origin)));
      for (const [i, origin] of origins.entries()) {
        const warning = backendClockNote(s.launchFacts?.epoch, clocks[i], origin);
        if (warning) note(`clock ${origin}`, warning);
      }
    }
    const end = op === 'end', early = READS.includes(op) && !clocked && !said.has('early');
    if (op === 'clock') clocked = true;
    // Said once a drive, after the first op that met it, whatever that op was: a first screenshot of an app whose
    // sources were refused their storage reads as an app bug (the euxy port). Not here when an input's reply said it.
    if (storage === undefined && !said.has('storage') && !s.storageSaid && ++probes <= 20) {
      try {
        const j = await bounded(peekLogs(s, peek), 3000);
        if (Array.isArray(j?.lines)) {
          peek = j.next;
          if (j.lines.some((l) => STORAGE_REFUSED.test(typeof l === 'string' ? l : JSON.stringify(l)))) note('storage', STORAGE_NOTE);
        }
      } catch { /* advice only */ }
    }
    if (!(end || early || (host === 'web' && INPUTS.includes(op)))) return;
    try {
      const j = await bounded(peekLogs(s, replies), 3000);
      if (Array.isArray(j?.lines)) replies = j.next;
      for (const [name, why] of refusedReplies(j?.lines)) note(`reply ${name} ${why}`, replyNote(name, why));
      const state = await bounded(s.state(), 3000);
      // Resource failures are normal authored states (failed/failure gates). Keep them in state/logs;
      // request-shape refusals above and failing assertions below still get actionable notes.
      if (process.env.EXACT_AGENT_DEBUG) for (const [name, why] of Object.entries(state?.failed ?? {})) note(`failed ${name} ${why}`, `resource ${name} failed: ${why}`);
      const pending = (state?.pending ?? []).filter((p) => !p.device).map((p) => p.name);
      if (early && pending.length) note('early', `${op} read the app with ${pending.length} request${pending.length === 1 ? '' : 's'} in flight (${pending.join(', ')}): a reply lands at a clock step, so a drive that reads the app's data starts with "clock data"`);
    } catch { /* advice only */ }
  };
}
/** A test's `request` line (LLP 1116.006 D8): `st.path` under `base`, the server URL the app calls (or a whole URL),
 * its body as JSON, `as` the bearer token, as the rest pattern names a persona. A status outside 200 through 299 or no
 * answer throws, saying what came back; with `expect <status>` (LLP 1116.010 D3) that status is what passes, and any other
 * throws: the test of a refusal. */
export async function serverRequest(base, st) {
  const whole = /^https?:\/\//.test(st.path);
  if (!whole && !base) throw new Error(`request: app.ts calls no local server (http://127.0.0.1:<port>), so ${JSON.stringify(st.path)} is under nothing; write the whole URL`);
  const url = whole ? st.path : `${base}${st.path}`, said = `request ${st.method} ${url}${st.as ? ` as ${st.as}` : ''}`;
  const headers = { ...(st.body != null ? { 'content-type': 'application/json' } : {}), ...(st.as ? { authorization: `Bearer ${st.as}` } : {}) };
  let res;
  try { res = await fetch(url, { method: st.method, headers, body: st.body ?? undefined, signal: AbortSignal.timeout(10_000) }); }
  catch (e) { throw new Error(`${said}: nothing answered (${e.message}); test and verify start the app's server (server.ts, at the address app.ts calls) for a run; a server of its own that they leave alone must be running`); }
  const text = (await res.text().catch(() => '')).trim();
  const answered = `the server answered ${res.status}${text ? ` ${text.slice(0, 300)}` : ''}`;
  if (st.expect != null ? res.status !== st.expect : !res.ok) throw new Error(`${said}: ${answered}${st.expect != null ? `, expected ${st.expect}` : ''}`);
}

/** A launch line's op and the `open` option it sets (`size` aside: it is two numbers). */
const LAUNCH = { epoch: 'epoch', 'time-zone': 'timeZone', locale: 'locale', seed: 'seed' };

/**
 * Run a `test "…"` file against a host. Each test is a session of its own
 * from the first frame, opened with its launch lines — `size`, `epoch`,
 * `time-zone`, `locale`, `seed`, the test's own or the file's (the compiler
 * puts them first), else the drive's flags — with app storage of its own: a scratch store
 * `<storage>.r<pid>-<tag>.t<n>` of this run's (Chrome's profile for it on the web), emptied at
 * launch and removed after the test (with any a killed run left), so an app
 * that keeps its data in storage loads and no test, concurrent run or
 * earlier run sees another's writes. The app's data lands before the first step (and after a `reload`), unless
 * the test says `before data`. A failed expect names the test, the line, and what
 * was seen. Returns `{ passed, failed, results }`, each result `{ name, failures, notes }`: notes are advice, never a failure.
 * `atEnd(s, { name, index, size, store, failures, reload, launch, reached, restart, typed, touched })` (verify's, LLP 1116.005 D7; `launch`: the epoch, zone, locale
 * and seed the test launched with, LLP 1116.007 D3.4; `reached`: the epoch a relaunch starts at, the date it launched at plus the virtual
 * time it reached, as a `reload` relaunches, LLP 1116.009 D1; `restart`: on the web, the `reload` step itself, run on the session so the page loads
 * again on its route and storage at that date, LLP 1116.010 D6; `typed`: the ids the test typed since its last other input, a draft; `touched`: every id it typed) runs on each test's session after its last step, unless the window closed; what it returns is the result's `end` (`{ error }` if it threw), and an
 * `end.keep` keeps the test's store for a later launch (the result's `store`), which the caller then removes.
 * `atStep(s, { name, line, op })` runs on the session after each input step (and `reload`) lands; returning 'stop' ends
 * that test there, 'done' every test from there (verify's tour of the screens the tests pass through).
 */
export async function runTests({ host, browser, file, plan, app, size, env, webDist, device = false, phone, url, seed, locale, timeZone, epoch, failFetch, storage = 'test', touch = 'agent', chrome: bars = 'agent', scale, atEnd, atStep, port } = {}) {
  const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
  if (host === 'web' && !url) await backendPreflight(resolveApp(app), resolve(webDist ?? defaultWebDist()));
  // A fetch prefix outside every grant fails nothing (sb4 farm wave 4, lane 070: an origin moved in app.ts only).
  const granted = host === 'web' && !url ? fetchOrigins(resolve(webDist ?? defaultWebDist())) : [];
  // The Snapback dev servers the tests share, and each one's head before the first test (`backendHead`).
  const snapback = Object.values(resolveApp(app).manifest?.typescript?.sources ?? {}).some(p => /(?:^|\/)snapback4\/(?:ts|online)\/?$/.test(p));
  const shared = host === 'web' && !url && snapback ? loopbackOrigins(resolve(webDist ?? defaultWebDist())) : [];
  const atStart = await Promise.all(shared.map((o) => backendHead(o)));
  const clocks = await Promise.all(shared.map(o => backendClock(o)));
  let schema = '';
  try { schema = readFileSync(resolve(resolveApp(app).dir, '../snapback/schema.q'), 'utf8'); } catch { /* No schema beside this app. */ }
  // What `fresh` resets: every loopback origin the app's web build fetches (its net.fetch grants).
  const backends = url ? [] : loopbackOrigins(resolve(webDist ?? defaultWebDist()));
  // Cargo owns target selection and freshness, including CARGO_TARGET_DIR.
  const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'test', resolve(file)], { cwd: root, encoding: 'utf8' });
  if (c.status !== 0) throw new Error(c.stderr?.trim() || c.error?.message || 'contract test compiler failed');
  // Native runs use the scaffold's same declared origin when no web admission file exists.
  const appOrigin = host !== 'web' && existsSync(resolve(resolveApp(app).dir, 'app.ts')) ? /const origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(readFileSync(resolve(resolveApp(app).dir, 'app.ts'), 'utf8'))?.[1] : null;
  const tests = backendFetchPrefixes(JSON.parse(c.stdout), granted.length ? granted : appOrigin ? [appOrigin] : []);
  const reseeded = tests.filter(t => t.steps.some(s => s.op === 'fresh')).length;
  if (reseeded) console.error(`fresh: reseeding before each of ${reseeded} tests`);
  const results = [];
  // Where the host keeps its stores, as it will see its environment (a drive's env overrides the driver's).
  const launched = { ...process.env, ...(env ?? {}) };
  const selected = resolveApp(app), id = selected.id, chrome = host === 'web' && (browser ?? launched.EXACT_WEB_BROWSER ?? 'chrome') === 'chrome';
  const base = device ? null : chrome ? driveStore(selected, storage, env).base : host === 'web' ? null : storeBase(id, host, launched, launched.HOME || homedir());
  // A run's own names where the driver can remove them; a simulator's (one drive at a time: a launch ends the
  // last) reuse one store a test, emptied at launch, so they cannot pile up in its app container.
  // Android's stores are on the device, out of the driver's reach, but a run's names stay its own there too (LLP 1107).
  const tag = base || host === 'android' ? `.r${process.pid}-${Math.random().toString(36).slice(2, 8)}` : '';
  sweepTestStores(base, storage);
  const server = serverUrl(selected.dir); // where a `request` line's path is (LLP 1116.006 D8)
  let done = false;
  for (const [n, t] of tests.entries()) {
    if (done) break;
    const failures = [], expectedRefusals = [];
    const refusalCursors = new Map();
    let end = null;
    const store = base || host !== 'web' ? `${storage}${tag}.t${n}` : storage, fresh = { ...(env ?? {}), EXACT_AGENT_STORAGE_FRESH: '1' };
    // A test's launch lines lead its steps and override the drive's flags (habits F7).
    // The drive's `--fail-fetch` is every test's, with its own leading `fail fetch` lines added (LLP 1103 D3).
    const facts = { size, seed, locale, timeZone, epoch, failFetch };
    const lines = [];
    let beforeData = false, leading = 0, reset = null;
    // `fail fetch` lines that lead the steps are armed before the app's first data load (LLP 1103 D3).
    const armed = new Map(); // prefix -> the line that armed it with a count
    // Advice beside the verdict, never a failure: a `fail fetch` armed before the app's data first loaded (app farm
    // round 2: six builds read the failed first load as a broken app; a client that must sync once cannot open offline).
    const notes = [], shots = []; // `shots`: the paths of the test's own `screenshot` steps (verify --shots copies them)
    for (const st of granted.length ? t.steps : []) if ((st.op === 'fail-fetch' || st.op === 'pass-fetch') && !granted.some((o) => o.startsWith(st.prefix) || st.prefix.startsWith(o))) notes.push(`line ${st.line}: \`${st.op.replace('-', ' ')} ${JSON.stringify(st.prefix)}\` names no origin app.ts fetches (its net.fetch grants: ${granted.join(', ')}), so it changes nothing`);
    const early = (st) => notes.push(`line ${st.line}: \`fail fetch ${JSON.stringify(st.prefix)}\` is armed before the app has loaded its data, so every first load to it fails; a client that must sync once before it works offline (Snapback4's) cannot open its data at all. To test going offline after a load, put \`clock data\` before it; a launch with no network is what it tests as written`);
    for (const st of t.steps) {
      if (st.op === 'before-data') beforeData = true;
      else if (st.op === 'fresh') reset = st;
      else if (st.op === 'fail-fetch') { early(st); facts.failFetch = [facts.failFetch, st.times == null ? st.prefix : `${st.prefix}\t${st.times}`].filter(Boolean).join('\n'); if (st.times != null) armed.set(st.prefix, st.line); }
      else if (st.op === 'size') facts.size = [st.width, st.height];
      else if (LAUNCH[st.op]) facts[LAUNCH[st.op]] = st.value;
      else break;
      lines.push(st.line);
      leading++;
    }
    // A zone or locale the driver refuses fails this test at its line, not the run. `epoch now` is read here, once
    // a test, so a `reload` relaunches from the same date on every host. `facts.epoch` is the date at the clock's zero
    // of the launch now running: a `reload` moves it on by the virtual time that launch reached (LLP 1116.009 D1).
    try { facts.epoch = launchFacts({ ...facts, env: env ?? {} }).epoch; } catch (e) {
      results.push({ name: t.name, failures: [`${t.name}: ${lines.length ? `line ${lines.join(', ')}` : "the drive's launch flags"}: ${e.message}`] });
      continue;
    }
    for (const [i, origin] of shared.entries()) {
      const warning = backendClockNote(facts.epoch, clocks[i], origin);
      if (warning) notes.push(warning);
    }
    // `fresh`: the backend emptied (and seeded) before the app opens; a refusal fails this test at its line.
    if (reset) try { await resetBackends(backends, freshRows(file, reset.seed), 10000, selected); } catch (e) {
      results.push({ name: t.name, failures: [`${t.name}: line ${reset.line}: ${e.message}`], notes });
      continue;
    }
    const heads = await Promise.all(shared.map((o) => backendHead(o)));
    // A drag on an iOS simulator is the touch runner's real gesture (LLP 1080.000 §11; chat2 diary: an authored
    // test could not drag there); every other step stays the agent's. A phone has no runner yet: its drag says so.
    // `--touch platform` makes every tap a real touch too (splitter rough 12: a test of what a finger reaches).
    const drags = !device && ['ios', 'host-ios'].includes(host) && t.steps.some((st) => st.op === 'drag');
    const fingers = touch !== 'agent' ? touch : drags ? 'drag' : 'agent';
    // The date a relaunch starts at: this launch's date at the clock's zero plus the virtual time the test has reached
    // (every `clock +N`, settle and timer), as the runner reports it; whole milliseconds, which every host takes.
    const first = facts.epoch; // the test's launch date, which a `reload` moves `facts.epoch` on from
    const reached = async () => { const { clock } = await s.op({ op: 'tags' }).catch(() => ({})); return facts.epoch + Math.floor(Number.isFinite(clock) ? clock : s.now); };
    const launch = (environment) => open({ host, browser, plan, ...facts, env: { ...environment, EXACT_AGENT_EPHEMERAL_PORT: '1' }, app, webDist, device, phone, url, storage: store, touch: fingers, chrome: bars, scale, port });
    let s = await launch(fresh);
    const mutationClock = testMutationClock();
    // The app's data lands before the first step, as `clock data` lands it: activation and every request in flight,
    // the clock unmoved and no timer fired (habits, pomodoro, kanban: a store opened at launch raced the first step).
    // `before data` does not wait (what has landed then is the host's: a native app ran on real time before the
    // driver connected). An unsettled wait leaves the expects to name what is still in flight.
    const data = async () => { if (!beforeData) await s.clock('data'); };
    // `reload`: the app restarts on the store it had (mail F19, kanban F25): the web page loads again in its
    // profile, keeping what the origin stored; a native app relaunches on the same scratch store, not emptied; both
    // at the virtual time the test had reached, as a phone restarted at 9:05 wakes at 9:05 (LLP 1116.009 D1).
    // It asks what persists, so storage the app started and did not await finishes first, as `clock data` lands it
    // (LLP 1097 D9): a write lost at `reload` is what a crash does. The next `logs` begins with what it waited for.
    const reload = async () => {
      mutationClock.reset();
      const b = (await s.state().catch(() => ({}))).background, waiting = b ? b.queued + b.inFlight : 0;
      if (waiting) {
        const r = await s.clock('data');
        if (r.settled === false) throw new Error(`reload: the app's storage did not finish before the restart: ${r.diagnostic ?? r.reason}`);
      }
      const notes = waiting ? [`reload: waited for ${waiting} storage operation${waiting === 1 ? '' : 's'}`] : [];
      // The fault table as it is now, not as it was at launch (LLP 1103 D3): a cleared or spent fault stays so.
      // A page whose table no fetch or fault request made yet still has its launch's; an unreadable state is an error.
      const now = await s.state().catch((e) => { throw new Error(`reload: could not read the fault table to carry: ${e.message}`); });
      const failFetch = now.faults ? faultSpecOf(now.faults) : undefined;
      facts.epoch = await reached();
      if (s.host === 'web') { await s.carrier.reset({ keep: true, failFetch, epoch: facts.epoch }); s.now = 0; resetLogs(s); s.notes = notes; return; }
      await s.close(); s = await open({ host, browser, plan, ...facts, failFetch: failFetch ?? facts.failFetch, env, app, webDist, device, phone, url, storage: store, touch: fingers, chrome: bars, scale, port }); s.notes = notes;
    };
    // The clock stands still between steps: what an input started (a reply,
    // a mutation's `then`, a timer, a transition) lands at a clock step. A
    // failed expect after an input with none says so (kanban F19).
    let input = null;
    // What the test typed since its last other input: fields it left holding a draft (verify's reload does not compare them); and all it typed.
    const typed = new Set(), touched = new Set();
    // Whether a clock step has let the app's first loads answer (without `before data`, the wait before the first step).
    let loaded = !beforeData;
    // The line whose `close` closed the window, if one did.
    let closedAt = null;
    // A `request` line's, until the clock moves on: the app shows the write when it next reads (its poll, `clock +N`).
    let unpolled = null;
    // An input the host could not perform fails its step: an unsupported drag or a refused tap did nothing to assert on.
    // An input that closed the window (`close`, or a press the app answered with `close()`) ends what can run.
    let current = null;
    const delivered = (r) => { if (r?.error || r?.delivery === 'unsupported') throw new Error(r.error ?? r.reason ?? 'the host does not support this input'); if (r?.landError) throw new Error(`the input was delivered; landing what it started failed: ${r.landError}`); if (r?.closed) closedAt = current; };
    // With no input since the clock last moved, a request still in flight (the boot's own, or one a jump
    // left on real time) is named: the expect read the value before its reply (workout F1).
    // A resource that failed is named first, with why (app farm round 1: a shape refusal read as a timing problem), then a
    // mutation whose reply was refused (sb4 farm wave 6, E3: the same refusal, with no `state.failed` to show it).
    const fail = async (message) => {
      const state = await s.state().catch(() => ({}));
      const failed = Object.entries(state.failed ?? {}).map(([name, why]) => ` (resource ${name} failed: ${why}; it shows its placeholder or last value)`).join('')
        + [...refusedReplies((await peekLogs(s, 0).catch(() => null))?.lines)].map(([name, why]) => ` (${replyNote(name, why)})`).join('');
      if (unpolled != null) return failures.push(`${message}${failed} (line ${unpolled} wrote to the server, and the clock has not moved past the app's next read since: \`clock +<its poll's ms>\`, then \`clock data\`)`);
      if (input != null) return failures.push(`${message}${failed} (the clock has not moved since line ${input}'s input: a reply, a mutation's \`then\` or a transition lands at \`clock settle\`; a timer fires when the clock reaches its time, \`clock +N\`)`);
      const pending = (state.pending ?? []).filter((p) => !p.device).map((p) => p.name);
      failures.push(`${message}${failed}${pending.length ? ` (${pending.length} request${pending.length === 1 ? '' : 's'} still in flight: ${pending.join(', ')}; a reply lands at a \`clock\` step, as \`clock settle\`)` : ''}`);
    };
    try {
      try { await data(); } catch (e) { failures.push(`${t.name}: waiting for the app's data before the first step: ${e.message}`); }
      // A counted fault that matched no fetch fails the test at the line that armed it (LLP 1103 D3).
      // A table the host never made (no fetch consulted it) matched nothing either; an unreadable state is said so.
      const unfired = async (prefix) => {
        const line = armed.get(prefix);
        if (line == null) return;
        armed.delete(prefix);
        let state;
        try { state = await s.state(); } catch (e) { failures.push(`${t.name}: line ${line}: fail fetch ${JSON.stringify(prefix)}: could not read whether it fired: ${e.message}`); return; }
        const f = (state.faults ?? []).find((e) => e.prefix === prefix);
        if (!f || f.hits === 0) failures.push(`${t.name}: line ${line}: fail fetch ${JSON.stringify(prefix)} times ${f?.times ?? '?'} matched no fetch`);
      };
      // Before an input while counted faults are outstanding: those that fired leave the check, so a press the app
      // answers with `close()` cannot take an unfired one's evidence with it (LLP 1103 D3).
      const INPUTS = new Set(['tap', 'drag', 'type', 'key', 'pick', 'clipboard', 'resize', 'back']);
      const settleFired = async () => {
        if (!armed.size) return;
        const faults = (await s.state()).faults ?? [];
        for (const f of faults) if (f.hits > 0) armed.delete(f.prefix);
      };
      for (const [n, st] of (failures.length ? [] : t.steps).entries()) {
        const at = `${t.name}: line ${st.line}`;
        if (closedAt != null) { failures.push(`${at}: the window closed at line ${closedAt}, so nothing after it runs`); break; }
        current = st.line;
        try {
          if (INPUTS.has(st.op)) await settleFired();
          if (st.op === 'type') { typed.add(st.target); touched.add(st.target); } else if (INPUTS.has(st.op) || st.op === 'reload') typed.clear();
          switch (st.op) {
            case 'size': case 'epoch': case 'time-zone': case 'locale': case 'seed': case 'before-data': case 'fresh': break; // the session opened with it
            // A driver fault (LLP 1103): a leading one was a launch line; a later one arms (or re-arms) now, a `pass` stops it.
            case 'fail-fetch': {
              if (n < leading) break;
              // Under `before data` nothing has loaded until a clock step (or a reload) waits for it.
              if (beforeData && !loaded) early(st);
              await unfired(st.prefix);
              const r = await s.op({ op: 'prefer', faults: { fail: st.prefix, ...(st.times != null ? { times: st.times } : {}) } });
              if (r?.error) throw new Error(r.error);
              if (st.times != null) armed.set(st.prefix, st.line);
              break;
            }
            case 'pass-fetch': { const r = await s.op({ op: 'prefer', faults: { pass: st.prefix } }); if (r?.error) throw new Error(r.error); break; }
            // The driver's `tap` forms (feed F10): `into` brings a virtualized list's row into view by its key.
            case 'tap': {
              const opts = st.form === 'into' ? { into: { key: st.key } }
                : st.form === 'pinch' ? { pinch: st.scale, ...(st.at ? { at: st.at } : {}) }
                : st.form === 'mediasession' ? { mediaSession: st.action, ...(st.seconds != null ? { seconds: st.seconds } : {}) }
                : st.form !== 'press' ? { [st.form]: true }
                : st.modifiers ? { modifiers: st.modifiers } : undefined;
              delivered(await s.tap(st.target, opts)); input = st.line; break;
            }
            // MIDI (LLP 1125 §4): the ports a request finds, and a message from an input now.
            case 'prefer-midi': { const r = await s.op({ op: 'prefer', midi: { ...(st.state ? { state: st.state } : {}), ...(st.inputs ? { inputs: st.inputs } : {}), ...(st.outputs ? { outputs: st.outputs } : {}) } }); if (r?.error) throw new Error(r.error); await s.clock('settle'); break; }
            case 'tap-midi': delivered(await s.tap(null, { midi: st.clock ? { input: st.input, clock: st.clock } : { input: st.input, data: st.data } })); input = st.line; break;
            case 'drag': {
              const drag = { ...(st.to != null ? { to: st.to, ...(st.at ? { at: st.at } : {}) } : { dx: st.dx, dy: st.dy }), ...(st.from ? { from: st.from } : {}), ...(st.mouse ? { mouse: true } : {}), ...(st.press != null ? { press: st.press } : {}), ...(st.over != null ? { over: st.over } : {}), ...(st.hold != null ? { hold: st.hold } : {}) };
              if (st.during?.length) drag.during = st.during.map((op) => () => duringOp(s, op));
              delivered(await s.tap(st.target, { drag })); input = st.line; break;
            }
            case 'type': {
              // `append`: after the field's value as the tree shows it, the text a keyboard would add (feed F8).
              let text = st.text;
              if (st.append) {
                const { nodes } = await s.tree(), field = nodes.find((n) => n.props.testId === st.target && !n.inactive) ?? nodes.find((n) => n.props.testId === st.target);
                if (field && typeof field.props.value !== 'string') throw new Error(`type … append: "${st.target}" shows no text value to append to`);
                if (field?.props.type === 'password' && field.props.value !== '') throw new Error(`type … append: "${st.target}" is a password field, whose value the tree does not show`);
                text = (field?.props.value ?? '') + text;
              }
              delivered(await s.type(st.target, text)); input = st.line; break;
            }
            case 'reload': await reload(); refusalCursors.clear(); expectedRefusals.length = 0; await data(); input = null; loaded = true; break;
            case 'key': delivered(await s.type(st.target, { key: st.key, ...(st.phase ? { phase: st.phase } : {}), ...(st.for != null ? { for: st.for } : {}) })); input = st.line; break;
            // A held picker, by the node its answer arrives at or its capability (files F11); paths are the test file's.
            case 'pick': {
              try { delivered(st.paths.length ? await s.type(`@${st.target}`, st.paths.map((p) => resolve(dirname(resolve(file)), p)).join('\n') + '\n') : await s.tap(`@${st.target}`, { choice: 'cancel' })); }
              catch (error) { if (error.message.includes('held: none')) error.message += '; first tap a button whose action calls showPicker("<input id>"); the file input needs id as well as testId'; throw error; }
              input = st.line; break;
            }
            case 'clipboard': delivered(await s.type(st.target, { clipboard: st.edit, text: st.text })); input = st.line; break;
            case 'clock': await s.clock(st.arg); input = null; loaded = true; if (/^\+?\d/.test(st.arg)) unpolled = null; break;
            // The driver writes through the app's server as another person (LLP 1116.006 D8), not the app: nothing it
            // shows changes until it reads again.
            case 'request': await serverRequest(server, st); unpolled = st.line; break;
            case 'resize': delivered(await s.resize(st.width, st.height)); input = st.line; break;
            // The window's close button (studio diary R17): a window a `beforeunload` keeps stays and the test goes on;
            // one that closed takes the session, so a step after it fails naming it.
            // Counted faults are checked first: a window that closes takes the table with it.
            case 'close': for (const prefix of [...armed.keys()]) await unfired(prefix); delivered(await s.closeWindow()); input = st.line; break;
            // The platform's Back (LLP 1116.006 D4): an open dialog's cancel, else the top sheet's dismissal, else the pushed
            // route popped. Nothing to go back from fails the step; a Back the app refuses (a disabled control, closedby="none") is a note.
            case 'back': { const r = await s.back(); delivered(r); if (r?.refused) notes.push(`line ${st.line}: back refused: ${r.refused}`); input = st.line; break; }
            case 'screenshot': await s.screenshot(st.path); shots.push(resolve(st.path)); break;
            case 'expect-tree': {
              const tree = await s.tree();
              const f = treeExpectation(tree.nodes, st.target, st.present);
              // The first step of a test that waited for data cannot see the boot's loading view (authoring bench).
              const first = !beforeData && t.steps.slice(0, t.steps.indexOf(st)).every((p) => p.op in LAUNCH || p.op === 'size');
              if (f?.unshown) failures.push(`${at}: ${f.message}`);
              else if (f) await fail(`${at}: ${f.message}${first && st.present ? ' (the test waited for the app\'s data before its first step; `before data`, a launch line, starts without that wait)' : ''}`);
              break;
            }
            case 'expect-refusal': {
              const logs=await s.op({op:'logs',since:refusalCursors.get(st.target) ?? 0});
              const refusal=refusalExpectation(logs,st.target);
              if (refusal.message) await fail(`${at}: ${refusal.message}`);
              else { expectedRefusals.push(...refusal.expected); refusalCursors.set(st.target,logs.next); }
              break;
            }
            case 'expect-disabled': {
              const f = disabledExpectation((await s.tree()).nodes, st.target, st.disabled);
              if (f) await fail(`${at}: ${f.message}`);
              break;
            }
            case 'expect-text': {
              const f = textExpectation((await s.tree()).nodes, st.target, st.value);
              if (f?.unshown) failures.push(`${at}: ${f.message}`);
              else if (f) await fail(`${at}: ${f.message}`);
              break;
            }
            case 'expect-state': {
              const state = await s.state();
              const bag = { ...(state.resources ?? {}), ...(state.derives ?? {}), ...(state.slots ?? {}) };
              // A field or a list index at any depth, `name.field` or `rows.0` (feed F10, drums R7).
              const [name, ...fields] = st.name.split('.');
              if (!(name in bag)) { failures.push(`${at}: no state named "${name}"`); break; }
              let got = bag[name], path = name, missing = null;
              for (const field of fields) {
                if (got === null || typeof got !== 'object' || !(field in got)) { missing = field; break; }
                got = got[field]; path += `.${field}`;
              }
              if (missing != null) { await fail(`${at}: ${path} has no field "${missing}" (${got !== null && typeof got === 'object' && !Array.isArray(got) ? `its fields: ${Object.keys(got).join(', ')}` : `it is ${JSON.stringify(got).slice(0, 200)}`})`); break; }
              if (JSON.stringify(got) !== JSON.stringify(st.value)) await fail(`${at}: ${st.name} is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            // The runner's voice table, the whole record (LLP 1096 D10): a voice of that
            // source matching every clause given, or none; a dropped call is not a voice.
            case 'expect-sound': {
              const sounds = (await s.state(undefined, undefined, false, false, { sounds: 'all' })).sounds;
              const clauses = ['at', 'gain', 'ends', 'playbackRate', 'pan', 'by'].filter((k) => st[k] != null), said = clauses.map((k) => ` ${k} ${st[k]}`).join('');
              // `state.sounds` names a voice's rate and pan only when not 1 and 0 (LLP 1125 D10).
              const field = (v, k) => v[k] ?? { playbackRate: 1, pan: 0 }[k];
              if (!sounds) { failures.push(`${at}: the app declares no sound, so the runner keeps no voice table`); break; }
              const first = sounds.voices[0]?.at ?? Infinity;
              if (sounds.evicted > 0 && (st.at != null ? st.at < first : !st.present)) { failures.push(`${at}: voices before t=${first} are no longer recorded (the record keeps the last ${sounds.recorded})`); break; }
              const found = sounds.voices.some((v) => v.src === st.src && clauses.every((k) => field(v, k) === st[k]));
              if (found !== st.present) {
                const of = sounds.voices.filter((v) => v.src === st.src).slice(-8).map((v) => `#${v.id} at ${v.at} gain ${v.gain}${v.playbackRate != null ? ` playbackRate ${v.playbackRate}` : ''}${v.pan != null ? ` pan ${v.pan}` : ''} ends ${v.ends} by ${v.by}`);
                // A voice is recorded by the commit that issued it: no clock hint applies.
                failures.push(`${at}: expected ${st.present ? 'a' : 'no'} voice of "${st.src}"${said}; ${of.length ? `its voices: ${of.join('; ')}` : 'it has no voice'}`);
              }
              break;
            }
            // The MIDI queue's record (LLP 1125 D5): a message to that output, not taken back, at `at` when given.
            case 'expect-midi': {
              const midi = (await s.state(undefined, undefined, false, false, { midi: 'all' })).midi;
              if (!midi) { failures.push(`${at}: the app keeps no MIDI queue (no sequence, no sendMIDI, exactMIDI or midimessage)`); break; }
              const hex = st.data.map((b) => b.toString(16).padStart(2, '0')).join(' '), said = `"${st.output}" ${hex}${st.at != null ? ` at ${st.at}` : ''}`;
              const found = midi.sent.some((m) => m.output === st.output && m.data === hex && m.state !== 'retracted' && (st.at == null || m.at === st.at));
              if (found !== st.present) {
                const of = midi.sent.filter((m) => m.output === st.output).slice(-8).map((m) => `#${m.id} ${m.data} at ${m.at} ${m.state}${m.by ? ` by ${m.by}` : ''}`);
                failures.push(`${at}: expected ${st.present ? 'a' : 'no'} message to ${said}; ${of.length ? `its messages: ${of.join('; ')}` : `nothing was queued for it (access ${midi.state}, outputs ${midi.outputs.map((o) => o.id).join(', ') || 'none'})`}`);
              }
              break;
            }
            // The media session the host reports (LLP 1098 D10): a field, the owner by its testId, or an offered action.
            case 'expect-mediasession': {
              const ms = (await s.state()).mediaSession;
              if (!ms) { failures.push(`${at}: this host reports no media session`); break; }
              if (st.action != null) {
                if (ms.actions.includes(st.action) !== st.present) await fail(`${at}: expected the media session ${st.present ? 'to offer' : 'not to offer'} "${st.action}"; it offers ${ms.actions.length ? ms.actions.join(', ') : 'nothing'}${ms.owner == null ? ' (no owner)' : ''}`);
                break;
              }
              const got = st.field === 'owner' ? (ms.owner == null ? null : ms.testId ?? `view ${ms.owner}`) : st.field === 'playbackState' ? ms.playbackState : ms.metadata?.[st.field] ?? null;
              if (got !== st.value) await fail(`${at}: mediasession ${st.field} is ${JSON.stringify(got)}, expected ${JSON.stringify(st.value)}`);
              break;
            }
            default: failures.push(`${at}: unknown step ${st.op}`);
          }
          if (closedAt == null) await mutationClock.advance(s);
          if (atStep && closedAt == null && (INPUTS.has(st.op) || st.op === 'reload')) {
            const next = await atStep(s, { name: t.name, line: st.line, op: st.op });
            if (next === 'done') done = true;
            if (next === 'stop' || next === 'done') break;
          }
        } catch (e) {
          failures.push(`${at}: ${e.message}`);
          break;
        }
      }
      if (closedAt == null) for (const prefix of [...armed.keys()]) await unfired(prefix);
      else for (const [prefix, line] of armed) failures.push(`${t.name}: line ${line}: fail fetch ${JSON.stringify(prefix)} matched no fetch before the window closed at line ${closedAt}`);
      if (atEnd && closedAt == null) end = await atEnd(s, { name: t.name, index: n, size: facts.size, store, failures, reload: t.steps.some((st) => st.op === 'reload'), launch: launchFacts({ ...facts, epoch: first, env: env ?? {} }), reached: await reached(), typed: [...typed], touched: [...touched], expectedRefusals: [...expectedRefusals], ...(s.host === 'web' ? { restart: async () => { await reload(); await data(); } } : {}) }).catch((e) => ({ error: e.message }));
      // A failed test shows what the data module logged, the last lines of it (sb4 farm wave 7, E1: 093 rendered
      // values into the view to see them; `logs` had them all along). Advice only.
      if (failures.length && closedAt == null) try {
        const state = await s.state();
        const mutations = new Map(Object.entries(state.mutationFailures ?? {}).map(([name, failure]) => [name, `${failure.code}: ${failure.message}`]));
        const refusal = rosterRefusalNote(reset, schema, mutations.size ? mutations : refusedReplies((await peekLogs(s, 0))?.lines));
        if (refusal) notes.push(refusal);
        const logs = await Promise.race([s.logs(), new Promise((done) => setTimeout(() => done(null), 3000))]);
        const said = (logs?.host ?? []).filter((l) => /^console\.\w+: /.test(l));
        if (said.length) notes.push(`the data module logged${said.length > 8 ? ` (the last 8 of ${said.length})` : ''}: ${said.slice(-8).join(' | ')}`);
      } catch { /* advice only */ }
    } finally {
      // A window the test closed took its session (on macOS, the app) with it: nothing is left to close but the carrier.
      await s.close().catch((e) => { if (closedAt == null) throw e; });
    }
    if (failures.length && !reset) shared.forEach((o, i) => { if (heads[i] > 0 && atStart[i] != null) notes.push(sharedBackendNote(o, atStart[i], heads[i] - atStart[i])); });
    results.push({ name: t.name, failures, notes, ...(shots.length ? { shots } : {}), ...(end ? { end } : {}), ...(end?.keep ? { store } : {}) });
    if (base && !end?.keep) rmSync(resolve(base, store), { recursive: true, force: true });
  }
  const failed = results.filter((r) => r.failures.length).length;
  return { passed: results.length - failed, failed, results };
}

/** Resolve the built grant alias and diagnose a fault that cannot meet a granted URL. */
export function resolveFaultPrefix(prefix, origins, say=console.warn) {
  const unique=[...new Set(origins.map(o=>o.replace(/\/+$/,'')))];
  if (prefix==='backend') {
    const loopback=unique.filter(origin=>/^https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?$/.test(origin));
    const candidates=loopback.length ? loopback : unique;
    if (candidates.length!==1) throw new Error(`fetch "backend" needs exactly one net.fetch origin; found ${candidates.length}. Use a literal URL for a multi-backend app.`);
    return candidates[0];
  }
  if (!unique.some(origin => origin.startsWith(prefix) || prefix.startsWith(origin+'/') || prefix===origin))
    say(`warning: fetch fault prefix ${JSON.stringify(prefix)} matches no granted origin (${unique.join(', ') || 'none'}); it will not intercept the backend`);
  return prefix;
}

export function faultRequest(op, args, origins = [], say = console.warn) {
  if (args[0] !== 'fetch' || !args[1]) return rawFaultRequest(op, args);
  return rawFaultRequest(op, [args[0], resolveFaultPrefix(args[1], origins, say), ...args.slice(2)]);
}

/** The same persistence/fault behavior as an authored test's web reload. */
export async function reloadWeb(s) {
  if (s.host!=='web') throw new Error('reload in an interactive drive supports web; native authored tests relaunch their host');
  const state=await s.state(), background=state.background;
  if (background && background.queued+background.inFlight) {
    const result=await s.clock('data');
    if (result.settled===false) throw new Error('reload: pending storage did not settle');
  }
  const now=await s.state();
  await s.carrier.reset({keep:true, failFetch: now.faults ? faultSpecOf(now.faults) : undefined});
  s.now=0; resetLogs(s);
  return {reloaded:true};
}

export { driverCapabilities } from './help.mjs';

/** One millisecond per source mutation request, counted once across data continuations.
 * Action arguments retain their atomic snapshot; the next action sees the advanced clock. */
export function testMutationClock() {
  let cursor=0;
  return {
    reset() { cursor=0; },
    async advance(s) {
      let total=0;
      for(let round=0;round<16;round++) {
        const requests=(await s.state()).mutationRequests;
        if (!Number.isSafeInteger(requests) || requests<cursor) throw new Error('test clock: driver did not return a monotonic mutation request count');
        const mutations=requests-cursor;
        cursor=requests;
        if (!mutations) return total;
        total+=mutations;
        await s.clock('+'+mutations);
      }
      throw new Error('test clock: mutations kept issuing requests across 16 automatic clock steps');
    },
  };
}
