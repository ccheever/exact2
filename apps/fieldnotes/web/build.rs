fn main() {
    exact_js_bake::build_mixed(
        std::path::Path::new(".."),
        "web",
        &fieldnotes_data::Backup::default(),
    )
    .expect("bake Fieldnotes");
}
