// An element's `data-*` words on the JS target (LLP 1075.003 §3.3; rt.js
// re-exports it; bundled only into a plan that binds one).
/** The runner's object of strings as one attribute per word; a word the
 * element's last value had and this one lacks goes. */
export function ds(e, json) {
  const words = JSON.parse(json ?? "{}"), last = e.$ds ?? [];
  for (const w of last) if (!Object.hasOwn(words, w)) e.removeAttribute("data-" + w);
  for (const [w, v] of Object.entries(words)) if (e.getAttribute("data-" + w) !== v) e.setAttribute("data-" + w, v);
  e.$ds = Object.keys(words);
}
