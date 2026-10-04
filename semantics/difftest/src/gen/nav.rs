//! Navigation (LLP 1038): a fixed `routes nav` table, the router's reads
//! as root derives of the types the generator writes, and `nav = verb(…)`
//! statements over it — the six verbs, `path(…)` with string and number
//! parameters, and locations that are canonicalized, unmatched or refused.

use super::ty::{quote, Ty};
use super::{Env, Gen};

/// Two tabs with nested rows, a top-level row and a notfound row.
pub(crate) const TABLE: &str = "routes nav
  tab home \"/\"
    item \"/item/:id\"
      part \"/item/:id/part/:part\"
  tab other \"/other\"
    note \"/note/:id\"
  person \"/people/:id\"
  notfound
";

/// Locations a verb is handed: matched, canonicalized, with a query, and
/// unmatched (absorbed by notfound, or refused by `replace` at a root).
const LOCATIONS: &[&str] = &[
    "/",
    "/item/1",
    "/item/a%20b/part/2",
    "/item/7/part/x?q=a+b&tag=t",
    "/other",
    "/note/n1",
    "/people/ada?q=1",
    "item/%7e/./../9",
    "/note/%ff",
];

/// Locations the compiler refuses as literals (no route matches them), so
/// they are computed: the runner refuses them when the verb runs.
const UNMATCHED: &[&str] = &["/nowhere", "/item/x/", "/item/1/part"];

const TABS: &[&str] = &["home", "other", "home", "item", "nope"];

impl Gen<'_> {
    /// The router's reads, as root derives: written into `decls`, their
    /// names and types returned.
    pub(crate) fn nav_derives(&mut self, decls: &mut String) -> Vec<(String, Ty)> {
        let all: &[(&str, &str, Ty)] = &[
            ("navUrl", "top(nav).url", Ty::Str),
            ("navName", "top(nav).name", Ty::Str),
            ("navTab", "nav.tab", Ty::Str),
            ("navDepth", "depth(nav)", Ty::Num),
            ("navId", "top(nav).params.id", Ty::Str),
            ("navIds", "params(nav, \"id\")", Ty::list(Ty::Str)),
            ("navQ", "searchParam(top(nav), \"q\")", Ty::Str),
            (
                "navUrls",
                "map(stack(nav), (e) => e.url)",
                Ty::list(Ty::Str),
            ),
            (
                "navTabs",
                "map(nav.tabs, (t) => `${t.name}:${length(t.stack)}`)",
                Ty::list(Ty::Str),
            ),
        ];
        let mut out = Vec::new();
        for (name, e, t) in all {
            // The url and depth always, so every view shows where it is.
            if out.len() < 2 || self.rng.chance(1, 2) {
                decls.push_str(&format!("  derive {name} = {e}\n"));
                out.push((name.to_string(), t.clone()));
            }
        }
        out
    }

    /// `nav = verb(nav, …)`: a verb over the router slot.
    pub(crate) fn nav_stmt(&mut self, env: &Env, pad: &str) -> String {
        let verb = match self.rng.weighted(&[4, 3, 2, 2, 2, 2, 2]) {
            0 => {
                let arg = self.path_arg(env);
                format!("push(nav, path(\"item\", {arg}))")
            }
            1 => format!("push(nav, {})", self.location(env)),
            2 => format!("open(nav, {})", self.location(env)),
            3 => "back(nav)".into(),
            4 => format!("select(nav, {})", quote(self.rng.pick(TABS))),
            5 => format!("go(nav, {})", self.location(env)),
            _ => {
                let (a, b) = (self.path_arg(env), self.path_arg(env));
                let verb = *self.rng.pick(&["replace", "push"]);
                format!("{verb}(nav, path(\"part\", {a}, {b}))")
            }
        };
        format!("{pad}nav = {verb}\n")
    }

    /// A location literal, or now and then one the program computes.
    fn location(&mut self, env: &Env) -> String {
        if self.rng.chance(1, 6) {
            let arg = self.path_arg(env);
            return format!("path(\"note\", {arg})");
        }
        if self.rng.chance(1, 8) {
            return format!("trim({})", quote(self.rng.pick(UNMATCHED)));
        }
        quote(self.rng.pick(LOCATIONS))
    }

    /// A `path` parameter: a string or a number (printed by `toString`);
    /// an empty or dot-only one is refused.
    fn path_arg(&mut self, env: &Env) -> String {
        let t = if self.rng.chance(1, 3) {
            Ty::Num
        } else {
            Ty::Str
        };
        if self.rng.chance(1, 2) {
            match t {
                Ty::Num => self.num_lit(),
                // An empty or dot-only literal is the compiler's refusal;
                // computed, it is the runner's.
                _ => match *self
                    .rng
                    .pick(&["a", "x y", "ü/€", "7", "..", "", "q?r", "."])
                {
                    s @ (".." | "" | ".") => format!("trim({})", quote(s)),
                    s => quote(s),
                },
            }
        } else {
            match self.expr(env, &t, 1, false) {
                e if ["\"\"", "\".\"", "\"..\""].contains(&e.as_str()) => format!("trim({e})"),
                // A computed string is held short: a location built from
                // the last one (`top(nav).url`) would otherwise grow with
                // every step, past the runner's string bound, which the
                // semantics leaves out (semantics/README.md).
                e if t == Ty::Str => format!("((length({e}) > 40) ? \"long\" : {e})"),
                e => e,
            }
        }
    }
}
