// The router on the JS target (LLP 1038; route/src), re-exported by rt.js: the verbs and reads, the route table, and the
// router slot's changes to the browser's history.
import { Refusal, effect, After, journal, clock, commit, W, pressFeedback, fact, Hosts, Resources } from "./rt.js";
import { projectRoots } from "./document.js";
import { canonical, locationOf, fragmentOf, enc, dec, matchPattern } from "./route-pattern.js";
export { canonical, locationOf };
const say = line => journal.push(`t=${clock.now} ${line}`);
// A Router is [tab, tabs, next]; a Tab [name, stack]; an Entry
// [id, name, url, tab, params, fragment], params positional in the table's
// first-declaration order of distinct `:names`.
let Routes = [], Names = [];
const names = p => p.split("/").filter(s => s[0] === ":").map(s => s.slice(1));
/** The plan's route table: [name, pattern, parent, tab, notfound] rows. */
export function routes(table) {
  Routes = table.map(([name, pattern, parent, tab, notfound]) => ({ name, pattern, parent, tab, notfound }));
  Names = [];
  for (const r of Routes) if (!r.notfound) for (const n of names(r.pattern)) if (!Names.includes(n)) Names.push(n);
}
const empty = () => Names.map(() => "");
function matchRoute(url) {
  const params = empty(), index = matchPattern(Routes, url, (name, value) => { params[Names.indexOf(name)] = value; });
  if (index >= 0) return [index, params];
  const nf = Routes.findIndex(r => r.notfound);
  return nf < 0 ? null : [nf, empty()];
}
const roots = () => { const r = Routes.map((x, i) => x.tab ? i : -1).filter(i => i >= 0); return r.length || !Routes.length ? r : [0]; };
function rootFor(i) {
  const rs = roots();
  if (!Routes[i].notfound) for (let c = i, k = 0; c != null && c >= 0 && k <= Routes.length; c = Routes[c].parent, k++) if (rs.includes(c)) return c;
  return rs[0];
}
const segmentOf = v => { if (["", ".", ".."].includes(v)) throw new Refusal("a path parameter cannot be empty, `.` or `..`"); return enc(v, b => !/[A-Za-z0-9\-_.!~*'()]/.test(String.fromCharCode(b))); };
const path = (r, values) => r.pattern.split("/").map(s => s[0] === ":" ? segmentOf(values.shift() ?? "") : s).join("/");
function chain(location) {
  const url = canonical(location), m = matchRoute(url);
  if (!m) return [];
  const [index, params] = m, root = rootFor(index);
  if (root == null) return [];
  const idx = [index];
  if (!Routes[index].notfound) for (let p = Routes[index].parent; idx.at(-1) !== root && p != null && p >= 0; p = Routes[p].parent) { if (idx.includes(p)) return []; idx.push(p); }
  if (!idx.includes(root)) idx.push(root);
  return idx.reverse().map(i => {
    const r = Routes[i], own = empty();
    for (const n of names(r.pattern)) own[Names.indexOf(n)] = params[Names.indexOf(n)];
    return { name: r.name, url: i === index ? url : canonical(path(r, names(r.pattern).map(n => params[Names.indexOf(n)]))), tab: Routes[root].name, params: own };
  });
}
const entry = (id, d, fragment = "") => [id, d.name, d.url, d.tab, d.params, fragment];
function refuse(r, why) { say(`router: ${why}`); return r; }
function mint(r, d, fragment) { const id = r[2]; r[2] = id + 1; return entry(id, d, fragment); }
/** The visit `e` with the fragment a location named: `e` itself when it already has it (LLP 1153 D5). */
const arrive = (e, fragment) => e[5] === fragment ? e : [...e.slice(0, 5), fragment];
const sel = r => r[1].findIndex(t => t[0] === r[0] && t[1].length);
const copy = r => [r[0], r[1].map(t => [t[0], t[1].slice()]), r[2]];
/** A launch: the location's chain, else, when no route matches it and no `notfound` route absorbs it (an app with no
 * navigation root, which the compiler writes none for: LLP 1116.009 D2), the first screen's (`/`), the refusal journaled;
 * the runner's `RouterContext::launch` (runner/src/runner/router.rs). The app boots, and its first commit puts `/` in the
 * address bar. */
export function launch(location) {
  if (!chain(location).length) { say(`router launch refused: no route matches ${canonical(location)}`); location = "/"; }
  return open_(["", [], 0], location);
}
/** A reload's router carry: keep only a stack the new table still describes; otherwise launch its old top. */
export function carryRouter(old, location) { const tabs = roots().map(i => Routes[i].name), stacks = old?.[1], valid = Array.isArray(stacks) && stacks.length === tabs.length && stacks.every((t, i) => t?.[0] === tabs[i] && Array.isArray(t[1]) && t[1].every(e => { const m = e?.length === 6 && matchRoute(e[2] ?? ""); return m && Routes[m[0]].name === e[1]; })); if (!valid) return launch(stacks?.find(t => t?.[0] === old?.[0])?.[1]?.at(-1)?.[2] ?? location); const out = copy(old); for (const t of out[1]) for (const e of t[1]) e[4] = matchRoute(e[2])[1]; return out; }
function open_(r, location) {
  const c = chain(location);
  if (!c.length) return refuse(r, `no route matches ${canonical(location)}`);
  const out = copy(r);
  if (!out[1].length) for (const i of roots()) out[1].push([Routes[i].name, [mint(out, { name: Routes[i].name, url: canonical(Routes[i].pattern), tab: Routes[i].name, params: empty() })]]);
  const t = out[1].find(t => t[0] === c[0].tab), leaf = c.length - 1, fragment = fragmentOf(location);
  if (!t) return refuse(r, `unknown tab ${c[0].tab}`);
  // The leaf takes the location's fragment; an ancestor kept keeps its own.
  t[1] = c.map((d, k) => t[1][k]?.[2] === d.url ? entry(t[1][k][0], d, k === leaf ? fragment : t[1][k][5]) : mint(out, d, k === leaf ? fragment : ""));
  out[0] = c[0].tab;
  return out;
}
function dest(r, location) { const url = canonical(location), m = matchRoute(url); return m && { name: Routes[m[0]].name, params: m[1], url, tab: r[0] }; }
function push_(r, location) {
  if (!r[1].length) return open_(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  const top = r[1][i][1].at(-1), fragment = fragmentOf(location);
  if (top?.[2] === d.url) { // the location on top: no new visit, a new fragment on it (route/src/router.rs)
    if (top[5] === fragment) return r;
    const out = copy(r), s = out[1][i][1]; s[s.length - 1] = arrive(top, fragment); return out;
  }
  const out = copy(r); out[1][i][1].push(mint(out, d, fragment)); return out;
}
function replace_(r, location) {
  if (!r[1].length) return open_(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  if (r[1][i][1].length === 1 && d.name !== r[1][i][0]) return refuse(r, "replace cannot change the tab's root route");
  const out = copy(r), s = out[1][i][1]; s[s.length - 1] = entry(s.at(-1)[0], d, fragmentOf(location)); return out;
}
function back_(r) { const i = sel(r); if (i < 0 || r[1][i][1].length < 2) return r; const out = copy(r); out[1][i][1].pop(); return out; }
function select_(r, name) {
  const i = r[1].findIndex(t => t[0] === name && t[1].length);
  if (i < 0) return refuse(r, `unknown tab ${name}`);
  const out = copy(r); if (r[0] === name) out[1][i][1].length = 1; out[0] = name; return out;
}
function go_(r, location) {
  const url = canonical(location), fragment = fragmentOf(location);
  if (!matchRoute(url)) return refuse(r, `no route matches ${url}`);
  const s = stack_(r), at = s.map(e => e[2]).lastIndexOf(url);
  if (at === s.length - 1 && s[at][5] === fragment) return r;
  if (at >= 0) { const i = sel(r); if (i < 0) return r; const out = copy(r), t = out[1][i][1]; t.length = at + 1; t[at] = arrive(t[at], fragment); return out; }
  const other = r[1].findIndex(t => t[0] !== r[0] && t[1].at(-1)?.[2] === url);
  if (other < 0) return push_(r, location);
  const out = copy(r), t = out[1][other][1]; t[t.length - 1] = arrive(t.at(-1), fragment); return select_(out, out[1][other][0]);
}
const stack_ = r => r[1].find(t => t[0] === r[0])?.[1] ?? [];
const top_ = r => stack_(r).at(-1) ?? [0, "", "", "", empty(), ""];
const depth_ = r => stack_(r).length;
const params_ = (r, name) => stack_(r).map(e => e[4][Names.indexOf(name)]).filter(v => v);
/** `fragment(entry)` (LLP 1153 D5): reads any entry, as `searchParam` does. */
export const x_fragment = e => e[5];
export function x_searchParam(e, name) {
  const q = e[2].split("?")[1]; if (!q) return "";
  for (const pair of q.split("#")[0].split("&").filter(Boolean)) { const [k, ...v] = pair.split("="); if (dec(k, true) === name) return dec(v.join("="), true); }
  return "";
}
// A received share's files by token (LLP 1133.024 D3; runner/src/runner/router.rs `share`): a token not set this launch answers none.
const Shared = new Map(); let SharedSet = null; const sharedSet = () => SharedSet ??= fact(); // made at first use: rt.js imports this module before its own body runs
export function x_sharedFiles(e, name) {
  sharedSet().get();
  const token = x_searchParam(e, name), files = Shared.get(token);
  if (!files && token) say(`router sharedFiles refused: no share under \`${name}\` this launch`);
  return files ?? [[], 0, 0];
}
/** What the host copied of a share so far: `files` Picked records, then the counts. */
export function setShared(token, files, dropped, pending) {
  say(`share: ${files.length} files set, ${pending} pending, ${dropped} dropped`);
  Shared.set(token, [files, dropped, pending]); commit(() => sharedSet().bump(), "shared");
}
export { segmentOf }; // `encodeRouteSegment` is budget.js's, checked
export const x_path = (name, ...values) => path(Routes.find(r => r.name === name && !r.notfound), values);
/** The router slot's changes, to the browser's history (`navigation.js`,
 * the web host's own), and a popstate back as the navigation root's
 * `navigate` (LLP 1038 D7, D11). */
let RouterSlot = null, Shown = null, Navigate = null, History = null; export const pageHistory = () => History; // the page's navigation.js, which the agent observes: its own copy's state is never written
/** The plan's navigation roots, with a router or without (document.js `projectRoots`). A page with one loads the input
 * piece after first paint: the focus follows its routes (input-glue.js `arrive`; LLP 1115.000 D4.2). */
export function navigationRoots(history) { History = history; projectRoots(history, location => Navigate ? (Navigate(location), true) : false, say, After, hostBack, hostGo, href => { const match = matchRoute(canonical(href)); return match !== null && !Routes[match[0]].notfound; }); pressFeedback(); }
/** The platform's own Back from the selected visit `id`, with no `navigate` handler (LLP 1115 D5): the router's `back`
 * as a commit of its own, the runner's `host_back`. */
function hostBack(id) { const r = RouterSlot?.n.v; if (r && validRouter(r) && top_(r)[0] === id && depth_(r) > 1) commit(() => W(RouterSlot, back_(r)), "host back"); }
/** The platform's own navigation to `location`, with no `navigate` handler (LLP 1115.000 D4.3: the browser's Forward, a
 * link to a declared route): the router's `go` as a commit of its own, the runner's `host_go`; nothing when `go` leaves
 * the router as it is. */
function hostGo(location) {
  const r = RouterSlot?.n.v; if (!r || !validRouter(r)) return;
  const next = go_(r, location); if (next !== r) commit(() => W(RouterSlot, next), "host go");
}
// @ref LLP 1153 D5 — the scroll to a fragment, the browser's own step done once the route's content has arrived: no
// resource's request in flight and none waiting for its source (the runner's `fragment_command`; an image or an
// observer's first report is no content). `fragment.js` finds and scrolls; it loads at first use.
let Due = null, Fragments = null;
const fragments = () => Fragments ??= import("./fragment.js");
const pageRoot = () => document.getElementById("exact-root");
function fragmentDue() {
  if (!Due || Resources.some(r => r.waiting || r.ticket?.waiting)) return;
  const { visit, fragment } = Due; Due = null;
  fragments().then(f => f.scroll(pageRoot(), visit, fragment, say), e => say(`fragment: ${e.message}`));
}
function wantScroll(op) { Due = { visit: op.top, fragment: op.fragment }; fragments().then(f => f.arm(pageRoot(), op)); queueMicrotask(fragmentDue); }
export function router(slot, history) {
  RouterSlot = slot; navigationRoots(history); After.push(fragmentDue);
  // An author's own `scrollIntoView` wins over a fragment's scroll still waiting.
  const own = Hosts.scrollIntoView; Hosts.scrollIntoView = (...a) => { if (Due) { say(`fragment #${Due.fragment}: the app's own scrollIntoView took its place`); Due = null; } return own(...a); };
  // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
  // route stays in this document, as input-glue.js's rule for the wasm host:
  // a link with its own `press` navigates by it; any other goes to the
  // root's `navigate` handler, as popstate does, and with no handler is the
  // router's own `go` (`hostGo`). A link to a fragment is a location
  // too (LLP 1153 D5): `href="#moves"` is the visit shown, read from the
  // attribute, since the shell's `<base href="/">` resolves it to `/`; the
  // same fragment again scrolls again, as the browser's does. A modified
  // click, a `target` or `download`, another origin or an undeclared path
  // (a file) is the browser's alone.
  document.addEventListener("click", ev => {
    const a = ev.target.closest?.("a[href]"), root = document.getElementById("exact-root");
    if (!a || !root?.contains(a) || ev.defaultPrevented) return;
    const press = (a.dataset.exactOn ?? "").split(" ").includes("press");
    if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey || (a.target && a.target !== "_self") || a.hasAttribute("download")) { if (press) ev.stopPropagation(); return; }
    const r = RouterSlot.n?.v, shown = r && validRouter(r) ? top_(r) : null, href = a.getAttribute("href") ?? "";
    const url = new URL(href[0] === "#" && shown ? shown[2] + href : a.href, location.href), to = url.pathname + url.search, m = matchRoute(canonical(to));
    const here = to === location.pathname + location.search || to === globalThis.history?.state?.url?.split("#")[0] || canonical(to) === shown?.[2];
    if (url.origin !== location.origin || !m || Routes[m[0]].notfound) return;
    ev.preventDefault();
    if (press || (here && !url.hash && !shown?.[5])) return;
    const target = to + url.hash;
    if (Navigate) Navigate(target); else hostGo(target);
    if (RouterSlot.n?.v !== r) return;
    if (here && url.hash) wantScroll({ top: shown[0], fragment: shown[5], scroll: true }); else say(`history: link ${JSON.stringify(target)} refused`);
  }, true);
  effect(() => {
    const r = slot(); if (!r || !r[1].length) return;
    const top = top_(r), ids = new Set(r[1].flatMap(t => t[1].map(e => e[0]))), before = Shown?.[1].flatMap(t => t[1]) ?? [];
    const removed = before.map(e => e[0]).filter(id => !ids.has(id)), was = before.find(e => e[0] === top[0]);
    Shown = r;
    // A new visit with a fragment, or a fragment that changed, scrolls (LLP 1153 D5); a visit returned to keeps its
    // place. The history mirrors the visit's url with its fragment.
    const op = { top: top[0], url: top[5] ? `${top[2]}#${top[5]}` : top[2], fragment: top[5], scroll: was ? was[5] !== top[5] : !!top[5], removed, visits: r[1].flatMap(t => t[1].map(e => [e[0], e[2]])) };
    history.apply(op);
    if (op.scroll) wantScroll(op);
  });
  // `effect` runs once synchronously. Its first apply follows
  // `projectRoots`' projection, so publish those initial visits now.
  history.project(document.getElementById("exact-root"), say);
}
/** `location` (a path and query) routed in place as a link to it is: the navigation root's `navigate` handler, else the
 * router's own `go` (the development page's `navigate` command, LLP 1122 T7). Why not, or "". */
export function goTo(location) {
  const m = matchRoute(canonical(location)), said = JSON.stringify(location.length > 200 ? `${location.slice(0, 200)}…` : location);
  if (m === null || Routes[m[0]].notfound) return `${said} is no route of this app`;
  const before = RouterSlot?.n?.v;
  if (Navigate) Navigate(location); else hostGo(location);
  return RouterSlot?.n?.v === before ? `the app's router took no step to ${said}` : "";
}
export const navigateTo = f => { Navigate = f; }, navigateRoot = location => Navigate ? (Navigate(location), true) : false; // the agent's `type <root> <location>` (LLP 1038 D11)
export const routeAt = location => matchRoute(canonical(location))?.[0] ?? -1;
/** Whether `location` names a declared route, never only the notfound fallback: the runner's `route_matches`. */
export const routeMatches = location => { const m = matchRoute(canonical(location)); return m !== null && !Routes[m[0]].notfound; };
// ---------------------------------------------------------------- validity
const int = n => typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 9007199254740991, str = s => typeof s === "string";
/** Whether `r` is a valid router (runner/src/runner/router.rs `router`): its shape; tabs of distinct names, each stack
 * starting at its tab's own route; entries of distinct ids below `next`, each URL canonical and matching the entry's
 * route name and params; and a top on the selected tab. A source can answer a forged one. */
export function validRouter(r) {
  if (!Array.isArray(r) || r.length !== 3 || !str(r[0]) || !Array.isArray(r[1]) || !int(r[2])) return false;
  const tabs = new Set(), ids = new Set();
  for (const t of r[1]) {
    if (!Array.isArray(t) || t.length !== 2 || !str(t[0]) || !Array.isArray(t[1]) || tabs.has(t[0]) || t[1][0]?.[1] !== t[0]) return false;
    tabs.add(t[0]);
    for (const e of t[1]) {
      if (!Array.isArray(e) || e.length !== 6 || !int(e[0]) || !str(e[1]) || !str(e[2]) || !str(e[3]) || !Array.isArray(e[4]) || e[4].length !== Names.length || !e[4].every(str) || !str(e[5])) return false;
      if (e[3] !== t[0] || e[0] >= r[2] || ids.has(e[0]) || canonical(e[2]) !== e[2]) return false;
      ids.add(e[0]);
      const m = matchRoute(e[2]);
      if (!m || Routes[m[0]].name !== e[1] || m[1].some((v, k) => v !== e[4][k])) return false;
    }
  }
  return stack_(r).length > 0;
}
/** At each commit (rt.js), before settlement: the router slot holds a valid router, or the commit is refused, as the
 * runner's `router_change`. The last value found valid is remembered. */
let Valid = null;
export function routerValid() {
  const v = RouterSlot?.n.v;
  if (!RouterSlot || v === Valid) return true;
  if (!validRouter(v)) return false;
  Valid = v; return true;
}
/** A verb or read of an invalid router traps, as the runner's (its `call_verb`); `searchParam` reads any entry. */
const checked = f => (r, ...a) => { if (!validRouter(r)) throw new Refusal("a router verb or read of an invalid router"); return f(r, ...a); };
export const x_open = checked(open_), x_push = checked(push_), x_replace = checked(replace_), x_back = checked(back_), x_select = checked(select_), x_go = checked(go_);
export const x_stack = checked(stack_), x_top = checked(top_), x_depth = checked(depth_), x_params = checked(params_);
