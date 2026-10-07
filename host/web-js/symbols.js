// Symbol images on the JS target (`image "symbol:<role>"`, LLP 1011): the
// web host's rendering (`glue.js` `refreshSymbols`) — the role's path as a
// mask over the node's tint (`AccentColor` when it sets none, LLP 1095 D8),
// sized by its font — after each commit. A
// dynamic source names a role from `table`, the plan's own strings that are
// roles, or, for one its data names (ledger diary F10), from every role,
// loaded once (`symbol-roles.js`, the build's). Imported by an app's module
// only when its plan draws a symbol, binds an image's source, or names an
// `app:/` file (below).
import { After, PropHooks, inflight, journal, clock, data } from "./rt.js";

let Table = {}, All = null; // every role: not asked for, loading, or loaded (true)
const draw = (e, role) => { const [path, filled] = Table[role] ?? [""]; e.setAttribute("data-symbol-path", path); e.toggleAttribute("data-symbol-fill", !!filled); };
function everyRole() {
  inflight.n++;
  All = import("./symbol-roles.js").then(m => {
    Table = { ...m.default, ...Table }; All = true;
    for (const e of document.querySelectorAll("#exact-root img[data-symbol-source]")) draw(e, e.getAttribute("data-symbol-source").slice(7));
    refresh();
  }).finally(() => inflight.n--);
}
const STYLE = '@property --exact-tint{syntax:"<color>";inherits:false;initial-value:#000}:where(img[data-symbol-path]){--exact-tint:AccentColor}img[data-symbol-path]{background-color:var(--exact-tint)!important;mask-image:var(--exact-symbol-mask);mask-repeat:no-repeat;mask-position:center;mask-size:var(--exact-symbol-fit,100% 100%);mask-origin:content-box;mask-clip:content-box}';
/** `table`: role → [path, filled], for sources a binding names. */
export function symbols(table) {
  Table = table;
  if (typeof document === "undefined") return;
  document.head.append(Object.assign(document.createElement("style"), { textContent: STYLE }));
  PropHooks.src = (e, v) => {
    // A source that is no longer a symbol drops the symbol's rendering first,
    // so neither `refresh` nor the role table's late load paints it back over
    // what the new source shows (an `app:/` file lands asynchronously).
    if (!v?.startsWith("symbol:")) unsymbol(e);
    if (e.localName === "img" && v?.startsWith("app:/")) { appSource(e, v); return true; }
    if (e.localName === "img" && e.$app) { e.$app = null; e.removeAttribute("data-app-src"); }
    // A `data:` source past its bound shows nothing, as on every host (LLP 1011 §2; exact_raster::MAX_DATA_URL_BYTES).
    if (e.localName === "img" && v?.startsWith("data:") && v.length > DATA_LIMIT) { journal.push(`t=${clock.now} image refused: a data: source is over ${DATA_LIMIT} bytes`); e.removeAttribute("src"); return true; }
    if (e.localName !== "img" || !v?.startsWith("symbol:")) { template(e, v); return false; }
    template(e, null);
    if (!Table[v.slice(7)] && !All && !v.startsWith("symbol:sf/")) everyRole();
    e.setAttribute("data-symbol-source", v); draw(e, v.slice(7));
    // Decorative unless its author named it (`alt`, `aria-label`), as the template writes it.
    if (!e.getAttribute("alt")) e.alt = "";
    return true;
  };
  After.push(refresh);
}
function unsymbol(e) {
  if (!e.hasAttribute?.("data-symbol-source") && !e.hasAttribute?.("data-symbol-path")) return;
  for (const a of ["data-symbol-path", "data-symbol-fill", "data-symbol-source"]) e.removeAttribute(a);
  e.style.removeProperty("--exact-symbol-mask"); e.style.removeProperty("--exact-symbol-fit");
  if (e.getAttribute("src") === e.symbolPlaceholder) e.removeAttribute("src");
  e.symbolKey = e.symbolMask = e.symbolPlaceholder = e.symbolRefusal = null;
}
// An `app:/` source (LLP 1069.002 D7): the app's own file — a picked one, or
// one its data module kept in `app:/data` — shown as an object URL by the web
// host's picker glue (`appURL`), as on the wasm host; counted in flight, so
// `clock settle` waits for it. A literal source arrives as `data-app-src`.
let Files = null;
const DATA_LIMIT = 1024 * 1024;
function appSource(e, v) {
  e.$app = v;
  if (e.getAttribute("data-app-src") !== v) e.setAttribute("data-app-src", v);
  inflight.n++;
  (Files ??= import(new URL("./picker-glue.js", import.meta.url).href).then(() => globalThis.exact.appURL))
    .then(appURL => appURL(v, data.appId))
    .then(url => { if (e.$app === v && e.getAttribute("src") !== url) { if (url) e.setAttribute("src", url); else e.removeAttribute("src"); template(e, url || null); } })
    .catch(() => {}).finally(() => inflight.n--);
}
// A raster with a `tint-color` is a template (element.rs `host_css`, LLP
// 1011 §3): its alpha masks the tint. A source that becomes a raster takes
// the mask; one that becomes a symbol (or no tint) drops it.
const TEMPLATE = ["mask-image", "mask-size", "mask-repeat", "mask-position", "mask-origin", "mask-clip", "object-position"];
// (the class rules nest under the root's selector: walk nested rules.)
const declares = (rules, sel) => [...rules].some(r => [sel, "& " + sel, "&" + sel].includes(r.selectorText) && r.style.getPropertyValue("--exact-tint") || r.cssRules && declares(r.cssRules, sel));
const tinted = e => !!e.style.getPropertyValue("--exact-tint") || [...e.classList].some(c => [...document.styleSheets].some(sh => { try { return declares(sh.cssRules, "." + c); } catch { return false; } }));
function template(e, v) {
  if (e.localName !== "img" || !v || !tinted(e)) { if (e.$template) { for (const p of TEMPLATE) e.style.removeProperty(p); e.style.removeProperty("background-color"); e.$template = false; } return; }
  const fit = getComputedStyle(e).objectFit, size = { fill: "100% 100%", contain: "contain", cover: "cover", none: "auto" }[fit] ?? "var(--exact-tint-fit,contain)";
  e.style.setProperty("background-color", "var(--exact-tint)");
  e.style.setProperty("mask-image", `url(${JSON.stringify(v)})`); e.style.setProperty("mask-size", size);
  e.style.setProperty("mask-repeat", "no-repeat"); e.style.setProperty("mask-position", "center"); e.style.setProperty("mask-origin", "content-box"); e.style.setProperty("mask-clip", "content-box"); e.style.setProperty("object-position", "-100000px 0");
  e.$template = true;
  if (fit === "scale-down") { const set = () => { const cs = getComputedStyle(e); e.style.setProperty("--exact-tint-fit", e.naturalWidth <= e.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight) && e.naturalHeight <= e.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom) ? "auto" : "contain"); }; if (e.complete) set(); else e.addEventListener("load", set, { once: true }); }
}
function refresh() {
  for (const el of document.querySelectorAll("#exact-root img[data-app-src]")) if (el.$app === undefined) appSource(el, el.getAttribute("data-app-src"));
  for (const el of document.querySelectorAll("#exact-root img[data-symbol-path]")) {
    const cs = getComputedStyle(el), size = parseFloat(cs.fontSize), weight = Number(cs.fontWeight);
    const path = el.getAttribute("data-symbol-path"), filled = el.hasAttribute("data-symbol-fill"), key = `${path}:${filled}:${size}:${weight}`;
    // As the web host says it (glue.js `refreshSymbols`), once every role is here.
    const source = el.getAttribute("data-symbol-source");
    if (!path && All === true && !source?.startsWith("symbol:sf/") && el.symbolRefusal !== source) {
      journal.push(`t=${clock.now} image ${source} refused: unknown symbol role`); el.symbolRefusal = source;
    }
    if (el.symbolKey !== key) {
      el.symbolKey = key;
      const point = size, stroke = 1.1 + (Math.max(100, Math.min(900, weight)) - 100) / 400;
      const paint = filled ? 'fill="black" fill-rule="evenodd"' : `fill="none" stroke="black" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round"`;
      el.symbolMask = `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}" viewBox="0 0 24 24"><path d="${path}" ${paint}/></svg>`)}")`;
      el.symbolPlaceholder = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
    }
    if (el.getAttribute("src") !== el.symbolPlaceholder) el.src = el.symbolPlaceholder;
    el.style.setProperty("--exact-symbol-mask", el.symbolMask);
    const px = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight), py = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const fits = size <= el.clientWidth - px && size <= el.clientHeight - py;
    el.style.setProperty("--exact-symbol-fit", cs.objectFit === "none" || (cs.objectFit === "scale-down" && fits) ? `${size}px ${size}px` : cs.objectFit === "scale-down" ? "contain" : cs.objectFit === "fill" ? "100% 100%" : cs.objectFit);
  }
}
