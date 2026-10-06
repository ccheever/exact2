// Tabs built on first selection on the JS target (LLP 1075.003 §3.7), as
// `runner/src/instance/tabs.rs`: a route in a `role="tabpanel"` builds its
// children once its panel holds the root's selected route (a route whose
// `navigationKey` is the root's), then keeps them. Imported by an app's
// module only when its plan has a panel route.
import { scope, untracked, owner, onEnd, unadopted, adopting, at, useTabs } from "./rt.js";

const Deferred = new Set();
const navKey = e => e.getAttribute("navigationKey");
/** A panel route `e`, in `panel` under navigation root `root`; `f` builds its children. */
export function dl(e, panel, root, f) {
  (panel.$routes ??= new Set()).add(e);
  onEnd(() => { panel.$routes.delete(e); Deferred.delete(e); });
  // A rendered page's route with children was built where it was rendered.
  if (adopting() && at(e)) { panel.$open = true; f(); return; }
  const own = owner();
  e.$build = () => scope(f, own); e.$panel = panel; e.$root = root;
  Deferred.add(e);
}
// Runs in each commit after its effects flush, so a selection and its panel's first build are one commit.
// Repeat until nothing opens: a route built here may hold its own navigation root.
useTabs(() => {
  for (let more = true; more;) {
    more = false;
    for (const e of Deferred) {
      const p = e.$panel, k = navKey(e.$root);
      if (!p.$open && k != null) for (const r of p.$routes) if (navKey(r) === k) { p.$open = true; break; }
    }
    for (const e of [...Deferred]) if (e.$panel.$open && Deferred.delete(e)) { more = true; untracked(() => unadopted(e.$build)); }
  }
});
