fn main() {
    exact_js_bake::build_mixed(std::path::Path::new(".."), "web", &update_lab_data::Probe)
        .expect("bake Update Lab");
}
