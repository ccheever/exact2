use super::*;
#[test]
fn whole_world_journals_report_each_canvas_dropped_prefix_after_rollover() {
    let (mut p, path) = fixture();
    let abi = &p.surfaces.abis[""];
    unsafe {
        let next = abi
            .library
            .get::<unsafe extern "C" fn(u32)>(b"test_journal_next")
            .unwrap();
        next(4000);
    }
    let first: Value =
        serde_json::from_str(&p.merge_surfaces(r#"{"op":"logs"}"#, r#"{"lines":[]}"#.into()))
            .unwrap();
    assert!(first["world"]
        .as_array()
        .unwrap()
        .iter()
        .all(|w| w["dropped"] == 0));
    unsafe {
        let next = p.surfaces.abis[""]
            .library
            .get::<unsafe extern "C" fn(u32)>(b"test_journal_next")
            .unwrap();
        next(9000);
    }
    let read = |p: &mut Presenter<()>, query| -> Value {
        serde_json::from_str(&p.merge_surfaces(query, r#"{"lines":[]}"#.into())).unwrap()
    };
    let delta = read(&mut p, r#"{"op":"logs"}"#);
    assert_eq!(delta["world"].as_array().unwrap().len(), 3);
    assert!(delta["world"]
        .as_array()
        .unwrap()
        .iter()
        .all(|w| w["dropped"] == 904));
    let all = read(&mut p, r#"{"op":"logs","all":true}"#);
    assert!(all["world"]
        .as_array()
        .unwrap()
        .iter()
        .all(|w| w["dropped"] == 4904 && w["canvas"].is_number()));
    drop(p);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
