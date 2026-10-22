//! Whether a page needs its presence module (LLP 1063): a plan that names an
//! exit animation or a layout transition, literal or bound. The page loads
//! `presence-glue.js` after first paint for one (`exact_presence`,
//! navigation.js `presenceLoader`), so a view whose bound layout transition
//! declares nothing at boot slides from where it was when it first gains one.
use super::Bridge;
use exact_kernel::StyleId;
use exact_plan::{BindingKind, Plan};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// `exact_presence`: 1 where the booted plan needs the presence module.
    pub fn presence(&self) -> u32 {
        let host = self.host.as_ref();
        u32::from(host.is_some_and(|h| needed(h.runner().plan())))
    }
}

/// Whether `plan` names `-exact-exit-animation` or `-exact-layout-transition`.
pub fn needed(plan: &Plan) -> bool {
    plan.bindings.iter().any(|b| {
        b.kind == BindingKind::Style
            && matches!(
                StyleId::from_bit(u32::from(b.id)),
                Some(StyleId::LayoutTransition | StyleId::ExitAnimation)
            )
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_plan_naming_either_row_needs_presence_and_one_naming_neither_does_not() {
        let plan = |row: &str| {
            contract::compile(&format!(
                "component App\n  state calm = true\n  view\n    column {row}\n      text \"a\"\n"
            ))
            .unwrap()
        };
        assert!(super::needed(&plan(
            r#"-exact-layout-transition=(calm ? "none" : "200ms ease")"#
        )));
        assert!(super::needed(&plan(
            r#"-exact-layout-transition="200ms ease""#
        )));
        assert!(!super::needed(&plan(r#"opacity=(calm ? 1 : 0.5)"#)));
        let exit = "keyframes leave\n  to opacity=0\n\ncomponent App\n  state shown = true\n  view\n    column\n      if shown\n        text \"a\" -exact-exit-animation=\"leave 200ms\"\n";
        assert!(super::needed(&contract::compile(exit).unwrap()));
    }
}
