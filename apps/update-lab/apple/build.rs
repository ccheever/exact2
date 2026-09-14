fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    exact_js_bake::build_mixed(
        std::path::Path::new(".."),
        platform,
        &update_lab_data::Probe,
    )
    .expect("bake Update Lab");
}
