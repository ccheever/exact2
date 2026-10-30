//! The props whose records the runner's bridge sends as named JSON, written
//! the same way on the JS target: a textarea's decorations (LLP 1114.007)
//! and a Markdown node's atom faces (LLP 1045.000 D2), each through its
//! rt.js converter, a face's colours through the build's colour map as a
//! bound colour row's are.

use super::Em;
use crate::code::Scope;
use exact_kernel::PropId;
use exact_plan::{BindingKind, BindingsRow};
use std::fmt::Write as _;

const NAMED: [(PropId, &str, &str); 2] = [
    (PropId::Decorations, "data-decorations", "decorationRecord"),
    (PropId::AtomFaces, "data-atom-faces", "atomFacesRecord"),
];

impl Em<'_> {
    /// Write `b` as its named record when it is one of [`NAMED`].
    pub(super) fn named_record(
        &mut self,
        i: u32,
        b: &BindingsRow,
        e: &str,
        scope: &Scope,
    ) -> Result<bool, String> {
        let Some(&(_, attr, record)) = NAMED
            .iter()
            .find(|n| b.kind == BindingKind::Prop && b.id == n.0 as u16)
        else {
            return Ok(false);
        };
        let (value, p, record) = (
            self.bound(i, e, b, scope)?,
            self.uses.rt("P"),
            self.uses.rt(record),
        );
        // A face's colours go through the colour map, a role in its web
        // form as a bound role colour's name, and a refusal names the node's
        // view, as the runner's journal line does.
        let colors = if b.id == PropId::AtomFaces as u16 {
            let plan = self.plan;
            let map = self.uses.constant("$cm", || crate::style::color_map(plan));
            let roles = self.uses.constant("$fr", face_roles);
            let view = self.uses.rt("viewId");
            format!(",{map},()=>{view}({e}),{roles}")
        } else {
            String::new()
        };
        let _ = write!(
            self.out,
            "{p}({e},\"{attr}\",()=>{record}(({value})(){colors}));"
        );
        Ok(true)
    }
}

/// The kernel's role table as `shape.js`'s `roleIndex` reads it to name the
/// role a face's `var(--exact-…)` colour is, as `roles::role` reads one
/// (LLP 1095 D2): each role, in the kernel's order, as `[name, alias, a CSS
/// system colour, drawn in the accent, the name a bound colour gives it]`,
/// which the colour map writes as the role's CSS.
fn face_roles() -> String {
    use exact_kernel::style::roles;
    let rows: Vec<String> = exact_kernel::COLOR_ROLES
        .iter()
        .map(|r| {
            let css = roles::is_css_system(r);
            let bound = match css {
                true => r.name.to_string(),
                false => format!("-exact-{}", r.name),
            };
            let (name, alias, accent) = (r.name, r.alias, roles::web_accent(r));
            format!("[{name:?},{alias:?},{css},{accent},{bound:?}]")
        })
        .collect();
    format!("[{}]", rows.join(","))
}

#[cfg(test)]
mod tests {
    /// A face's colours go through the build's colour map (LLP 1045.000
    /// D2), the one constant every colour site shares, a role in its web
    /// form through the role table, and a refusal names the node's view.
    #[test]
    fn atom_faces_take_the_colour_map() {
        let plan = contract::compile(
            r#"atom mention prefix="@"
component App
  state c = "red"
  derive faces = [AtomFace(source="@a", label="", icon="", title="", color=c, backgroundColor="", direction="")]
  view
    column
      textarea value="@a " markup="markdown" atoms="mention" atomFaces=faces
      text "x" color=c
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(
            js.contains("atomFacesRecord(") && js.contains("(),$cm,()=>") && js.contains(",$fr));"),
            "{js}"
        );
        assert_eq!(js.matches("const $cm=").count(), 1);
        assert_eq!(js.matches("const $fr=").count(), 1);
    }

    /// `script` run in Bun with `shape.js`'s path as its argument: what it
    /// prints, as JSON.
    fn bun(script: &str) -> serde_json::Value {
        let out =
            std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
                .args(["--eval", script])
                .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shape.js"))
                .output()
                .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }

    /// `shape.js`'s `roleIndex` reads a role as the kernel's `roles::role`
    /// does, over every role's spellings, their web forms, the tint's form
    /// around each, its recursion, a role's name in another case (ASCII
    /// only: the Kelvin sign is no `k`), and the white space either trims.
    #[test]
    fn the_js_target_reads_a_role_as_the_kernel_does() {
        use exact_kernel::style::roles;
        let mut corpus: Vec<String> = Vec::new();
        for r in exact_kernel::COLOR_ROLES.iter() {
            let name = r.name;
            let mut spellings = vec![
                name.to_string(),
                name.to_ascii_uppercase(),
                format!("-exact-{name}"),
                format!("var(--exact-{name})"),
                format!("var(--exact-{}, #fff)", name.to_ascii_lowercase()),
                format!("VAR(--EXACT-{})", name.to_ascii_uppercase()),
                format!("var(--exact-{name} , #fff)"),
                format!("var(--exact-{name}"),
            ];
            if !r.alias.is_empty() {
                spellings.push(r.alias.to_string());
            }
            for spelling in spellings {
                corpus.push(format!("var(--exact-accent, {spelling})"));
                corpus.push(format!("var(--exact-accent,{spelling})"));
                corpus.push(spelling);
            }
        }
        corpus.extend(
            [
                "var(--exact-accent-text, AccentColorText)",
                " var( --exact-accent-text , accentcolortext ) ",
                "var(--exact-lin\u{212A}text)",
                "var(--exact-accent, var(--exact-lin\u{212A}text))",
                "\u{a0}var(--exact-separator)\u{2003}",
                "\u{feff}var(--exact-separator)",
                "var(--exact-accent, var(--exact-linktext))",
                "var(--exact-accent, var(--exact-accent, LinkText))",
                "var(--exact-separator) red",
                "var(--exact-)",
                "var(--exact-accent, )",
                "var(--exact-accent , LinkText)",
            ]
            .map(String::from),
        );
        let want: Vec<i64> = (corpus.iter())
            .map(|t| roles::role(t).map_or(-1, i64::from))
            .collect();
        assert!(want.iter().filter(|&&w| w >= 0).count() > 100, "{want:?}");
        let script = format!(
            "const {{ roleIndex }} = await import(process.argv[1]);\nconst roles = {};\nconsole.log(JSON.stringify({}.map(t => roleIndex(t, roles))));\n",
            super::face_roles(),
            serde_json::to_string(&corpus).unwrap()
        );
        let got = bun(&script);
        for (i, text) in corpus.iter().enumerate() {
            assert_eq!(got[i].as_i64(), Some(want[i]), "{text:?}");
        }
    }

    /// `shape.js`'s `atomFacesRecord` run in Bun on `faces` (positional
    /// records, as the compiler builds them) with `map`, the build's role
    /// table and a stand-in for the browser's `CSS.supports`: the named
    /// records and the journal.
    fn record(faces: &serde_json::Value, map: &str) -> serde_json::Value {
        bun(&format!(
            r#"
const {{ atomFacesRecord }} = await import(process.argv[1]);
const journal = [];
globalThis.exact = {{ journal, now: () => 7 }};
globalThis.CSS = {{ supports: (property, value) => property === "color" && !value.includes("notacolour") }};
const out = atomFacesRecord({faces}, {map}, () => 4, {roles});
console.log(JSON.stringify({{ out: JSON.parse(out), journal }}));
"#,
            roles = super::face_roles()
        ))
    }

    /// The converter copies the compiler's positional record field by
    /// field: each value lands under its field's name, in the order the
    /// runner's bridge reads (`ATOM_FACE_FIELDS`, which a Contract test ties
    /// to the compiler's shape).
    #[test]
    fn a_faces_fields_land_under_their_names() {
        let fields = exact_runner::decorations::ATOM_FACE_FIELDS;
        let value = |name: &str| match name {
            "color" => "red".to_string(),
            "backgroundColor" => "blue".to_string(),
            "direction" => "rtl".to_string(),
            name => format!("the {name}"),
        };
        let face: Vec<String> = fields.iter().map(|f| value(f)).collect();
        let got = record(&serde_json::json!([face]), "c=>c");
        let named = got["out"][0].as_object().unwrap();
        assert_eq!(named.len(), fields.len(), "{got}");
        for f in fields {
            assert_eq!(named[f], value(f), "{f}: {got}");
        }
    }

    /// A face's colours through the build's own colour map: what the map
    /// takes, then the browser (here its stand-in), is kept, and a role in
    /// its web form is the role, as a bound colour's name for it maps, the
    /// tint's form around it too. A colour the map refuses (an unadmitted
    /// platform colour, in a role's fallback too), the browser refuses, or
    /// that has `currentcolor`, a cascade keyword or a substitution function
    /// (`var()`, `env()`, `attr()`, `if()`, `inherit()`) outside a role's web
    /// form (escaped, too, a CRLF after the escape folded as CSS folds it)
    /// is empty, so its kind's paint stands, with one journal line in the
    /// runner's words and no other.
    #[test]
    fn a_faces_colours_go_through_the_real_colour_map() {
        let plan = contract::compile(
            r#"theme
  brand="light-dark(#d52c36, #ff6762)"
atom mention prefix="@"
component App
  state name = "systemRedColor"
  derive faces = [AtomFace(source="@a", label="", icon="", title="", color="brand", backgroundColor="-exact-platform-color(web Highlight, #ffffff)", direction=""), AtomFace(source="@b", label="", icon="", title="", color=`-exact-platform-color(web ${name}, #ff0000)`, backgroundColor="", direction="")]
  view
    textarea value="@a @b " markup="markdown" atoms="mention" atomFaces=faces
"#,
        )
        .unwrap();
        let map = crate::style::color_map(&plan);
        let faces = serde_json::json!([
            [
                "@a",
                "",
                "",
                "",
                "brand",
                "-exact-platform-color(web Highlight, #ffffff)"
            ],
            [
                "@b",
                "",
                "",
                "",
                "-exact-platform-color(web systemRedColor, #ff0000)",
                "var(--accent)"
            ],
            [
                "@c",
                "",
                "",
                "",
                "light-dark(currentcolor, #000)",
                "inh\\65rit"
            ],
            ["@d", "", "", "", "notacolour", "var(--inherit)"],
            [
                "@e",
                "",
                "",
                "",
                "var(--exact-separator)",
                "VAR(--exact-separator, inherit)"
            ],
            [
                "@f",
                "",
                "",
                "",
                "env(safe-area-inset-top)",
                "\\69\r\nnherit"
            ],
            ["@g", "", "", "", "var(--exact-nosuchrole, #fff)", ""],
            [
                "@h",
                "",
                "",
                "",
                "if(media(print): red)",
                "light-dark(if(style(--x: 1): red), #000)"
            ],
            [
                "@i",
                "",
                "",
                "",
                "inherit(--x)",
                "var(--exact-separator, -exact-platform-color(web systemRedColor, #ff0000))"
            ],
            [
                "@j",
                "",
                "",
                "",
                "var(--exact-accent, var(--exact-linktext))",
                "var(--exact-lin\u{212A}text)"
            ],
        ]);
        let got = record(&faces, &map);
        let paint = |i: usize, key: &str| got["out"][i][key].as_str().unwrap().to_string();
        assert!(
            paint(0, "color").contains("d52c36"),
            "a theme entry's name: {got}"
        );
        assert_eq!(
            paint(0, "backgroundColor"),
            "Highlight",
            "an admitted platform colour"
        );
        // A role's web form is what the colour map writes for the role's name.
        let role = record(
            &serde_json::json!([["@a", "", "", "", "-exact-separator", ""]]),
            &map,
        );
        let separator = role["out"][0]["color"].as_str().unwrap();
        assert!(separator.starts_with("var(--exact-separator,"), "{role}");
        assert_eq!(paint(4, "color"), separator, "{got}");
        assert_eq!(
            paint(4, "backgroundColor"),
            separator,
            "its fallback unread: {got}"
        );
        // The tint's web form around a role drawn in the accent is that role.
        let link = record(
            &serde_json::json!([["@a", "", "", "", "LinkText", ""]]),
            &map,
        );
        assert_eq!(
            paint(9, "color"),
            link["out"][0]["color"].as_str().unwrap(),
            "{got}"
        );
        for (i, key) in [
            (1, "color"),
            (1, "backgroundColor"),
            (2, "color"),
            (2, "backgroundColor"),
            (3, "color"),
            (3, "backgroundColor"),
            (5, "color"),
            (5, "backgroundColor"),
            (6, "color"),
            (7, "color"),
            (7, "backgroundColor"),
            (8, "color"),
            (8, "backgroundColor"),
            (9, "backgroundColor"),
        ] {
            assert_eq!(paint(i, key), "", "{i} {key}: {got}");
        }
        let journal: Vec<&str> = got["journal"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(
            journal,
            [
                r#"t=7 view 4: invalid atomFaces colour "-exact-platform-color(web systemRedColor, #ff0000)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "var(--accent)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "light-dark(currentcolor, #000)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "inh\\65rit"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "notacolour"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "var(--inherit)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "env(safe-area-inset-top)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "\\69\r\nnherit"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "var(--exact-nosuchrole, #fff)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "if(media(print): red)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "light-dark(if(style(--x: 1): red), #000)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "inherit(--x)"; its kind's paint stands"#,
                r#"t=7 view 4: invalid atomFaces colour "var(--exact-separator, -exact-platform-color(web systemRedColor, #ff0000))"; its kind's paint stands"#,
                "t=7 view 4: invalid atomFaces colour \"var(--exact-lin\u{212A}text)\"; its kind's paint stands",
            ],
            "one line a colour"
        );
    }
}
