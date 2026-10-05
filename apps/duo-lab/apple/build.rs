fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // tvOS bakes the iOS host's plan.
        Ok("ios" | "tvos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).expect("bake Duo Lab");
}
