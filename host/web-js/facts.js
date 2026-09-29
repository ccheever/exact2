// `exactViewport` on the JS target (LLP 1039 D1–D4, 1061 D4): the runner's
// reserved source (runner/src/viewport.rs), answered before the app's
// (`data.reserved`, as the entry answers `exactTime`) by the declared
// fields' names, from the page's own readings (`navigation.js`), and
// re-answered on every change in one commit (glue.js `mediaChanged`).
// Imported by an app's module only when its plan declares it.
import { data, Resources, commit, R } from "./rt.js";
import { preferences, onPreferences } from "./navigation.js";

const CONTRAST = ["no-preference", "more", "less", "custom"];
/** `fields`: the declared record's fields, in the shape's order. */
export function viewport(fields) {
  (data.reserved ??= {}).exactViewport = () => {
    const p = preferences(), v = { width: innerWidth, height: innerHeight, prefersReducedMotion: !!(p & 1), prefersReducedTransparency: !!(p & 2),
      prefersContrast: CONTRAST[(p >> 2) & 3], prefersColorScheme: p & 16 ? "dark" : "light" };
    return fields.map(n => v[n]);
  };
  if (typeof addEventListener !== "function") return;
  const again = () => commit(() => { for (const r of Resources) if (r.source === "exactViewport") R(r); }, "viewport");
  addEventListener("resize", again);
  onPreferences(again);
}
