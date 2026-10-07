// A `video` on the JS target (LLP 1042): the web host's own media-glue.js,
// fetched after the first frame by a page with one (a loaded capability, as
// on the wasm host), applies its props — `paused`, `volume`, `currentTime`,
// `playbackRate`, `preservesPitch`, the visibility policy — and reports its
// events. This adapter hands it the node's props as the wasm host's
// `syncMedia` does (static ones from the element's attributes, by the
// plan's names) and hears its reports as `exact-media` events. A node the
// tree has ended is retired: its player stops and it reports nothing more,
// so a late `pause`, `timeupdate` or refused play never reaches whatever now
// holds its place (jukebox F1, F5, F6, F20).
import { onEnd, inflight, journal, data, clock } from "./rt.js";

// The media session's actions (LLP 1098 D2, D6): the glue sends them as it
// sends the element's events, with `seekOffset seekTime fastSeek`.
const SESSION = ["seekbackward", "seekforward", "seekto", "previoustrack", "nexttrack", "stop"];
const BOOL = new Set(["autoplay", "controls", "loop", "muted", "playsinline", "disablepictureinpicture", "disableremoteplayback"]);
export const MEDIA_EVENTS = new Set(["loadedmetadata", "durationchange", "timeupdate", "play", "playing", "pause", "ended", "waiting", "seeking", "seeked", "ratechange", "volumechange", "error", "canplay", "fullscreenchange", ...SESSION]);
let Glue = null, Install = null;
// Nodes whose props changed: handed to the glue together after the commit
// that built or changed them, when they are in the document.
const Dirty = new Set();
const send = e => text => e.dispatchEvent(new CustomEvent("exact-media", { detail: text }));
function flush() {
  const later = [];
  for (const e of Dirty) if (!e.$media.retired) { if (e.isConnected) { Install(e, send(e)); holdUntilPlayable(e); } else later.push(e); }
  Dirty.clear();
  if (later.length) requestAnimationFrame(() => { for (const e of later) install(e); });
}
// The wasm host does not count a load with no source, or one that stalls or
// empties, as in flight. An opening seek still holds, so `clock settle` is
// not taken between `seeking` and `seeked` (synthetic-media).
function mediaSource(e) {
  return e.currentSrc || (typeof e.getAttribute === "function" && e.getAttribute("src")) || e.exactMedia?.props?.src || "";
}
function holdUntilPlayable(e) {
  if (e.$media.hold || e.error || (e.readyState >= 3 && e.seeking !== true)) return;
  if (e.seeking !== true && !mediaSource(e)) return;
  e.$media.hold = true;
  inflight.n++;
  const names = ["seeked", "canplay", "error", "emptied", "stalled"];
  const release = () => { if (!e.$media.hold) return; e.$media.hold = false; inflight.n--; for (const n of names) e.removeEventListener(n, check); };
  const check = (ev) => {
    if (e.$media.retired || e.error) return release();
    const kind = ev && ev.type;
    if (kind === "emptied" || kind === "stalled") return release();
    if (e.seeking !== true && !mediaSource(e)) return release();
    if (e.readyState >= 3 && e.seeking !== true) release();
  };
  e.$media.release = release;
  for (const n of names) e.addEventListener(n, check);
}
function install(e) {
  if (e.$media.retired) return;
  if (!Dirty.size && Install) queueMicrotask(flush);
  Dirty.add(e);
  if (Glue) return;
  inflight.n++;
  Glue = new Promise(r => requestAnimationFrame(r)).then(() => { globalThis.exact ??= {}; return import("./media-glue.js"); })
    .then(() => { Install = globalThis.exact.installMedia; flush(); })
    .catch(err => journal.push(`media: unavailable: ${err.message}`)).finally(() => inflight.n--);
}
/** The glue on its way (the agent waits for it before an operation). */
export const mediaPiece = () => Glue;
/** A `video` built or adopted with its static props (`h`). */
export function media(e, attrs) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  const props = {};
  for (const k in attrs) if (!k.startsWith("data-")) props[k] = BOOL.has(k) ? "true" : attrs[k];
  e.$media = { retired: false, app: null, poster: attrs["data-app-poster"] ?? null, command: (name, seconds) => command(e, name, seconds) };
  e.exactMedia = { props, handlers: [] };
  if (attrs["data-app-src"]) appSource(e, attrs["data-app-src"]);
  if (attrs["data-app-poster"]) appPoster(e, attrs["data-app-poster"]);
  onEnd(() => {
    e.$media.retired = true;
    e.$media.release?.();
    globalThis.exact?.removeMedia?.(e);
    // As the wasm host retires a video: stopped, its source let go.
    e.pause(); e.removeAttribute("src"); e.load();
  });
  install(e);
}
/** A dynamic prop's value (`P`); true when it took the source itself (an `app:/` file). */
export function mediaProp(e, name, v) {
  if (!e.$media) return false;
  if (name === "src") {
    if (v?.startsWith("app:/")) { appSource(e, v); return true; }
    e.$media.app = null;
  }
  if (name === "poster") {
    e.$media.poster = v?.startsWith("app:/") ? v : null;
    if (e.$media.poster) { appPoster(e, v); return true; }
  }
  if (v == null) delete e.exactMedia.props[name]; else e.exactMedia.props[name] = v;
  install(e);
  return false;
}
// An `app:/` source (LLP 1069.002 D7): the app's own file, one its data
// module wrote to `app:/data` (a downloaded episode, podcast F19), as an
// object URL from the web host's picker glue (`appURL`), as an `image` shows
// one; counted in flight, so `clock settle` waits for it. A path with no
// file is handed to the element as written, which refuses it
// (`src-not-supported`), as HTML refuses a source it cannot fetch.
let Files = null;
const appURL = v => (Files ??= import(new URL("./picker-glue.js", import.meta.url).href).then(() => globalThis.exact.appURL)).then(f => f(v, data.appId));
function appSource(e, v, then) {
  e.$media.app = v;
  inflight.n++;
  appURL(v)
    .then(url => {
      if (e.$media.app !== v || e.$media.retired) return;
      if (e.getAttribute("src") !== (url || v)) e.setAttribute("src", url || v);
      e.exactMedia.props.src = url || v;
      then?.();
      install(e);
    })
    .catch(err => journal.push(`media: ${v}: ${err.message}`)).finally(() => inflight.n--);
}
// A poster that is the app's own file: shown once it resolves, none if there is none.
function appPoster(e, v) {
  inflight.n++;
  appURL(v).then(url => { if (e.$media.poster === v) { if (url) e.setAttribute("poster", url); else e.removeAttribute("poster"); } })
    .catch(() => {}).finally(() => inflight.n--);
}
// `fastSeek(id, seconds)` and `load(id)` (commands.js): queued for the glue,
// which runs them in order once it has the element. A `load` of an `app:/`
// source resolves the file again first: one written since shows.
function command(e, name, seconds) {
  if (e.$media.retired) return;
  const queue = () => (e.exactMedia.commands ??= []).push([name, seconds]);
  if (name === "load" && e.$media.app) { appSource(e, e.$media.app, queue); return; }
  queue();
  install(e);
}
/** A media event's handler (`on`): its payload, a number for the two that
 * carry one; a session action's `MediaSessionActionDetails` as the trailing
 * record (`fastSeek` the token "1", the times numbers), as `scroll`'s. */
export function mediaOn(e, kind, f) {
  e.exactMedia.handlers.push(kind);
  e.addEventListener("exact-media", ev => {
    const at = ev.detail.indexOf("\n"), name = ev.detail.slice(0, at), payload = ev.detail.slice(at + 1);
    if (name !== kind || e.$media.retired) return;
    if (SESSION.includes(kind)) { const [offset, time, fast] = payload.split(" "); f([kind, Number(offset), Number(time), fast === "1"]); }
    else if (kind === "timeupdate" || kind === "durationchange") f(Number(payload)); else if (kind === "error") f(payload); else if (kind === "fullscreenchange") f(payload === "true"); else f();
  });
}

// `requestFullscreen("id")` (rt.js's Hosts): the video with that HTML id takes
// the screen, as HTML's Element.requestFullscreen(); the glue reports
// `fullscreenchange`.
export const requestFullscreen = id => {
  const el = document.getElementById(String(id ?? ""));
  if (!el?.$media) return journal.push(`t=${clock.now} requestFullscreen: refused: no video with id "${id}"`);
  el.requestFullscreen().catch(e => journal.push(`t=${clock.now} requestFullscreen: refused: ${e.name}`));
};
// What rt.js reaches through `useMedia`, installed by the generated module only where its plan has media.
export const mediaUse = () => ({ media, mediaProp, mediaOn, mediaPiece, MEDIA_EVENTS, requestFullscreen });
