//! @ref LLP 1122 T2b — the JS target's inspect table says of each site what
//! the runner's `node` op says of every view instantiated from it: its type,
//! the rows it sets with their values, sources and tokens, and the regions
//! above it.

use serde_json::Value as Json;

fn table_and_runner(dir: &std::path::Path) -> (Json, exact_runner::Runner<()>) {
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let sites = crate::emit::Sites::new(&plan).unwrap();
    let table: Json = serde_json::from_str(&crate::inspect::table(&plan, &sites).unwrap()).unwrap();
    let runner = exact_runner::Runner::boot(plan, (), exact_kernel::Kernel::with_monospace(), Default::default(), "/").unwrap();
    (table, runner)
}

/// The tuple index of `modes` (`{axis: value}`), as the runner numbers it:
/// the last axis the least significant.
fn tuple(table: &Json, modes: &Json) -> usize {
    let mut t = 0;
    for axis in table["axes"].as_array().unwrap() {
        let values = axis[1].as_array().unwrap();
        let v = modes.get(axis[0].as_str().unwrap()).and_then(Json::as_str).unwrap_or(values[0].as_str().unwrap());
        t = t * values.len() + values.iter().position(|x| x == v).unwrap_or(0);
    }
    t
}

#[test]
fn every_views_own_rows_values_sources_and_tokens_are_its_sites_in_the_table() {
    check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/theme-fixture"));
}

#[test]
fn a_site_in_rows_and_arms_names_the_regions_above_it() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-inspect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  state on = true\n  view\n    column gap=4\n      for x in [1, 2] key=x\n        if on\n          text `${x}` color=\"#336699\" font-weight=600\n        if not on\n          text \"off\"\n      text \"after\" padding=2\n",
    )
    .unwrap();
    check(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_relative_length_is_the_kernels_and_a_controls_start_stops_what_it_resets() {
    let dir = std::env::temp_dir().join(format!("exact-web-js-inspect-em-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  action go\n    let n = 1\n  view\n    column text-transform=\"uppercase\"\n      text \"big\" font-size=20 padding-left=\"1em\"\n      button \"Go\" press=go\n",
    )
    .unwrap();
    let (table, _) = table_and_runner(&dir);
    // The `em` length is read from the element; the plain one is not.
    let rows: Vec<&Json> = table["sites"].as_object().unwrap().values().filter_map(|s| s["r"].as_array()).flatten().collect();
    assert!(rows.iter().any(|r| r["n"] == "padding_left" && r["u"] == true), "{rows:?}");
    assert!(rows.iter().any(|r| r["n"] == "font_size" && r["u"] == false), "{rows:?}");
    check(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

fn check(dir: &std::path::Path) {
    let (table, runner) = table_and_runner(dir);
    let sites: Json = serde_json::from_str(&exact_runner::agent::handle(&runner, r#"{"op":"sites"}"#)).unwrap();
    let pairs = sites["sites"].as_array().unwrap();
    assert!(pairs.len() >= 4, "{sites}");
    let mut checked = 0;
    for pair in pairs {
        let (view, site) = (pair[0].as_u64().unwrap(), pair[1].as_u64().unwrap());
        let node: Json = serde_json::from_str(&exact_runner::agent::handle(&runner, &format!(r#"{{"op":"node","id":{view}}}"#))).unwrap();
        let entry = &table["sites"][site.to_string()];
        assert_eq!(node["type"], entry["t"], "site {site}");
        // The regions above it: an arm's index, or a row (its key the page's).
        let chain: Vec<(u64, Option<u64>)> = entry["c"].as_array().map_or(vec![], |c| c.iter().map(|s| (s[0].as_u64().unwrap(), s[1].as_u64())).collect());
        let path: Vec<(u64, Option<u64>)> = node["instance"].as_array().unwrap().iter().map(|s| (s["region"].as_u64().unwrap(), s.get("arm").and_then(Json::as_u64))).collect();
        assert_eq!(chain, path, "site {site}: {node}");
        let t = tuple(&table, &node["modes"]);
        let own: Vec<&Json> = entry["r"].as_array().map_or(vec![], |r| r.iter().collect());
        for (name, row) in node["style"].as_object().unwrap() {
            let source = row["source"].as_str().unwrap();
            let mine = own.iter().find(|r| r["n"] == *name);
            // A mode boundary's implied row (LLP 1121 D7) is the page's to
            // find: the table says the site is a boundary.
            if row["token"]["implied"].as_bool() == Some(true) {
                let at = match row["from"].as_u64() {
                    // Inherited from it: the ancestor is the boundary.
                    Some(v) => &table["sites"][pairs.iter().find(|p| p[0].as_u64() == Some(v)).unwrap()[1].as_u64().unwrap().to_string()],
                    None => entry,
                };
                assert_eq!(at["b"], true, "site {site} {name}: an implied row off a boundary: {row}");
                assert!(mine.is_none(), "site {site} {name}");
                continue;
            }
            match source {
                "authored" | "dynamic" | "heading" => {
                    let mine = mine.unwrap_or_else(|| panic!("site {site}: {name} is {source} but not in the table"));
                    assert_eq!(mine["d"].as_bool().unwrap(), source == "dynamic", "site {site} {name}");
                    assert_eq!(mine["h"].as_bool().unwrap(), source == "heading", "site {site} {name}");
                    if source != "dynamic" {
                        let v = &mine["v"];
                        let v = v.get("tuples").map_or(v, |tuples| &tuples[t]);
                        assert_eq!(*v, row["value"], "site {site} {name} in tuple {t}");
                    }
                    match row.get("token") {
                        Some(token) => {
                            assert_eq!(mine["k"], token["names"], "site {site} {name}");
                            assert_eq!(mine["x"], token["text"], "site {site} {name}");
                        }
                        None => assert!(mine["k"].is_null(), "site {site} {name}: {mine}"),
                    }
                }
                "style" => {
                    let given = &entry["s"][name];
                    let from = row["from"].as_u64().and_then(|v| pairs.iter().find(|p| p[0].as_u64() == Some(v))).and_then(|p| p[1].as_u64());
                    assert_eq!(given[0].as_u64(), from, "site {site} {name}: the style's site");
                    let v = &given[1];
                    assert_eq!(v.get("tuples").map_or(v, |tuples| &tuples[t]), &row["value"], "site {site} {name}: the style's value");
                }
                "inherited" => assert!(mine.is_none(), "site {site} {name}"),
                "initial" => {
                    assert_eq!(entry["i"].get(name.as_str()).unwrap_or(&table["initial"][name]), &row["value"], "site {site} {name}");
                    // The page walks up for a row only when the table does not
                    // say it starts here: no view above may set it then.
                    if entry["i"].get(name.as_str()).is_none() {
                        let mut up = node["parent"].as_u64();
                        while let Some(v) = up {
                            let p: Json = serde_json::from_str(&exact_runner::agent::handle(&runner, &format!(r#"{{"op":"node","id":{v}}}"#))).unwrap();
                            if let Some(s) = p["site"].as_u64() {
                                let sets = table["sites"][s.to_string()]["r"].as_array().is_some_and(|r| r.iter().any(|r| r["n"] == *name));
                                assert!(!sets, "site {site} {name}: initial here, but site {s} above sets it and the table does not say it stops");
                            }
                            up = p["parent"].as_u64();
                        }
                    }
                }
                other => panic!("site {site} {name}: source {other}"),
            }
        }
        // And no row the table has that the view does not set.
        for r in &own {
            let name = r["n"].as_str().unwrap();
            assert!(node["style"][name]["source"].as_str().is_some_and(|s| ["authored", "dynamic", "heading"].contains(&s)), "site {site}: {name} in the table, not own: {}", node["style"][name]);
        }
        checked += 1;
    }
    assert!(checked >= 4);
}
