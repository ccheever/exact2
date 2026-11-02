fn main() {
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        "web",
        &update_lab_data::Probe,
        update_lab_data::RUST_SOURCES,
        |javascript| {
            Box::new(update_lab_data::compose(
                javascript,
                update_lab_data::Probe,
                false,
                |_| Ok(update_lab_data::Probe),
            ))
        },
    )
    .expect("bake Update Lab");
}
