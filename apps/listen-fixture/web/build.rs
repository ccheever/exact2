fn main() {
    exact_js_bake::build(std::path::Path::new(".."), "web").expect("bake Weatherlight");
}
