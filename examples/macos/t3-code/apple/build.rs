fn main() {
    exact_js_bake::build(std::path::Path::new(".."), "macos").expect("bake T3 Code for macOS");
}
