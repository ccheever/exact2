// The router on the JS target (LLP 1038; route/src), re-exported by rt.js: the verbs and reads, the route table, and the
// router slot's changes to the browser's history.
import { Refusal, effect, After, journal, clock } from "./rt.js";
import { projectRoots } from "./document.js";
const say = line => journal.push(`t=${clock.now} ${line}`);
// A Router is [tab, tabs, next]; a Tab [name, stack]; an Entry
// [id, name, url, tab, params], params positional in the table's
// first-declaration order of distinct `:names`.
let Routes = [], Names = [];
const names = p => p.split("/").filter(s => s[0] === ":").map(s => s.slice(1));
/** The plan's route table: [name, pattern, parent, tab, notfound] rows. */
export function routes(table) {
  Routes = table.map(([name, pattern, parent, tab, notfound]) => ({ name, pattern, parent, tab, notfound }));
  Names = [];
  for (const r of Routes) if (!r.notfound) for (const n of names(r.pattern)) if (!Names.includes(n)) Names.push(n);
}
const HEX = "0123456789ABCDEF", utf8 = new TextEncoder();
const enc = (s, esc) => { let o = ""; for (const b of utf8.encode(s)) o += esc(b) ? "%" + HEX[b >> 4] + HEX[b & 15] : String.fromCharCode(b); return o; };
const dec = (s, plus) => { const out = []; for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i); if (c === 37 && /^[0-9a-f]{2}$/i.test(s.substr(i + 1, 2))) { out.push(parseInt(s.substr(i + 1, 2), 16)); i += 2; } else if (plus && c === 43) out.push(32); else out.push(...utf8.encode(s[i])); } return new TextDecoder().decode(new Uint8Array(out)); };
const clean = s => s.replace(/^[\0- ]+|[\0- ]+$/g, "").replace(/[\t\n\r]/g, "");
/** `canonical` (route/src/location.rs): path and query, dot segments resolved, escaped. */
export function canonical(location) {
  let input = clean(location[0] === "/" ? location : "/" + location).split("#")[0];
  let [path, ...q] = input.split("?"); const query = q.join("?");
  path = path.replace(/\\/g, "/").replace(/^\//, "");
  const segs = [], parts = path.split("/");
  parts.forEach((seg, i) => {
    const d = seg.toLowerCase(), last = i === parts.length - 1;
    if (d === "." || d === "%2e") { if (last) segs.push(""); }
    else if (["..", ".%2e", "%2e.", "%2e%2e"].includes(d)) { segs.pop(); if (last) segs.push(""); }
    else segs.push(enc(seg, b => b < 0x21 || b > 0x7e || '"#<>?^`{}|'.includes(String.fromCharCode(b))));
  });
  return "/" + segs.join("/") + (query ? "?" + enc(query, b => b < 0x21 || b > 0x7e || "\"#<>'".includes(String.fromCharCode(b))) : "");
}
const empty = () => Names.map(() => "");
const segments = p => p === "/" ? [] : p.replace(/^\//, "").split("/");
function matchRoute(url) {
  const path = url.split("?")[0], parts = segments(path);
  if (path === "/" || !path.endsWith("/")) for (let i = 0; i < Routes.length; i++) {
    const r = Routes[i]; if (r.notfound) continue;
    const pat = segments(canonical(r.pattern)); if (pat.length !== parts.length) continue;
    const params = empty();
    if (pat.every((s, k) => s[0] === ":" ? parts[k] !== "" && (params[Names.indexOf(s.slice(1))] = dec(parts[k], false), true) : s === parts[k])) return [i, params];
  }
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
const entry = (id, d) => [id, d.name, d.url, d.tab, d.params];
function refuse(r, why) { say(`router: ${why}`); return r; }
function mint(r, d) { const id = r[2]; r[2] = id + 1; return entry(id, d); }
const sel = r => r[1].findIndex(t => t[0] === r[0] && t[1].length);
const copy = r => [r[0], r[1].map(t => [t[0], t[1].slice()]), r[2]];
export const launch = location => open_(["", [], 0], location);
/** A reload's router carry: keep only a stack the new table still describes; otherwise launch its old top. */
export function carryRouter(old, location) { const tabs = roots().map(i => Routes[i].name), stacks = old?.[1], valid = Array.isArray(stacks) && stacks.length === tabs.length && stacks.every((t, i) => t?.[0] === tabs[i] && Array.isArray(t[1]) && t[1].every(e => { const m = matchRoute(e?.[2] ?? ""); return m && Routes[m[0]].name === e[1]; })); if (!valid) return launch(stacks?.find(t => t?.[0] === old?.[0])?.[1]?.at(-1)?.[2] ?? location); const out = copy(old); for (const t of out[1]) for (const e of t[1]) e[4] = matchRoute(e[2])[1]; return out; }
function open_(r, location) {
  const c = chain(location);
  if (!c.length) return refuse(r, `no route matches ${canonical(location)}`);
  const out = copy(r);
  if (!out[1].length) for (const i of roots()) out[1].push([Routes[i].name, [mint(out, { name: Routes[i].name, url: canonical(Routes[i].pattern), tab: Routes[i].name, params: empty() })]]);
  const t = out[1].find(t => t[0] === c[0].tab);
  if (!t) return refuse(r, `unknown tab ${c[0].tab}`);
  t[1] = c.map((d, k) => t[1][k]?.[2] === d.url ? entry(t[1][k][0], d) : mint(out, d));
  out[0] = c[0].tab;
  return out;
}
function dest(r, location) { const url = canonical(location), m = matchRoute(url); return m && { name: Routes[m[0]].name, params: m[1], url, tab: r[0] }; }
function push_(r, location) {
  if (!r[1].length) return open_(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  if (r[1][i][1].at(-1)?.[2] === d.url) return r; // the location on top: no new visit (route/src/router.rs)
  const out = copy(r); out[1][i][1].push(mint(out, d)); return out;
}
function replace_(r, location) {
  if (!r[1].length) return open_(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  if (r[1][i][1].length === 1 && d.name !== r[1][i][0]) return refuse(r, "replace cannot change the tab's root route");
  const out = copy(r), s = out[1][i][1]; s[s.length - 1] = entry(s.at(-1)[0], d); return out;
}
function back_(r) { const i = sel(r); if (i < 0 || r[1][i][1].length < 2) return r; const out = copy(r); out[1][i][1].pop(); return out; }
function select_(r, name) {
  const i = r[1].findIndex(t => t[0] === name && t[1].length);
  if (i < 0) return refuse(r, `unknown tab ${name}`);
  const out = copy(r); if (r[0] === name) out[1][i][1].length = 1; out[0] = name; return out;
}
function go_(r, location) {
  const url = canonical(location);
  if (!matchRoute(url)) return refuse(r, `no route matches ${url}`);
  const s = stack_(r), at = s.map(e => e[2]).lastIndexOf(url);
  if (at >= 0) { const i = sel(r); if (i < 0) return r; const out = copy(r); out[1][i][1].length = at + 1; return out; }
  const other = r[1].find(t => t[0] !== r[0] && t[1].at(-1)?.[2] === url);
  return other ? select_(r, other[0]) : push_(r, location);
}
const stack_ = r => r[1].find(t => t[0] === r[0])?.[1] ?? [];
const top_ = r => stack_(r).at(-1) ?? [0, "", "", "", empty()];
const depth_ = r => stack_(r).length;
const params_ = (r, name) => stack_(r).map(e => e[4][Names.indexOf(name)]).filter(v => v);
export function x_searchParam(e, name) {
  const q = e[2].split("?")[1]; if (!q) return "";
  for (const pair of q.split("#")[0].split("&").filter(Boolean)) { const [k, ...v] = pair.split("="); if (dec(k, true) === name) return dec(v.join("="), true); }
  return "";
}
export { segmentOf }; // `encodeRouteSegment` is budget.js's, checked
export const x_path = (name, ...values) => path(Routes.find(r => r.name === name && !r.notfound), values);
/** The router slot's changes, to the browser's history (`navigation.js`,
 * the web host's own), and a popstate back as the navigation root's
 * `navigate` (LLP 1038 D7, D11). */
let RouterSlot = null, Shown = null, Navigate = null, History = null; export const pageHistory = () => History; // the page's navigation.js, which the agent observes: its own copy's state is never written
/** The plan's navigation roots, with a router or without (document.js `projectRoots`). */
export function navigationRoots(history) { History = history; projectRoots(history, location => Navigate?.(location), say, After); }
export function router(slot, history) {
  RouterSlot = slot; navigationRoots(history);
  // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
  // route stays in this document, as input-glue.js's rule for the wasm host:
  // a link with its own `press` navigates by it; any other goes to the
  // root's `navigate` handler, as popstate does. A modified click, a
  // `target` or `download`, another origin, this page's fragment or an
  // undeclared path (a file) is the browser's alone.
  document.addEventListener("click", ev => {
    const a = ev.target.closest?.("a[href]"), root = document.getElementById("exact-root");
    if (!a || !root?.contains(a) || ev.defaultPrevented) return;
    const press = (a.dataset.exactOn ?? "").split(" ").includes("press");
    if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey || (a.target && a.target !== "_self") || a.hasAttribute("download")) { if (press) ev.stopPropagation(); return; }
    const url = new URL(a.href), to = url.pathname + url.search, here = to === location.pathname + location.search || to === globalThis.history?.state?.url, m = matchRoute(canonical(to));
    if (url.origin !== location.origin || (here && url.hash) || !m || Routes[m[0]].notfound) return;
    if (!press && !Navigate) return;
    ev.preventDefault();
    if (press || here) return;
    const before = RouterSlot.n?.v; Navigate(to); if (RouterSlot.n?.v === before) say(`history: link ${JSON.stringify(to)} refused`);
  }, true);
  effect(() => {
    const r = slot(); if (!r || !r[1].length) return;
    const top = top_(r), ids = new Set(r[1].flatMap(t => t[1].map(e => e[0])));
    const removed = Shown ? Shown[1].flatMap(t => t[1].map(e => e[0])).filter(id => !ids.has(id)) : [];
    Shown = r;
    history.apply({ top: top[0], url: top[2], removed });
  });
}
export const navigateTo = f => { Navigate = f; }, navigateRoot = location => Navigate ? (Navigate(location), true) : false; // the agent's `type <root> <location>` (LLP 1038 D11)
export const routeAt = location => matchRoute(canonical(location))?.[0] ?? -1;
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
      if (!Array.isArray(e) || e.length !== 5 || !int(e[0]) || !str(e[1]) || !str(e[2]) || !str(e[3]) || !Array.isArray(e[4]) || e[4].length !== Names.length || !e[4].every(str)) return false;
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
