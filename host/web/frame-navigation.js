// An iframe's navigation on the web (@ref LLP 1020.002 D2, D3, §11): the
// subset a page can see of a frame. Both web targets load it when a plan
// hears a frame's navigation events or sends it a frame command.
//
// - A same-origin guest: each guest document is watched from its `load`, so
//   a new document's own earlier events are not seen. Its `navigation`
//   (the Navigation API) gives `navigate`, `currententrychange`,
//   `navigatesuccess` and `navigateerror` for its same-document navigations,
//   and `navigate` for a cross-document one, which the next document's
//   `load` commits and ends. A title or icon change with no navigation is a
//   `currententrychange`.
// - A browser without the Navigation API: `load` commits and ends a
//   navigation, and `hashchange` and `popstate` are same-document ones;
//   `canGoBack` and `canGoForward` are false, and `traverseHistory` cannot
//   tell a missing entry.
// - A cross-origin guest: `load` only, no records; of the commands only
//   `navigate`, which a parent may do to its frame's `location`.
// - `partition` is never honoured (a browser partitions by top-level site),
//   so every record says `partitioned: false`.
import { frameURL } from './navigation.js';

/** `FrameNavigation`'s fields in the contract's order: the JS target passes
 * a record to an action as this positional array. */
export const FIELDS = ['url', 'destination', 'navigationId', 'navigationType', 'title', 'icons', 'canGoBack', 'canGoForward', 'loading',
  'partitioned', 'controller', 'error', 'message', 'recovering'];
export const EVENTS = new Set(['navigate', 'currententrychange', 'navigatesuccess', 'navigateerror']);
export const positional = record => FIELDS.map(k => record[k]);

/** The guest's window and document when the page may read them, else null. */
function guest(e) {
  try { const doc = e.contentDocument; return doc && e.contentWindow ? { win: e.contentWindow, doc } : null; } catch { return null; }
}

function icons(doc) {
  const out = [...doc.querySelectorAll('link[rel~="icon" i]')].map(l => l.href).filter(Boolean);
  try { const u = new URL(doc.URL); if (u.protocol === 'http:' || u.protocol === 'https:') { const f = `${u.origin}/favicon.ico`; if (!out.includes(f)) out.push(f); } } catch {}
  return out;
}

/** The frame's navigation state, kept on the element. */
function state(e) {
  return e.exactNavigation ??= { listeners: new Set(), nextId: 0, lastId: 0, lastType: 'push', flight: null, committed: '', watched: null, last: null, sent: null };
}

function record(e, { ending = null, error = '', message = '' } = {}) {
  const s = state(e), g = guest(e), nav = g?.win.navigation, flight = ending ?? s.flight;
  return {
    url: s.committed,
    destination: ending ? '' : flight && !flight.committed ? flight.destination : '',
    navigationId: flight?.id ?? s.lastId,
    navigationType: flight?.type ?? s.lastType,
    title: g?.doc.title ?? '',
    icons: g ? icons(g.doc) : [],
    canGoBack: !!nav?.canGoBack,
    canGoForward: !!nav?.canGoForward,
    loading: !ending && !!flight,
    partitioned: false,
    controller: 'person',
    error, message,
    recovering: false,
  };
}

const key = r => JSON.stringify([r.url, r.title, r.icons, r.canGoBack, r.canGoForward]);

function send(e, name, options) {
  const s = state(e), r = record(e, options);
  s.last = r;
  if (name === 'currententrychange' || name === 'navigatesuccess') s.sent = key(r);
  for (const l of s.listeners) l(name, r);
}

function begin(e, destination, type) {
  const s = state(e);
  if (s.flight) { const old = s.flight; s.flight = null; send(e, 'navigateerror', { ending: old, error: 'AbortError', message: 'superseded by a new navigation' }); }
  s.flight = { id: ++s.nextId, destination, type, committed: false };
  s.lastId = s.flight.id; s.lastType = type;
  send(e, 'navigate');
}

function end(e, name, error = '', message = '') {
  const s = state(e), f = s.flight;
  if (!f) return;
  s.flight = null;
  send(e, name, { ending: f, error, message });
}

/** A title, icon or history change with no navigation in flight. */
function changed(e) {
  const s = state(e);
  if (s.flight || !s.committed || key(record(e)) === s.sent) return;
  send(e, 'currententrychange');
}

/** The document now in the frame: commit and end the navigation its load
 * finished, then watch its own navigations. */
function loaded(e) {
  const s = state(e), g = guest(e);
  if (!g) {
    // Another origin's document: no record of it, but a navigation this frame announced still ends.
    s.watched = null;
    if (s.flight) { s.committed = s.flight.destination; s.flight.committed = true; end(e, 'navigatesuccess'); }
    return;
  }
  if (s.watched === g.doc) return;
  s.watched = g.doc;
  if (!s.flight) { s.flight = { id: ++s.nextId, destination: g.doc.URL, type: 'push', committed: false }; s.lastId = s.flight.id; s.lastType = 'push'; }
  s.committed = g.doc.URL;
  s.flight.committed = true;
  send(e, 'currententrychange');
  end(e, 'navigatesuccess');
  const { win, doc } = g, nav = win.navigation, here = () => s.watched === doc;
  if (nav) {
    nav.addEventListener('navigate', ev => {
      if (!here()) return;
      begin(e, ev.destination.url, ev.navigationType); // a cross-document one is committed by the next document's `load`
    });
    nav.addEventListener('currententrychange', () => {
      if (!here() || !nav.currentEntry || nav.currentEntry.url === s.committed && !s.flight) return;
      s.committed = nav.currentEntry.url;
      if (!s.flight) { s.flight = { id: ++s.nextId, destination: s.committed, type: 'traverse', committed: false }; s.lastId = s.flight.id; s.lastType = 'traverse'; }
      s.flight.committed = true;
      send(e, 'currententrychange');
      // A same-document traversal or replace may finish with no `navigatesuccess` of its own (no intercept).
      queueMicrotask(() => { if (here() && s.flight?.committed && s.flight.destination === s.committed) end(e, 'navigatesuccess'); });
    });
    nav.addEventListener('navigatesuccess', () => { if (here()) end(e, 'navigatesuccess'); });
    nav.addEventListener('navigateerror', ev => { if (here()) end(e, 'navigateerror', ev.error?.name ?? 'AbortError', String(ev.message ?? ev.error?.message ?? '')); });
  } else {
    const same = type => { if (!here() || doc.URL === s.committed) return; begin(e, doc.URL, type); s.committed = doc.URL; s.flight.committed = true; send(e, 'currententrychange'); end(e, 'navigatesuccess'); };
    win.addEventListener('hashchange', () => same('push'));
    win.addEventListener('popstate', () => same('traverse'));
  }
  try { new win.MutationObserver(() => { if (here()) changed(e); }).observe(doc.head ?? doc.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['href', 'rel'] }); } catch {}
}

/** Hear `e`'s navigation records (`deliver(name, record)`). A document that
 * already loaded is taken as it stands. */
export function watch(e, deliver) {
  const s = state(e);
  s.listeners.add(deliver);
  if (!s.load) {
    s.load = () => loaded(e);
    e.addEventListener('load', s.load);
    const g = guest(e);
    if (g && g.doc.readyState === 'complete' && g.doc.URL !== 'about:blank') loaded(e);
  }
  return () => s.listeners.delete(deliver);
}

/** The last record a frame reported, for the agent's `tree`. */
export const last = e => e.exactNavigation?.last ?? null;

/** Refused: a `navigateerror` to whoever hears the frame, and the journal. */
function refuse(e, error, message, say) {
  say(message);
  send(e, 'navigateerror', { error, message });
}

/** `navigate(id, url)`, `traverseHistory(id, delta)`, `reload(id,
 * ignoreCache)` and `stopLoading(id)` (@ref LLP 1020.002 D3), to the one
 * live iframe with that id. A URL takes `src`'s policy (`frameURL`). */
export function command(name, args, say, live = [...document.querySelectorAll('#exact-root iframe')]) {
  const id = args[0], frames = live.filter(f => f.isConnected && !f.closest('[data-exiting]') && f.id === id);
  if (frames.length !== 1) return say(`${name}: no unique live iframe with id "${id}"`);
  const e = frames[0], g = guest(e), nav = g?.win.navigation;
  if (name === 'navigate') {
    const url = args[1];
    let href;
    try { href = new URL(url, document.baseURI).href; } catch { href = null; }
    if (href == null || !frameURL(url, e.getAttribute('sandbox'))) return refuse(e, 'NotSupportedError', `navigate: ${JSON.stringify(String(url).slice(0, 80))} is not a URL a frame may show`, say);
    // The location, not `navigation.navigate()`: the guest's `navigate` event is the same, and Playwright's
    // WebKit (2370) crashes when a frame stops a navigation that `navigate()` began.
    try { (g ? g.win : e.contentWindow).location.href = href; } catch { e.src = href; }
    return;
  }
  if (!g) return refuse(e, 'NotSupportedError', `${name}: iframe "${id}" shows another origin; a page may only navigate such a frame`, say);
  if (name === 'traverseHistory') {
    const delta = Number(args[1]) || 0;
    if (delta === 0) return g.win.location.reload();
    if (!nav) return g.win.history.go(delta);
    const entries = nav.entries(), at = (nav.currentEntry?.index ?? -1) + delta;
    if (at < 0 || at >= entries.length) return refuse(e, 'InvalidStateError', `traverseHistory: no history entry at ${delta}`, say);
    return void nav.traverseTo(entries[at].key).finished.catch(() => {});
  }
  if (name === 'reload') {
    if (args[1] === true) return refuse(e, 'NotSupportedError', 'reload: ignoreCache: a page cannot reload a frame past its cache', say);
    return g.win.location.reload();
  }
  if (name === 'stopLoading') {
    // Chrome's Navigation API ends a stopped navigation with `navigateerror`; Firefox's and WebKit's fire
    // nothing, so a navigation still in flight ends here, as the Navigation API names a stopped one.
    g.win.stop();
    return end(e, 'navigateerror', 'AbortError', 'stopLoading');
  }
  say(`${name}: not a frame command`);
}

(globalThis.exact ??= {}).frameNavigation = { watch, command, last, positional };
