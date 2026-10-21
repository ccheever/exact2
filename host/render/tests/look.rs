//! Server renders choose neutral roles before projecting either document form.
use exact_kernel::style::roles::{self, Column};
use exact_render::{Ids, Projection};
use exact_runner::{DataError, DataSource, Value};
use exact_web::document::Site;
use std::time::Duration;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn render_without_a_shell_uses_neutral_roles_in_both_projections() {
    exact_web::link(exact_web_capabilities::ALL);
    let plan = contract::compile(
        "component App\n  view\n    column border-color=\"-exact-separator\"\n      input type=\"checkbox\"\n",
    )
    .unwrap();
    for projection in [Projection::Kernel, Projection::Direct] {
        roles::set_column(Column::Apple);
        let rendered = exact_render::render_with_at(
            &plan,
            &|| NoData,
            Default::default(),
            "/",
            &Site {
                name: "Neutral",
                origin: None,
            },
            Duration::from_secs(5),
            Ids::Any,
            projection,
            0.0,
        )
        .unwrap();
        assert_eq!(roles::column(), Column::Neutral);
        assert!(
            rendered
                .document
                .root
                .contains("light-dark(#0000001f, #ffffff29)"),
            "{:?}: {}",
            projection,
            rendered.document.root
        );
        assert!(rendered.document.root.contains("data-exact-control"));
    }
}
