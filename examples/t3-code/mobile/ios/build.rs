// @ref llp/1106.000-mobile-app-layout.decision.md#decision
fn main() {
    exact_js_bake::build(std::path::Path::new(".."), "ios").expect("bake T3 Code for iOS");
}
