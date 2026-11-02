fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS");
    // It records from a microphone, which an Apple TV does not have built in.
    if target_os.as_deref() == Ok("tvos") {
        panic!("Recorder has no tvOS build: it needs a microphone");
    }
    let platform = match target_os.as_deref() {
        Ok("ios" | "tvos" | "watchos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).expect("bake the recorder");
}
