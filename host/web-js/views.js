// A package's views on the JS target (LLP 1133.001 D1; LLP 1133.008 D8
// items 1 and 2), as the runner's `modules/views.rs` does natively. A tag the
// plan's `views` table names is made by its extension's web implementation,
// whose chunk exports `roster`, each tag's view class:
//
//   export const roster = { 'swatch-box': { view: SwatchBox } };
//   class SwatchBox {
//     constructor({ implementation, element, props, events, view }) // the session's instance
//     setProps(props)   // every prop, as strings, on each change
//     destroy()
//   }
//
// native-glue.js makes it through `globalThis.exact.packageViews`. After each
// commit, each view's implementation is told when the view's displayed state
// changes (`displayed(view, shown, origin)`, by wakelock-glue.js's rule, for a
// view declared `"displayed": true`) and when a prop its declaration marks as
// binding a device changes (`bound(view, prop, origin)`). The web has no tool
// or wake door, so every origin is `app`.
import { After, say, viewId } from "./rt.js";
import { displayed } from "./wakelock-glue.js";

/** `views`: tag → [extension, displayed, binding props]; `chunk(extension)`, its web implementation's module;
 * `instance(extension)`, the session's instance of it. */
export function packageViews(views, chunk, instance) {
  const tags = Object.keys(views);
  if (!tags.length) return;
  (globalThis.exact ??= {}).packageViews = {
    serves: tag => Object.hasOwn(views, tag),
    async make(tag, { element, props, events, view }) {
      const [extension] = views[tag];
      const [m, implementation] = await Promise.all([chunk(extension), instance(extension)]);
      const View = m?.roster?.[tag]?.view;
      if (typeof View !== "function") throw new Error(`${extension}'s web implementation serves no ${tag}`);
      return new View({ implementation, element, props, events, view: String(view) });
    },
  };
  const told = new Map(); // element → { shown, bound: prop → its value's JSON }
  const tell = (extension, tag, id, what, ...args) => {
    say(`view ${tag} ${id}: ${what === "displayed" ? `displayed ${args[0]}` : `bound ${args[0]}`} (app origin)`);
    instance(extension)?.then(impl => impl?.[what]?.(id, ...args, "app"));
  };
  After.push(() => {
    const root = typeof document === "object" ? document.getElementById("exact-root") : null;
    if (!root) return;
    const now = new Set();
    for (const tag of tags) {
      const [extension, hears, binds] = views[tag];
      for (const el of root.querySelectorAll(tag)) {
        now.add(el);
        const id = String(viewId(el)), was = told.get(el) ?? { shown: false, bound: new Map() };
        told.set(el, was);
        const shown = hears && displayed(el, root);
        if (hears && shown !== was.shown) { was.shown = shown; tell(extension, tag, id, "displayed", shown); }
        let props = {};
        try { props = JSON.parse(el.getAttribute("data-nativeviewprops") ?? "{}"); } catch {}
        for (const prop of binds) {
          if (props[prop] == null) continue;
          const value = JSON.stringify(props[prop]);
          if (was.bound.get(prop) === value) continue;
          was.bound.set(prop, value);
          tell(extension, tag, id, "bound", prop);
        }
      }
    }
    for (const [el, was] of told) {
      if (now.has(el)) continue;
      told.delete(el);
      if (was.shown) tell(views[el.localName][0], el.localName, String(viewId(el)), "displayed", false);
    }
  });
}
