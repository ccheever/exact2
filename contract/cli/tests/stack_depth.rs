//! A 2 MiB debug-thread depth regression and a manual pass-by-pass probe.

fn nested_view_source(sites: usize) -> String {
    let mut source = String::from("component App\n  view\n");
    for level in 0..sites {
        source.push_str(&" ".repeat(4 + level * 2));
        source.push_str("column\n");
    }
    source
}

fn nested_branch_source(branches: usize, kind: &str, wrapped: bool) -> String {
    let mut source = String::from("component App\n  view\n");
    if wrapped {
        source.push_str("    column\n");
    }
    let base = if wrapped { 6 } else { 4 };
    for level in 0..branches {
        let indent = " ".repeat(base + level * if kind == "match" { 4 } else { 2 });
        if kind == "match" {
            source.push_str(&format!("{indent}match some(1)\n{indent}  case some(x)\n"));
        } else {
            source.push_str(&format!("{indent}when true\n"));
        }
    }
    source.push_str(&" ".repeat(base + branches * if kind == "match" { 4 } else { 2 }));
    source.push_str("text \"leaf\"\n");
    if kind == "match" {
        for level in (0..branches).rev() {
            let indent = " ".repeat(base + level * 4);
            source.push_str(&format!(
                "{indent}  case none\n{indent}    text \"other\"\n"
            ));
        }
    }
    source
}

fn nested_button_source(buttons: usize) -> String {
    let mut source = String::from("component App\n  view\n");
    for level in 0..buttons {
        source.push_str(&" ".repeat(4 + level * 2));
        source.push_str("button \"ok\"\n");
    }
    source
}

fn deep_component_source() -> String {
    let mut source = nested_view_source(254);
    source.push_str(&" ".repeat(4 + 254 * 2));
    source.push_str("Child()\ncomponent Child\n  view\n");
    for level in 0..255 {
        source.push_str(&" ".repeat(4 + level * 2));
        source.push_str("column\n");
    }
    source.push_str(&" ".repeat(4 + 255 * 2));
    source.push_str("text \"leaf\"\n");
    source
}

fn deep_slot_fill_source() -> String {
    let mut source = nested_view_source(254);
    source.push_str(&" ".repeat(4 + 254 * 2));
    source.push_str("Child()\ncomponent Child\n  view\n    Frame()\n");
    for level in 0..254 {
        source.push_str(&" ".repeat(6 + level * 2));
        source.push_str("column\n");
    }
    source.push_str(&" ".repeat(6 + 254 * 2));
    source.push_str("text \"leaf\"\n");
    source.push_str("component Frame\n  slot\n  view\n    column\n      children\n");
    source
}

fn deep_forwarded_slot_fill_source() -> String {
    deep_slot_fill_source().replace(
        "component Frame\n  slot\n  view\n    column\n      children\n",
        "component Frame\n  slot\n  view\n    Pass()\n      children\ncomponent Pass\n  slot\n  view\n    children\n",
    )
}

fn component_chain_source(components: usize, sites_each: usize) -> String {
    let mut source = String::new();
    for component in 0..components {
        let name = if component == 0 {
            "App".to_owned()
        } else {
            format!("C{component}")
        };
        source.push_str(&format!("component {name}\n  view\n"));
        for level in 0..sites_each {
            source.push_str(&" ".repeat(4 + level * 2));
            source.push_str("column\n");
        }
        source.push_str(&" ".repeat(4 + sites_each * 2));
        if component + 1 == components {
            source.push_str("text \"leaf\"\n");
        } else {
            source.push_str(&format!("C{}()\n", component + 1));
        }
    }
    source
}

fn assert_small_stack(cases: Vec<(String, Option<&'static str>)>) {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            for (source, refused) in cases {
                match (contract::compile(&source), refused) {
                    (Ok(_), None) => {}
                    (Err(error), Some(id)) => assert_eq!(error.id, id, "{error}"),
                    (result, expected) => {
                        panic!("expected {expected:?}, got {:?}", result.map(|_| ()))
                    }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn deep_columns_fit_a_small_stack() {
    assert_small_stack(vec![
        (nested_view_source(256), None),
        (nested_view_source(257), Some("syntax-view-depth")),
    ]);
}

#[test]
fn deep_matches_fit_a_small_stack() {
    assert_small_stack(vec![
        (nested_branch_source(254, "match", true), None),
        (
            nested_branch_source(255, "match", true),
            Some("syntax-view-depth"),
        ),
    ]);
}

#[test]
fn deep_whens_fit_a_small_stack() {
    assert_small_stack(vec![
        (nested_branch_source(254, "when", true), None),
        (
            nested_branch_source(255, "when", true),
            Some("syntax-view-depth"),
        ),
    ]);
}

#[test]
fn implicit_button_text_uses_a_site() {
    assert_small_stack(vec![
        (nested_button_source(255), None),
        (nested_button_source(256), Some("syntax-view-depth")),
    ]);
}

#[test]
fn component_uses_fit_a_small_stack() {
    assert_small_stack(vec![
        (component_chain_source(8, 15), None),
        (deep_component_source(), Some("syntax-inline-depth")),
        (deep_slot_fill_source(), Some("syntax-inline-depth")),
        (
            deep_forwarded_slot_fill_source(),
            Some("syntax-inline-depth"),
        ),
        (component_chain_source(17, 15), None),
        (component_chain_source(40, 60), Some("syntax-inline-depth")),
    ]);
}

#[test]
fn discarded_slot_fill_does_not_consume_expanded_site_depth() {
    let mut source = String::from(
        "component App\n  view\n    column\n      column\n        Child()\ncomponent Child\n  view\n    Discard()\n",
    );
    for level in 0..255 {
        source.push_str(&" ".repeat(6 + level * 2));
        source.push_str("column\n");
    }
    source.push_str("component Discard\n  slot\n  view\n    text \"ok\"\n");
    assert_small_stack(vec![(source, None)]);
}

#[test]
fn discarded_slot_fill_under_deep_callers_fits_a_small_stack() {
    let mut source = nested_view_source(255);
    source.push_str(&" ".repeat(4 + 255 * 2));
    source.push_str("P1()\n");
    for component in 1..=29 {
        source.push_str(&format!(
            "component P{component}\n  view\n    {}()\n",
            if component == 29 {
                "Host".to_owned()
            } else {
                format!("P{}", component + 1)
            }
        ));
    }
    source.push_str("component Host\n  view\n    Discard()\n");
    for level in 0..255 {
        source.push_str(&" ".repeat(6 + level * 2));
        source.push_str("column\n");
    }
    source.push_str("component Discard\n  slot\n  view\n    text \"ok\"\n");
    assert_small_stack(vec![(source, None)]);
}

#[test]
fn children_under_a_slotless_component_are_refused_before_they_are_copied() {
    let mut source = nested_view_source(255);
    source.push_str(&" ".repeat(4 + 255 * 2));
    source.push_str("P1()\n");
    for component in 1..=29 {
        source.push_str(&format!(
            "component P{component}\n  view\n    {}()\n",
            if component == 29 {
                "Host".to_owned()
            } else {
                format!("P{}", component + 1)
            }
        ));
    }
    source.push_str("component Host\n  view\n    Plain()\n");
    for level in 0..255 {
        source.push_str(&" ".repeat(6 + level * 2));
        source.push_str("column\n");
    }
    source.push_str("component Plain\n  view\n    text \"ok\"\n");
    assert_small_stack(vec![(source, Some("syntax-no-slot"))]);
}

#[test]
fn recursive_empty_slot_fills_are_bounded() {
    let mut source = String::from(
        "component App\n  view\n    Recursive()\ncomponent Recursive\n  slot\n  view\n",
    );
    for level in 0..128 {
        source.push_str(&" ".repeat(4 + level * 2));
        source.push_str("column\n");
    }
    let indent = " ".repeat(4 + 128 * 2);
    source.push_str(&format!("{indent}Recursive()\n{indent}Recursive()\n"));
    let started = std::time::Instant::now();
    assert_small_stack(vec![(source, Some("syntax-inline-depth"))]);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "empty recursive fills took {:?}",
        started.elapsed()
    );
}

// Run in a separate test process for each pass/depth: a stack overflow aborts
// the process, so a caller can binary-search the last successful depth.
#[test]
#[ignore = "manual stack-depth probe"]
fn probe_view_pass_on_a_small_stack() {
    let Ok(pass) = std::env::var("EXACT_STACK_PASS") else {
        return;
    };
    let sites: usize = std::env::var("EXACT_STACK_DEPTH").unwrap().parse().unwrap();
    let stack_kib: usize =
        std::env::var("EXACT_STACK_KIB").map_or(2048, |value| value.parse().unwrap());
    let source = match std::env::var("EXACT_STACK_KIND").as_deref() {
        Ok("match") => nested_branch_source(sites, "match", false),
        Ok("when") => nested_branch_source(sites, "when", false),
        Ok("slot") => deep_slot_fill_source(),
        _ => nested_view_source(sites),
    };
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(move || {
            let small = |f: &mut (dyn FnMut() + Send)| {
                std::thread::scope(|scope| {
                    std::thread::Builder::new()
                        .stack_size(stack_kib << 10)
                        .spawn_scoped(scope, f)
                        .unwrap()
                        .join()
                        .unwrap();
                });
            };
            if pass == "parse" {
                small(&mut || {
                    contract_syntax::parse(&source).unwrap();
                });
                return;
            }
            let file = contract_syntax::parse(&source).unwrap();
            if pass == "inline" {
                small(&mut || {
                    contract_syntax::inline(&file).unwrap();
                });
                return;
            }
            if pass == "types" {
                small(&mut || {
                    contract_types::check_all(&file, false, contract_lower::tags::style, None)
                        .unwrap();
                });
                return;
            }
            let checked =
                contract_types::check_all(&file, false, contract_lower::tags::style, None).unwrap();
            if pass == "analyze" {
                small(&mut || {
                    contract_analyze::check_all(&checked).unwrap();
                });
                return;
            }
            let analysis = contract_analyze::check_all(&checked).unwrap();
            if pass == "lower" {
                small(&mut || {
                    contract_lower::lower_all(
                        &checked,
                        &analysis,
                        None,
                        false,
                        contract_lower::Profile::Web,
                    )
                    .unwrap();
                });
                return;
            }
            if pass == "plan" {
                let plan = contract_lower::lower_all(
                    &checked,
                    &analysis,
                    None,
                    false,
                    contract_lower::Profile::Web,
                )
                .unwrap()
                .0;
                small(&mut || {
                    exact_plan::builder::PlanBuilder::from_plan(plan.clone())
                        .finish()
                        .unwrap();
                });
                return;
            }
            panic!("unknown stack pass {pass}");
        })
        .unwrap()
        .join()
        .unwrap();
}
