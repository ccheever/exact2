fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // tvOS bakes the iOS host's plan.
        Ok("ios" | "tvos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        platform,
        &fieldnotes_data::Backup::default(),
        fieldnotes_data::RUST_SOURCES,
        |javascript| {
            Box::new(fieldnotes_data::mixed(
                javascript,
                fieldnotes_data::Placement::Main,
            ))
        },
    )
    .expect("bake Fieldnotes");
}
