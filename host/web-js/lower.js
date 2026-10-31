// @ref LLP 1071.001 part B, B5 — an enumerated style binding as a class (lower.rs). A lowered node's rows are
// one key (the enum's arm, by index) and, for each arm, the `S` statements `style_row` wrote (`recipe`) run on
// that arm's literals (`values[k]`). The first time an arm is used at a site its declarations are built from
// those statements, the same maps and `cssText`, with `css`'s refusal read-back (A8), into one rule of
// `<style id="exact-arms">` at three ids and a class: over every static class, under every `!important` rule.
// That DOM work is once a site an arm, never an instance; after it the node's write is its class alone, cached.
import { adopting, css, cssText, effect, normal, say } from "./rt.js";

let Arms = null;
// What builds a rule's text besides the recipe: a host whose runtime changed (a reload under a kept document) starts the
// sheet over, since the names cover only the recipe, its literals and its constants. The build stamps `RUNTIME` with a
// digest of the runtime it copies, this file's own source included (module.mjs `withRuntimeStamp`). An unbuilt runtime
// (a test serving the sources) has no digest of what its recipes reach, so it never reuses a kept sheet: each module
// generation is a host of its own.
const RUNTIME = "exact-runtime-unstamped";
const HOST = RUNTIME !== "exact-runtime-unstamped" ? RUNTIME : `unstamped ${Math.random()} ${Date.now()}`;
/** The rule `name` (`xa<digest>-<arm>`), built once: the arms sheet keeps what it holds across module reloads of the
 * same host. A test page (`__exactArmsCheck`) also checks that a name already built stood for the same content. */
function rule(name, recipe, b) {
  if (!Arms) {
    Arms = document.getElementById("exact-arms");
    if (!Arms) { Arms = document.createElement("style"); Arms.id = "exact-arms"; document.head.append(Arms); }
    if (Arms.$host !== HOST) { Arms.$host = HOST; Arms.$built = new Map(); while (Arms.sheet.cssRules.length) Arms.sheet.deleteRule(0); }
  }
  const built = Arms.$built, check = globalThis.__exactArmsCheck;
  if (built.has(name)) {
    if (check && built.get(name) !== `${recipe}\0${JSON.stringify(b)}`) throw new Error(`exact-arms: ${name} stands for other declarations`);
    return;
  }
  const sheet = Arms.sheet, at = sheet.insertRule(`#exact-root#exact-root#exact-root{& .${name}{}}`, sheet.cssRules.length), st = sheet.cssRules[at].cssRules[0].style;
  recipe((_, prop, unit, f) => {
    const v = f(), t = cssText(prop, unit, v);
    if (t == null) return st.removeProperty(prop);
    if (prop.startsWith("--") || normal(prop, t)) return st.setProperty(prop, t);
    st.setProperty(prop, "unset"); say(`unset ${prop}: ${JSON.stringify(v)} is not a value it takes`);
  }, b);
  // Only once its declarations are in: a recipe that throws leaves nothing cached.
  built.set(name, check ? `${recipe}\0${JSON.stringify(b)}` : true);
}

/** Node `e`'s lowered rows: `key()` picks the arm; `statics` is its static class and a space, or empty. */
export function Pc(e, key, statics, name, values, props, recipe) {
  let rendered = adopting(), last;
  e.$lowered = props; // the rows a binding writes, as `S`'s `$css` says (agent.js `nodeDetail`)
  effect(() => {
    const k = key();
    if (k === last) return;
    last = k;
    // A render writes the rows inline, as `S` does, and the page it makes is as before; adoption swaps them.
    if (globalThis.__exactRender) {
      const now = new Map();
      recipe((_, prop, unit, f) => now.set(prop, [unit, f()]), values[k]);
      for (const p of props) { const w = now.get(p); css(e, p, w ? w[0] : "", w ? w[1] : null); }
      return;
    }
    rule(`${name}-${k}`, recipe, values[k]);
    e.className = `${statics}${name}-${k}`;
    // The renderer's inline rows go after the class is on, in the same task: no frame shows neither (theme2 condition 9).
    if (rendered) for (const p of props) e.style.removeProperty(p);
    rendered = false;
  });
}
