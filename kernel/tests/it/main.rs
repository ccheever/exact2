//! The kernel's integration tests: one binary, so one link and one launch.

mod animation;
mod apply;
mod browser_cases;
mod browser_controls;
mod browser_flex;
mod browser_position;
mod browser_ratio;
mod browser_replaced;
mod canvas;
mod content_region;
mod cover;
mod env;
mod export;
mod flow;
mod flow_auto;
mod flow_rows;
mod geometry;
mod head;
mod height_binding;
mod image;
mod layout_equality;
mod motion;
mod no_panic;
mod paint;
mod paragraph_stamp;
mod presence;
mod presented_height;
mod reader;
mod rem;
mod support {
    pub mod reader;
}
mod review_fixes;
mod style_dynamic;
mod svg;
mod svg_scene;
mod text_measurement_cache;
mod timeline;
mod timeline_scope;
mod transform_binding;
mod video;
mod wire;

/// A shorthand then the `@keyframes name{…}` rules it names, resolved as a
/// runner resolves a row against its plan's table (LLP 1055 D5).
pub fn keyframed(text: &str) -> exact_motion::Animations {
    let (head, rules) = text.split_at(text.find("@keyframes").unwrap_or(text.len()));
    let table: Vec<(String, exact_motion::Keyframes)> = rules
        .split("@keyframes")
        .skip(1)
        .map(|rule| {
            let open = rule.find('{').expect("a rule");
            let body = rule[open + 1..]
                .trim_end()
                .strip_suffix('}')
                .expect("a body");
            let k = exact_motion::Keyframes::parse(body).expect("a valid rule");
            (rule[..open].trim().to_string(), k)
        })
        .collect();
    let mut a = exact_motion::Animations::parse(head).expect("a valid shorthand");
    let dropped = a.resolve(|n| table.iter().find(|(m, _)| m == n).map(|(_, k)| k));
    assert!(dropped.is_empty(), "{dropped:?}");
    a
}
