fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS");
    // Its crop handles and pinch need a touchscreen, which an Apple TV lacks.
    if target_os.as_deref() == Ok("tvos") {
        panic!("Photo Editor has no tvOS build: its editing needs a touchscreen");
    }
    let platform = match target_os.as_deref() {
        Ok("ios" | "tvos" | "watchos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).expect("bake the photo editor");
}
