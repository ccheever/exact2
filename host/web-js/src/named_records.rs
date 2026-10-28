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
}
