//! Right to left, a padding or margin pair whose two sides different hands
//! say: a side its author binds keeps the sheet's other side physical while
//! it has a value, and unset, neither side is the author's and the sheet's
//! pair mirrors, as the kernel's `mirror_unsaid` and css.rs's `unsaid_side`
//! have it. Logical sides, inline, outrank the class's physical ones.

use super::Em;
use crate::code::Scope;
use crate::style;
use exact_kernel::StyleId;
use exact_plan::BindingsRow;

/// A style write: (CSS property, unit, map), as rows.rs writes them.
type Write = (String, String, Option<String>);

impl Em<'_> {
    /// What binding `b`, a side of a pair, writes beside its own `writes`:
    /// an author's side beside the sheet's static other side (in `css`, the
    /// class) mirrors that pair while it is unset; a side only the sheet
    /// binds, beside an author's bound other side, is physical while the
    /// author's has a value and logical while it is unset.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn side_writes(
        &mut self,
        i: u32,
        e: &str,
        scope: &Scope,
        b: &BindingsRow,
        other: Option<&BindingsRow>,
        css: &str,
        timeline: bool,
        writes: &mut Vec<Write>,
    ) -> Result<(), String> {
        let plan = self.plan;
        let Some(id) = StyleId::from_bit(b.id as u32).filter(|id| style::other_side(*id).is_some())
        else {
            return Ok(());
        };
        let fixed = |o: &BindingsRow| style::literal(plan, plan.code(o.expr)).is_some();
        if b.written && other.is_none_or(fixed) {
            writes.extend(style::unset_mirrors(id, css));
            return Ok(());
        }
        let Some(theirs) = other.filter(|o| !b.written && o.written && !fixed(o)) else {
            return Ok(());
        };
        let getter = self.bound(i, e, theirs, scope)?;
        let unset = format!("{}(({getter})())==null", self.uses.rt("blank"));
        let gate = |map: Option<String>, shown: bool| {
            let inner = map.unwrap_or_else(|| "v=>v".into());
            let (on, off) = if shown {
                (format!("({inner})(v)"), "null".to_string())
            } else {
                ("null".to_string(), format!("({inner})(v)"))
            };
            Some(format!("v=>{unset}?{on}:{off}"))
        };
        for w in writes.iter_mut() {
            w.2 = gate(w.2.take(), false);
        }
        for w in style::style_writes(b.id, timeline, false)? {
            writes.push((w.name, w.unit, gate(w.map.map(str::to_string), true)));
        }
        Ok(())
    }
}
