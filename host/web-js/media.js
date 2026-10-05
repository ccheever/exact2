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
import { onEnd, inflight, journal } from "./rt.js";

// The media session's actions (LLP 1098 D2, D6): the glue sends them as it
// sends the element's events, with `seekOffset seekTime fastSeek`.
const SESSION = ["seekbackward", "seekforward", "seekto", "previoustrack", "nexttrack", "stop"];
const BOOL = new Set(["autoplay", "controls", "loop", "muted", "playsinline", "disablepictureinpicture", "disableremoteplayback"]);
export const MEDIA_EVENTS = new Set(["loadedmetadata", "durationchange", "timeupdate", "play", "playing", "pause", "ended", "waiting", "seeking", "seeked", "ratechange", "volumechange", "error", "canplay", ...SESSION]);
let Glue = null, Install = null;
// Nodes whose props changed: handed to the glue together after the commit
// that built or changed them, when they are in the document.
const Dirty = new Set();
const send = e => text => e.dispatchEvent(new CustomEvent("exact-media", { detail: text }));
function flush() {
  const later = [];
  for (const e of Dirty) if (!e.$media.retired) { if (e.isConnected) Install(e, send(e)); else later.push(e); }
  Dirty.clear();
  if (later.length) requestAnimationFrame(() => { for (const e of later) install(e); });
}
function install(e) {
  if (e.$media.retired) return;
  if (!Dirty.size && Install) queueMicrotask(flush);
  Dirty.add(e);
  if (Glue) return;
  inflight.n++;
  Glue = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => { globalThis.exact ??= {}; return import("./media-glue.js"); })
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
  e.$media = { retired: false };
  e.exactMedia = { props, handlers: [] };
  onEnd(() => {
    e.$media.retired = true;
    globalThis.exact?.removeMedia?.(e);
    // As the wasm host retires a video: stopped, its source let go.
    e.pause(); e.removeAttribute("src"); e.load();
  });
  install(e);
}
/** A dynamic prop's value (`P`). */
export function mediaProp(e, name, v) {
  if (!e.$media) return;
  if (v == null) delete e.exactMedia.props[name]; else e.exactMedia.props[name] = v;
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
    else if (kind === "timeupdate" || kind === "durationchange") f(Number(payload)); else if (kind === "error") f(payload); else f();
  });
}
