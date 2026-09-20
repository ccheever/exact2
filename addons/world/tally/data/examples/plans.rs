fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/plans");
    std::fs::create_dir_all(&dir).unwrap();
    let text = include_str!("../../app.contract");
    for ms in [10, 16, 20, 100] {
        let plan = contract::compile(&text.replace("every(10,", &format!("every({ms},"))).unwrap();
        let baked = contract::bake(plan, tally_data::TallySource::default()).unwrap();
        std::fs::write(dir.join(format!("{ms}.plan")), baked.encode()).unwrap();
    }
}
