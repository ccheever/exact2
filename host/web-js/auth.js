// `openAuthSession` on the JS target (LLP 1069.006): the runner's rules
// (runner/src/auth.rs — the request's checks, the grants, one session per
// window and supersession, the callback's match, the agent's hold) for a
// TypeScript source running in the page. Outside the agent the web host's
// own `auth-glue.js` opens the popup, inside the press's call stack, and
// reports; under the agent the request is held (D7) and `tap @t cancel` or
// `type @t <callback URL>` answers it (agent.js). Bundled only for an app
// whose grants name `auth.session`.
import { journal, clock, nextTicket } from "./rt.js";

const say = line => journal.push(`t=${clock.now} ${line}`);
const CALLBACK = "/.exact/auth/callback";

// ---------------------------------------------------------------- URLs (auth.rs `Url`)
function parse(text) {
  const colon = text.indexOf(":");
  if (colon < 1 || !/^[A-Za-z][A-Za-z0-9+.-]*$/.test(text.slice(0, colon)) || /[\s\x00-\x1f\x7f]/.test(text)) return null;
  let rest = text.slice(colon + 1), fragment = null, query = null, authority = null;
  const hash = rest.indexOf("#"); if (hash >= 0) { fragment = rest.slice(hash + 1); rest = rest.slice(0, hash); }
  const q = rest.indexOf("?"); if (q >= 0) { query = rest.slice(q + 1); rest = rest.slice(0, q); }
  let path = rest;
  if (rest.startsWith("//")) { const after = rest.slice(2), end = after.indexOf("/") < 0 ? after.length : after.indexOf("/"); authority = after.slice(0, end); path = after.slice(end); if (!authority) return null; }
  else if (!(path.startsWith("/") && !path.startsWith("//"))) return null;
  return { scheme: text.slice(0, colon).toLowerCase(), authority, path, query, fragment };
}
const origin = u => u.authority == null ? null : `${u.scheme}://${u.authority.toLowerCase()}`;
const decode = s => { try { return decodeURIComponent(s.replace(/\+/g, " ")); } catch { return s.replace(/\+/g, " "); } };
const pairs = q => (q ?? "").split("&").filter(Boolean).map(p => { const i = p.indexOf("="); return i < 0 ? [decode(p), ""] : [decode(p.slice(0, i)), decode(p.slice(i + 1))]; });
const loopback = u => ["localhost", "127.0.0.1", "[::1]"].includes((u.authority ?? "").replace(/:\d+$/, ""));

/** `auth.session`'s grammar: an origin, or `scheme://*.domain` of two labels or more. */
function admits(grant, u) {
  const o = origin(u);
  if (!o) return false;
  const wild = grant.indexOf("://*.");
  if (wild >= 0) {
    const scheme = grant.slice(0, wild), domain = grant.slice(wild + 5).toLowerCase(), host = (u.authority ?? "").toLowerCase();
    return scheme.toLowerCase() === u.scheme && domain.split(".").length >= 2 && !/[:/*]/.test(domain) && host.length > domain.length + 1 && host.endsWith("." + domain);
  }
  return !grant.includes("*") && grant.replace(/\/+$/, "").toLowerCase() === o;
}

/** D2: the authorization URL on a granted `auth.session` origin; the callback
 * a granted `auth.callback`, or on a loopback page that page's own. */
function checkGrants(s, grants) {
  const lines = grants.split("\n").map(l => l.trim()).filter(Boolean);
  const u = parse(s.url);
  if (!u) return "the authorization URL is not absolute";
  if (!lines.some(l => l.startsWith("auth.session ") && admits(l.slice(13).trim(), u))) return `outside the app's grants (auth.session ${origin(u) ?? ""})`;
  const granted = lines.some(l => l.startsWith("auth.callback ") && l.slice(14).trim() === s.callback);
  const page = parse(location.origin);
  if (!granted && !(s.callback === location.origin + CALLBACK && page && loopback(page))) return `outside the app's grants (auth.callback ${s.callback})`;
  return null;
}

/** D3: scheme, host and path exactly the request's, no userinfo, `state`
 * exactly once and equal. The reason never carries the URL. */
function accept(s, got) {
  const want = parse(s.callback), u = parse(got);
  if (!want) return "the request's callback is unreadable";
  if (!u) return "the callback is not an absolute URL";
  if (u.scheme !== want.scheme) return "the callback's scheme is not the request's";
  if (u.authority?.includes("@")) return "the callback carries userinfo";
  if (u.authority?.toLowerCase() !== want.authority?.toLowerCase()) return "the callback's host is not the request's";
  if (u.path !== want.path) return "the callback's path is not the request's";
  const states = pairs(u.query).filter(([n]) => n === "state").map(([, v]) => v);
  return states.length === 1 ? (states[0] === s.state ? null : "the callback's state is not the request's") : states.length ? "the callback carries state more than once" : "the callback carries no state";
}

/** D7: the hold's summary — the URL's origin and path, its parameter
 * names, the values of `client_id` and `request_uri` only, the callback,
 * `state` and `ephemeral`. */
function summary(s) {
  const u = parse(s.url), out = { origin: u ? `${u.scheme}://${u.authority ?? ""}` : "", path: u?.path ?? "", params: [] };
  for (const [n, v] of pairs(u?.query)) { out.params.push(n); if (n === "client_id" || n === "request_uri") out[n] = v; }
  return Object.assign(out, { callback: s.callback, state: s.state, ephemeral: s.ephemeral });
}

// ---------------------------------------------------------------- sessions
/** Live sessions: { ticket, target, session, done(status, body), held }. */
const Live = [];
let Glue = null;
function settle(ticket, status, body) {
  const at = Live.findIndex(l => l.ticket === ticket);
  if (at < 0) return say(`auth ${ticket}: a late completion, dropped`);
  const [l] = Live.splice(at, 1);
  say(`auth ${ticket}: ${status === 200 ? "callback accepted" : status === 499 ? "cancelled" : `answered ${status}`}`);
  l.done(status, body);
}

/** Install `authCallback` and `openAuthSession` for the page's source.
 * `target()` names the resource or mutation whose answer is running. */
export function install(grants, target) {
  (globalThis.exact ??= {}).auth = { holds, answer };
  globalThis.authCallback = () => location.origin + CALLBACK;
  if (!clock.agent && typeof requestAnimationFrame === "function") {
    // The glue is in the page before any press: the popup opens in the press's own call stack.
    requestAnimationFrame(() => setTimeout(() => { Glue = import("./auth-glue.js").then(() => globalThis.exact.authHost); Glue.then(g => { Glue = g; }); }));
  }
  globalThis.openAuthSession = (url, o = {}) => new Promise((resolve, reject) => {
    if (typeof o.callback !== "string" || typeof o.state !== "string" || !o.state) return reject(new TypeError("openAuthSession(url, {callback, state}): callback is authCallback() and state a non-empty string"));
    const ticket = nextTicket(), name = target();
    const done = (status, body) => { if (status === 200) resolve(body); else { const e = new Error(body || "the auth session failed"); Object.defineProperty(e, "status", { value: status, enumerable: true }); reject(e); } };
    const refuse = (status, why) => { say(`auth ${ticket}: answered ${status}`); done(status, why); };
    const s = { url: String(url), callback: o.callback, state: o.state, ephemeral: !!o.ephemeral };
    const u = parse(s.url), cb = parse(s.callback);
    if (!u || !["https", "http"].includes(u.scheme) || u.authority == null) return refuse(502, u ? "the authorization URL is not http: or https:" : "the authorization URL is not absolute");
    if (!cb || cb.fragment != null || cb.query != null) return refuse(502, "the callback is not a scheme:/path or https://host/path");
    const denied = checkGrants(s, grants);
    if (denied) return refuse(403, denied);
    // One session per window: the same target's earlier one is superseded;
    // another target's live one refuses this (409).
    for (let i = Live.length - 1; i >= 0; i--) if (Live[i].target === name) { say(`auth ${Live[i].ticket}: cancelled by supersession`); Live[i].close?.(); Live.splice(i, 1); }
    if (Live.length) return refuse(409, "already open");
    const l = { ticket, target: name, session: s, done };
    Live.push(l);
    if (clock.agent) { l.held = true; say(`device auth ${ticket} held (agent)`); return; }
    if (!Glue || typeof Glue.arm !== "function") return settle(ticket, 501, "no system browser on this host");
    say(`auth ${ticket}: opened`);
    // The glue's words (auth-glue.js `arm`): the runner's `exact_auth`, here.
    let report = null;
    Glue.arm({ ticket }, {
      agent: false, log: say, active: t => Live.some(x => x.ticket === t),
      call: req => {
        if (req.op === "arm") return req.popup === false ? (report = [428, "popup blocked"], { settled: true }) : { present: { url: s.url, callback: s.callback } };
        if (req.op === "done") { const why = req.url != null ? accept(s, req.url) : null; report = req.url != null ? (why ? [502, why] : [200, req.url]) : [req.status, req.message ?? ""]; }
        return {};
      },
      deliver: t => { if (report) settle(t, ...report); },
    });
    l.close = () => {};
  });
}

/** Held requests for the agent's `state` (`pending`, after the network's). */
export const holds = () => Live.filter(l => l.held).map(l => ({ name: l.target, ticket: l.ticket, device: { capability: "auth", args: summary(l.session) } }));
/** The agent's answer (LLP 1069.007 D4): checked before the hold is spent. */
export function answer(req) {
  const l = Live.find(x => x.held && x.ticket === req.ticket);
  if (!l) return { error: `not pending: @${req.ticket}` };
  if (req.op === "tap") {
    if (req.choice !== "cancel") return { error: `@${req.ticket} (auth): tap takes cancel, not ${req.choice}` };
    say(`device auth ${req.ticket} cancelled`);
    settle(req.ticket, 499, "cancelled");
    return { ticket: req.ticket, capability: "auth", answered: "cancel", delivery: "substituted" };
  }
  const why = accept(l.session, String(req.text ?? ""));
  if (why) return { error: `@${req.ticket} (auth): ${why}` };
  say(`device auth ${req.ticket} answered: a callback`);
  settle(req.ticket, 200, String(req.text));
  return { ticket: req.ticket, capability: "auth", answered: "value", delivery: "substituted" };
}
