fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // tvOS bakes the iOS host's plan.
        Ok("ios" | "tvos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        platform,
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
