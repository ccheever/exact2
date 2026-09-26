//! The kernel's integration tests: one binary, so one link and one launch.

mod apply;
mod browser_cases;
mod browser_flex;
mod browser_ratio;
mod canvas;
mod content_region;
mod env;
mod export;
mod flow;
mod flow_rows;
mod head;
mod height_binding;
mod image;
mod layout_equality;
mod motion;
mod no_panic;
mod paragraph_stamp;
mod presented_height;
mod reader;
mod support {
    pub mod reader;
}
mod review_fixes;
mod style_dynamic;
mod text_measurement_cache;
mod transform_binding;
mod video;
mod wire;
