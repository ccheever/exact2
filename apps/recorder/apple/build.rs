fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // It records from a microphone, which an Apple TV does not have built in.
        Ok("tvos") => panic!("Recorder has no tvOS build: it needs a microphone"),
        Ok("ios") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).expect("bake the recorder");
}
