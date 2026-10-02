// Symbol images on the JS target (`image "symbol:<role>"`, LLP 1011): the
// web host's rendering (`glue.js` `refreshSymbols`) — the role's path as a
// mask over the node's tint, sized by its font — after each commit. A
// dynamic source names a role from `table`, the plan's own strings that are
// roles. Imported by an app's module only when its plan draws a symbol.
import { After, PropHooks } from "./rt.js";

let Table = {};
const STYLE = '@property --exact-tint{syntax:"<color>";inherits:false;initial-value:#000}img[data-symbol-path]{background-color:var(--exact-tint)!important;mask-image:var(--exact-symbol-mask);mask-repeat:no-repeat;mask-position:center;mask-size:var(--exact-symbol-fit,100% 100%);mask-origin:content-box;mask-clip:content-box}';
/** `table`: role → [path, filled], for sources a binding names. */
export function symbols(table) {
  Table = table;
  if (typeof document === "undefined") return;
  document.head.append(Object.assign(document.createElement("style"), { textContent: STYLE }));
  PropHooks.src = (e, v) => {
    if (e.localName !== "img" || !v?.startsWith("symbol:")) { e.removeAttribute("data-symbol-path"); e.removeAttribute("data-symbol-fill"); e.removeAttribute("data-symbol-source"); e.symbolKey = null; return false; }
    const [path, filled] = Table[v.slice(7)] ?? [""];
    e.setAttribute("data-symbol-source", v); e.setAttribute("data-symbol-path", path); e.toggleAttribute("data-symbol-fill", !!filled); e.alt = "";
    return true;
  };
  After.push(refresh);
}
function refresh() {
  for (const el of document.querySelectorAll("#exact-root img[data-symbol-path]")) {
    const cs = getComputedStyle(el), size = parseFloat(cs.fontSize), weight = Number(cs.fontWeight);
    const path = el.getAttribute("data-symbol-path"), filled = el.hasAttribute("data-symbol-fill"), key = `${path}:${filled}:${size}:${weight}`;
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
