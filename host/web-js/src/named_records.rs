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
            self.f(b.expr, scope)?,
            self.uses.rt("P"),
            self.uses.rt(record),
        );
        let colors = if b.id == PropId::AtomFaces as u16 {
            let plan = self.plan;
            let map = self.uses.constant("$cm", || crate::style::color_map(plan));
            format!(",{map}")
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

#[cfg(test)]
mod tests {
    /// A face's colours go through the build's colour map (LLP 1045.000
    /// D2), the one constant every colour site shares.
    #[test]
    fn atom_faces_take_the_colour_map() {
        let plan = contract::compile(
            r#"atom mention prefix="@"
component App
  state c = "red"
  derive faces = [AtomFace(source="@a", label="", icon="", title="", color=c, backgroundColor="")]
  view
    column
      textarea value="@a " markup="markdown" atoms="mention" atomFaces=faces
      text "x" color=c
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(
            js.contains("atomFacesRecord(") && js.contains("(),$cm));"),
            "{js}"
        );
        assert_eq!(js.matches("const $cm=").count(), 1);
    }

    /// A face's colours through the build's own colour map, run in Bun
    /// (`shape.js` `atomFacesRecord`): what the map takes is kept; what it
    /// refuses it journals, once; what is no colour (a substitution, an
    /// escape, a keyword, what the browser refuses) is journalled once more,
    /// and each is empty, so its kind's paint stands.
    #[test]
    fn a_faces_colours_go_through_the_real_colour_map() {
        let plan = contract::compile(
            r#"theme
  brand="light-dark(#d52c36, #ff6762)"
atom mention prefix="@"
component App
  state name = "systemRedColor"
  derive faces = [AtomFace(source="@a", label="", icon="", title="", color="brand", backgroundColor="-exact-platform-color(web Highlight, #ffffff)"), AtomFace(source="@b", label="", icon="", title="", color=`-exact-platform-color(web ${name}, #ff0000)`, backgroundColor="")]
  view
    textarea value="@a @b " markup="markdown" atoms="mention" atomFaces=faces
"#,
        )
        .unwrap();
        let map = crate::style::color_map(&plan);
        // `CSS.supports` stands for the browser's own check, which Bun lacks.
        let script = format!(
            r#"
const {{ atomFacesRecord }} = await import(process.argv[1]);
const journal = [];
globalThis.exact = {{ journal, now: () => 7 }};
globalThis.CSS = {{ supports: (property, value) => property === "color" && !value.includes("notacolour") }};
const cm = {map};
const faces = [
  ["@a", "", "", "", "brand", "-exact-platform-color(web Highlight, #ffffff)"],
  ["@b", "", "", "", "-exact-platform-color(web systemRedColor, #ff0000)", ""],
  ["@c", "", "", "", "env(safe-area-inset-top)", "attr(data-c type(<color>))"],
  ["@d", "", "", "", "v\\61r(--x)", "inherit"],
  ["@e", "", "", "", "notacolour", ""],
];
console.log(JSON.stringify({{ out: JSON.parse(atomFacesRecord(faces, cm)), journal }}));
"#
        );
        let out = std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", &script])
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shape.js"))
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let got: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let paint = |i: usize, key: &str| got["out"][i][key].as_str().unwrap().to_string();
        assert!(paint(0, "color").contains("d52c36"), "a theme entry's name: {got}");
        assert_eq!(paint(0, "backgroundColor"), "Highlight", "an admitted platform colour");
        for (i, key) in [(1, "color"), (2, "color"), (2, "backgroundColor"), (3, "color"), (3, "backgroundColor"), (4, "color")] {
            assert_eq!(paint(i, key), "", "{i} {key}: {got}");
        }
        let journal: Vec<&str> = got["journal"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
        assert_eq!(
            journal,
            [
                r#"t=7 invalid color value "-exact-platform-color(web systemRedColor, #ff0000)"; unset"#,
                r#"t=7 invalid atomFaces colour "env(safe-area-inset-top)"; its kind's paint stands"#,
                r#"t=7 invalid atomFaces colour "attr(data-c type(<color>))"; its kind's paint stands"#,
                r#"t=7 invalid atomFaces colour "v\\61r(--x)"; its kind's paint stands"#,
                r#"t=7 invalid atomFaces colour "inherit"; its kind's paint stands"#,
                r#"t=7 invalid atomFaces colour "notacolour"; its kind's paint stands"#,
            ],
            "one line a colour"
        );
    }
}
