//! The props whose records the runner's bridge sends as named JSON, written
//! the same way on the JS target: a textarea's decorations (LLP 1114.007)
//! and a Markdown node's atom faces (LLP 1045.000 D2), each through its
//! rt.js converter.

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
        let _ = write!(self.out, "{p}({e},\"{attr}\",()=>{record}(({value})()));");
        Ok(true)
    }
}
