//! @ref LLP 1038 D2/D3, D5/D6 — compiled declarations, visits and keyed screens.
use exact_kernel::{Kernel, PropId};
use exact_plan::Plan;
use exact_runner::{agent, DataError, DataSource, Runner, Value};
use serde_json::{json, Value as Json};

const SOURCE: &str = include_str!("../../../corpus/routes.contract");

#[derive(Default)]
struct Data {
    asked: Vec<Vec<Value>>,
}
impl DataSource for Data {
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        match name {
            "loadRouter" => Ok(Value::record(vec![Value::str("own action")])),
            "loadQuestions" => {
                self.asked.push(args.to_vec());
                Ok(args[0].clone())
            }
            "loadPosts" => Ok(Value::list(
                [
                    ["a", "Post A: The first perspective", "p"],
                    ["b", "Post B: A different perspective", "p"],
                    ["c", "Post C: The last perspective", "q"],
                ]
                .into_iter()
                .map(|row| Value::record(row.into_iter().map(Value::str).collect()))
                .collect(),
            )),
            "loadPeople" => Ok(Value::list(
                [
                    ["p", "Person P: Pat Rivera", "b"],
                    ["q", "Person Q: Quinn Lee", "c"],
                ]
                .into_iter()
                .map(|row| Value::record(row.into_iter().map(Value::str).collect()))
                .collect(),
            )),
            _ => Err(DataError::UnknownSource(name.into())),
        }
    }
}
fn boot(plan: Plan, launch: &str) -> Runner<Data> {
    Runner::boot(
        plan,
        Data::default(),
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
    .unwrap()
}
fn state(r: &Runner<Data>) -> Json {
    serde_json::from_str(&agent::state(r)).unwrap()
}
fn selected(nav: &Json) -> &Vec<Json> {
    nav["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == nav["tab"])
        .unwrap()["stack"]
        .as_array()
        .unwrap()
}
fn rows(r: &Runner<Data>) -> Vec<(String, u32)> {
    let root = r.kernel().roots()[0];
    r.kernel()
        .node(root)
        .unwrap()
        .children()
        .iter()
        .filter_map(|id| {
            let node = r.kernel().node(*id).unwrap();
            Some((node.props.str(PropId::NavigationKey)?.into(), *id))
        })
        .collect()
}
fn assert_rows(r: &Runner<Data>) {
    let s = state(r);
    let entries = selected(&s["slots"]["nav"]);
    let ids: Vec<_> = entries.iter().map(|e| e["id"].to_string()).collect();
    assert_eq!(
        rows(r).into_iter().map(|(key, _)| key).collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn interview_table_shapes_roundtrip_and_launch_fill() {
    let plan = contract::compile(SOURCE).unwrap();
    assert_eq!(plan.encode(), contract::compile(SOURCE).unwrap().encode());
    let plan = Plan::decode(&plan.encode()).unwrap();
    assert_eq!(plan.str(plan.slot(plan.router.unwrap()).name), "nav");
    assert!(plan.slot(plan.router.unwrap()).owner.is_none());
    for (name, fields) in [
        ("Router", vec!["tab", "tabs", "next"]),
        ("Tab", vec!["name", "stack"]),
        ("Entry", vec!["id", "name", "url", "tab", "params"]),
        ("Params", vec!["post", "question", "person"]),
    ] {
        let row = plan
            .types
            .iter()
            .find(|t| plan.str(t.name) == name)
            .unwrap();
        let actual: Vec<_> = plan.fields
            [row.fields.start as usize..(row.fields.start + row.fields.len) as usize]
            .iter()
            .map(|f| plan.str(f.name))
            .collect();
        assert_eq!(actual, fields);
    }
    let corpus: Json =
        serde_json::from_str(include_str!("../../../../route/tests/corpus.json")).unwrap();
    let actual: Vec<_> = plan.routes.iter().map(|r| json!({"name":plan.str(r.name),"pattern":plan.str(r.pattern),"parent":r.parent.map(|p|p.0),"tab":r.tab,"notfound":r.notfound})).collect();
    assert_eq!(json!(actual), corpus["tables"]["interview"]["routes"]);
    let baked = contract::bake(plan, Data::default()).unwrap();
    for (url, names) in [
        ("/", vec!["home"]),
        ("/prompt/5/write", vec!["prompts", "question", "write"]),
    ] {
        let mut r = boot(baked.clone(), url);
        let s = state(&r);
        assert_eq!(s["slots"]["initialUrl"], url);
        assert_eq!(s["derives"]["current"]["url"], url);
        assert_eq!(s["derives"]["count"], names.len());
        assert_eq!(
            selected(&s["slots"]["nav"])
                .iter()
                .map(|e| e["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            names
        );
        assert_rows(&r);
        let change = r.take_router_change().unwrap();
        assert_eq!(change.url, url);
        assert_eq!(json!(change.top), s["derives"]["current"]["id"]);
        assert!(change.removed.is_empty());
        assert!(r.take_router_change().is_none());
        if url == "/" {
            assert!(
                r.data_ref().asked.is_empty(),
                "same baked arguments use the cache"
            );
        } else {
            assert_eq!(
                r.data_ref().asked,
                vec![vec![Value::list(vec![Value::str("5"), Value::str("5")])]]
            );
            assert_eq!(s["resources"]["questions"], json!(["5", "5"]));
        }
    }
    let mut r = boot(baked, "/");
    r.act("openPost", vec![Value::str("a")]).unwrap();
    r.act("push", vec![Value::str("/people/p")]).unwrap();
    r.act("openPost", vec![Value::str("b")]).unwrap();
    assert_eq!(
        r.resource_args("posts"),
        Some([Value::list(vec![Value::str("a"), Value::str("b")])].as_slice())
    );
    assert_eq!(
        r.resource_args("people"),
        Some([Value::list(vec![Value::str("p")])].as_slice())
    );
    let tree = agent::tree(&r);
    for text in [
        "Post A: The first perspective",
        "Person P: Pat Rivera",
        "Post B: A different perspective",
    ] {
        assert!(tree.contains(text), "{tree}");
    }
}

#[test]
fn actions_encode_params_and_keep_row_identity_through_push_back_and_tabs() {
    let mut r = boot(contract::compile(SOURCE).unwrap(), "/");
    r.take_router_change();
    let home = rows(&r)[0].clone();
    r.act("openPost", vec![Value::str("a/b ?%é")]).unwrap();
    let pushed = r.take_router_change().unwrap();
    assert_eq!(pushed.url, "/post/a%2Fb%20%3F%25%C3%A9");
    assert!(pushed.removed.is_empty());
    assert_eq!(state(&r)["derives"]["current"]["params"]["post"], "a/b ?%é");
    assert_eq!(rows(&r)[0], home);
    assert_rows(&r);
    r.act("back", vec![]).unwrap();
    let back = r.take_router_change().unwrap();
    assert_eq!(back.url, "/");
    assert_eq!(back.removed, vec![pushed.top]);
    assert_eq!(rows(&r), vec![home]);
    r.act("selectTab", vec![Value::str("prompts")]).unwrap();
    assert_eq!(r.take_router_change().unwrap().url, "/prompts");
    assert_eq!(state(&r)["slots"]["nav"]["tab"], "prompts");
    r.act("openQuestion", vec![Value::Number(5.0)]).unwrap();
    assert_eq!(r.take_router_change().unwrap().url, "/prompt/5");
    r.act("replace", vec![Value::str("/prompt/5?q=hello+world")])
        .unwrap();
    assert_eq!(state(&r)["derives"]["query"], "hello world");
    assert_rows(&r);
    r.act("selectTab", vec![Value::str("home")]).unwrap();
    assert_eq!(r.take_router_change().unwrap().url, "/");
    r.act("selectTab", vec![Value::str("prompts")]).unwrap();
    assert_eq!(
        r.take_router_change().unwrap().url,
        "/prompt/5?q=hello+world"
    );
    assert_rows(&r);
}

#[test]
fn corpus_verb_sequences_run_through_compiled_actions_without_copying_expectations() {
    let corpus: Json =
        serde_json::from_str(include_str!("../../../../route/tests/corpus.json")).unwrap();
    let mut failures = Vec::new();
    for sequence in corpus["sequences"].as_array().unwrap() {
        let table = &corpus["tables"][sequence["table"].as_str().unwrap()]["routes"];
        let table = table.as_array().unwrap();
        let mut source = "routes nav\n".to_string();
        for row in table {
            let mut depth = 1;
            let mut parent = row["parent"].as_u64();
            while let Some(p) = parent {
                depth += 1;
                parent = table[p as usize]["parent"].as_u64();
            }
            source.push_str(&"  ".repeat(depth));
            if row["notfound"] == true {
                source.push_str("notfound\n");
                continue;
            }
            if row["tab"] == true {
                source.push_str("tab ");
            }
            source.push_str(&format!(
                "{} {}\n",
                row["name"].as_str().unwrap(),
                row["pattern"]
            ));
        }
        source.push_str("component App\n");
        for verb in ["open", "push", "replace", "back", "select", "go"] {
            let (param, arg) = if verb == "back" {
                ("", "")
            } else {
                ("(url: string)", ", url")
            };
            source.push_str(&format!(
                "  action {verb}{param}\n    nav = {verb}(nav{arg})\n"
            ));
        }
        source.push_str("  view\n    text top(nav).url\n");
        let plan = contract::compile(&source).unwrap();
        let steps = sequence["steps"].as_array().unwrap();
        let mut r = boot(
            Plan::decode(&plan.encode()).unwrap(),
            steps[0]["arg"].as_str().unwrap(),
        );
        for (index, step) in steps.iter().enumerate() {
            if index > 0 {
                let args = step["arg"]
                    .as_str()
                    .map(|s| vec![Value::str(s)])
                    .unwrap_or_default();
                r.act(step["verb"].as_str().unwrap(), args).unwrap();
            }
            if state(&r)["slots"]["nav"] != step["expected"] {
                failures.push(format!(
                    "{} step {index}: {}",
                    sequence["name"],
                    agent::state(&r)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn route_diagnostics_and_dynamic_location_boundary() {
    let base = "routes nav\n  home \"/\"\n    post \"/post/:post\"\ncomponent App\n  action visit(id: string)\n    nav = push(nav, DEST)\n  view\n    text top(nav).url\n";
    for (expr, id) in [
        ("path(\"post\")", "route-unknown"),
        ("path(\"post\", id, id)", "route-unknown"),
        ("path(id)", "route-unknown"),
        ("`/post/${id}`", "route-template"),
        ("\"/missing\"", "route-no-match"),
    ] {
        let error = contract::compile(&base.replace("DEST", expr)).unwrap_err();
        assert_eq!(error.id, id, "{expr}: {error}");
        if id == "route-template" {
            assert_eq!(error.message, "use `path()`");
        }
    }
    for expr in [
        "id",
        "id + \"?q=yes\"",
        "(id == \"/\" ? id : \"/post/1\")",
        "path(\"post\", id)",
    ] {
        contract::compile(&base.replace("DEST", expr)).unwrap();
    }
    let fallback = base
        .replace("component App", "  notfound\ncomponent App")
        .replace("DEST", "\"/missing\"");
    assert_eq!(
        contract::compile(&fallback).unwrap_err().id,
        "route-no-match"
    );
    let dynamic = fallback.replace("push(nav, \"/missing\")", "push(nav, id)");
    let mut r = boot(contract::compile(&dynamic).unwrap(), "/");
    r.act("visit", vec![Value::str("/missing")]).unwrap();
    assert_eq!(state(&r)["derives"], json!({}));
    assert_eq!(
        selected(&state(&r)["slots"]["nav"]).last().unwrap()["name"],
        "notfound"
    );
    assert_eq!(
        contract::compile(
            &base
                .replace("DEST", "path(\"post\", id)")
                .replace("top(nav).url", "top(nav).params.missing")
        )
        .unwrap_err()
        .id,
        "type-unknown-field"
    );
    assert_eq!(
        contract::compile(&format!("routes other\n  first \"/\"\n{base}"))
            .unwrap_err()
            .id,
        "route-duplicate"
    );
    for name in ["Router", "Tab", "Entry", "Params"] {
        assert_eq!(
            contract::compile(&format!(
                "shape {name}\n  a: string\n{}",
                base.replace("DEST", "id")
            ))
            .unwrap_err()
            .id,
            "type-shape-reserved"
        );
    }
    assert_eq!(
        contract::compile(
            &base
                .replace("DEST", "id")
                .replace("component App", "component App\n  state nav = 1")
        )
        .unwrap_err()
        .id,
        "type-duplicate-name"
    );
}

#[test]
fn imported_routes_cannot_claim_a_root_and_root_routes_survive_component_lifting() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/router-tmp/imports-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let app = dir.join("app.contract");
    std::fs::write(
        &app,
        "use Shared from \"./shared.contract\"\ncomponent App\n  view\n    text \"root\"\n",
    )
    .unwrap();
    for body in [
        "shape Shared\n  label: string\n",
        "component Shared\n  view\n    text \"child\"\n",
    ] {
        std::fs::write(
            dir.join("shared.contract"),
            format!("routes nav\n  home \"/\"\n{body}"),
        )
        .unwrap();
        assert_eq!(
            contract::compile_path(&app).unwrap_err().id,
            "analyze-routes-not-root"
        );
    }
    let child = "component Screen\n  props\n    entry: Entry\n  state draft = entry.url\n  action edit(value: string)\n    draft = value\n  view\n    input value=draft change=edit testId=`draft-${entry.id}`\n";
    std::fs::write(dir.join("shared.contract"), child).unwrap();
    let source = SOURCE.replace(
        "          button appearance=\"none\" id=\"back\" press=back",
        "          Screen(entry=e)\n          button appearance=\"none\" id=\"back\" press=back",
    );
    std::fs::write(
        &app,
        format!("use Screen from \"./shared.contract\"\n{source}"),
    )
    .unwrap();
    let mut r = boot(contract::compile_path(&app).unwrap(), "/");
    assert_eq!(
        r.plan().slots.iter().filter(|s| s.owner.is_some()).count(),
        1
    );
    let home = r.kernel().find_by_test_id("draft-0")[0];
    let node = r.kernel().node_by_key(home).unwrap().id;
    r.dispatch(node, exact_runner::Event::Change("kept draft".into()))
        .unwrap();
    r.act("openPost", vec![Value::str("1")]).unwrap();
    assert_eq!(
        r.kernel()
            .node_by_key(home)
            .unwrap()
            .props
            .str(PropId::Value),
        Some("kept draft")
    );
    assert_rows(&r);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ordinary_action_references_keep_precedence_and_router_first_arguments_select_roster() {
    let source = r#"routes nav
  home "/"
    post "/post/:post"
component App
  state label = ""
  action open(value: string)
    label = value
  action visit
    nav = push(nav, "/post/1")
  action visitOther
    nav = push(nav, "/post/2")
  view
    main
      button press=open(top(nav).url) testId="read"
        text "Read"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}`
          text e.url
"#;
    let mut r = boot(contract::compile(source).unwrap(), "/");
    r.act("visit", vec![]).unwrap();
    let prior = rows(&r);
    // A push of the location on top is no new visit (LLP 1038 `push`).
    r.act("visit", vec![]).unwrap();
    assert_eq!(rows(&r), prior);
    r.act("visitOther", vec![]).unwrap();
    r.act("visit", vec![]).unwrap();
    assert_rows(&r);
    assert_eq!(&rows(&r)[..2], prior.as_slice());
    assert_ne!(
        rows(&r)[1].0,
        rows(&r)[3].0,
        "repeated URLs have distinct entry keys"
    );
    let button = r.kernel().find_by_test_id("read")[0];
    let id = r.kernel().node_by_key(button).unwrap().id;
    r.dispatch(id, exact_runner::Event::Press).unwrap();
    assert_eq!(r.slot("label"), Some(&Value::str("/post/1")));
    let child_source = r#"routes nav
  home "/"
    post "/post/:post"
component App
  state label = ""
  action open(value: string)
    label = value
  action commit(value: Router)
    nav = value
  view
    Child(nav=nav, open=open, commit=commit)
component Child
  props
    nav: Router
    open: action
    commit: action
  view
    column
      button press=open("prop called") testId="prop"
        text "Prop"
      button press=commit(open(nav, path("post", "42"))) testId="verb"
        text "Visit"
"#;
    let mut child = boot(contract::compile(child_source).unwrap(), "/");
    for id in ["prop", "verb"] {
        let key = child.kernel().find_by_test_id(id)[0];
        let node = child.kernel().node_by_key(key).unwrap().id;
        child.dispatch(node, exact_runner::Event::Press).unwrap();
    }
    assert_eq!(child.slot("label"), Some(&Value::str("prop called")));
    assert_eq!(child.take_router_change().unwrap().url, "/post/42");
    // A Router first argument wins even over a compatible typed action.
    let typed = source
        .replace("action open(value: string)", "action open(value: Router)")
        .replace("label = value", "label = top(value).url")
        .replace("open(top(nav).url)", "open(nav)");
    assert_eq!(contract::compile(&typed).unwrap_err().id, "type-arity");
    for name in [
        "open",
        "push",
        "replace",
        "select",
        "go",
        "back",
        "stack",
        "top",
        "depth",
        "params",
        "searchParam",
        "encodeURIComponent",
    ] {
        // An app's `fn` of a router verb's name shadows it.
        let source = format!(
            "fn {name}(value: string): string = value\ncomponent App\n  view\n    text {name}(\"hello\")\n"
        );
        contract::compile(&source).unwrap();
    }
    let plan = contract::compile("component App\n  view\n    text \"hello\"\n").unwrap();
    assert!(plan.router.is_none() && plan.routes.is_empty());
    assert!(!plan
        .types
        .iter()
        .any(|t| matches!(plan.str(t.name), "Router" | "Tab" | "Entry" | "Params")));
}

#[test]
fn boot_root_requires_a_pattern_or_fallback_and_tab_can_name_a_tab() {
    let source =
        "routes nav\n  tab tab \"/x\"\n  notfound\ncomponent App\n  view\n    text top(nav).url\n";
    let mut r = boot(contract::compile(source).unwrap(), "/");
    assert_eq!(r.take_router_change().unwrap().url, "/");
    assert_eq!(selected(&state(&r)["slots"]["nav"])[0]["name"], "tab");
    let mut r = boot(contract::compile(source).unwrap(), "/x");
    assert_eq!(selected(&state(&r)["slots"]["nav"]).len(), 1);
    assert_eq!(r.take_router_change().unwrap().url, "/x");
    let error = contract::compile(&source.replace("  notfound\n", "")).unwrap_err();
    assert_eq!(error.id, "route-root");
    assert_eq!(
        error.message,
        "the first tab's root must be `/`, or declare `notfound`"
    );
}

#[test]
fn router_roster_requires_routes_but_ordinary_shapes_and_actions_remain_available() {
    let source = "shape Router\n  label: string\nshape Entry\n  label: string\ncomponent App\n  resource value = loadRouter() as shape Router\n  resource entry = loadEntry() as shape Entry\n  derive result = EXPR\n  view\n    text \"home\"\n";
    for (name, expr) in [
        ("open", "open(value, \"/\")"),
        ("push", "push(value, \"/\")"),
        ("replace", "replace(value, \"/\")"),
        ("go", "go(value, \"/\")"),
        ("select", "select(value, \"home\")"),
        ("back", "back(value)"),
        ("stack", "stack(value)"),
        ("top", "top(value)"),
        ("depth", "depth(value)"),
        ("params", "params(value, \"id\")"),
        ("searchParam", "searchParam(entry, \"q\")"),
    ] {
        let error = contract::compile(&source.replace("EXPR", expr)).unwrap_err();
        assert_eq!(error.id, "type-unknown-function", "{name}: {error}");
        assert_eq!(error.message, format!("declare `routes` to use `{name}`"));
    }
    contract::compile(&source.replace("EXPR", "value.label")).unwrap();
    contract::compile(&source.replace("EXPR", "encodeURIComponent(value.label)")).unwrap();
    let source = "shape Router\n  label: string\ncomponent App\n  state label = \"\"\n  resource value = loadRouter() as shape Router\n  action open(value: Router)\n    label = value.label\n  view\n    button press=open(value) testId=\"own\"\n      text \"Open\"\n";
    let mut r = boot(contract::compile(source).unwrap(), "/");
    let key = r.kernel().find_by_test_id("own")[0];
    let node = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(node, exact_runner::Event::Press).unwrap();
    assert_eq!(r.slot("label"), Some(&Value::str("own action")));
}

#[test]
fn numeric_path_parameters_use_javascript_exponent_boundaries() {
    let source = "routes nav\n  home \"/\"\n    item \"/item/:item\"\ncomponent App\n  action visit(id: number)\n    nav = push(nav, path(\"item\", id))\n  view\n    text top(nav).url\n";
    let mut r = boot(contract::compile(source).unwrap(), "/");
    for (value, url, param) in [
        (1e21, "/item/1e%2B21", "1e+21"),
        (1e20, "/item/100000000000000000000", "100000000000000000000"),
        (1e-6, "/item/0.000001", "0.000001"),
        (1e-7, "/item/1e-7", "1e-7"),
    ] {
        r.act("visit", vec![Value::Number(value)]).unwrap();
        assert_eq!(r.take_router_change().unwrap().url, url);
        assert_eq!(
            selected(&state(&r)["slots"]["nav"]).last().unwrap()["params"]["item"],
            param
        );
    }
}

#[test]
fn path_refuses_empty_and_dot_only_parameters_before_fallback_or_slot_writes() {
    let source = "routes nav\n  home \"/\"\n    item \"/item/:item\"\n  notfound\ncomponent App\n  state marker = 0\n  action visit(id: string)\n    marker = marker + 1\n    nav = push(nav, path(\"item\", id))\n  view\n    text top(nav).url\n";
    let mut r = boot(contract::compile(source).unwrap(), "/");
    r.take_router_change();
    let before = r.slot("nav").cloned();
    for value in ["", ".", ".."] {
        let literal = source.replace("path(\"item\", id)", &format!("path(\"item\", {value:?})"));
        assert_eq!(contract::compile(&literal).unwrap_err().id, "route-unknown");
        assert!(r.act("visit", vec![Value::str(value)]).is_err());
        assert_eq!(r.slot("nav"), before.as_ref());
        assert_eq!(r.slot("marker"), Some(&Value::Number(0.0)));
        assert!(r.take_router_change().is_none());
        assert!(r.journal().any(|line| line
            .contains("router path refused: a path parameter cannot be empty, `.` or `..`")));
        assert!(!r.is_poisoned());
    }
    for (value, url) in [
        ("a.b", "/item/a.b"),
        ("%2E", "/item/%252E"),
        ("a/b", "/item/a%2Fb"),
    ] {
        r.act("visit", vec![Value::str(value)]).unwrap();
        assert_eq!(r.take_router_change().unwrap().url, url);
        assert_eq!(
            selected(&state(&r)["slots"]["nav"]).last().unwrap()["params"]["item"],
            value
        );
    }
}

#[test]
fn navigate_delivers_one_location_or_lets_the_action_ignore_it() {
    // @ref LLP 1038 D8/D11 — the ordinary dispatch, including its journal.
    let mut runner = boot(contract::compile(SOURCE).unwrap(), "/");
    let root = runner.kernel().roots()[0];
    runner
        .dispatch(
            root,
            exact_runner::Event::Navigate("/post/42?q=hello".into()),
        )
        .unwrap();
    assert_eq!(
        state(&runner)["derives"]["current"]["url"],
        "/post/42?q=hello"
    );
    let logs = agent::logs(&runner, 0);
    assert_eq!(logs.matches("navigate view").count(), 1, "{logs}");
    let source = SOURCE.replace("navigate=followLink", "navigate=home");
    let mut runner = boot(contract::compile(&source).unwrap(), "/post/42");
    let root = runner.kernel().roots()[0];
    runner
        .dispatch(root, exact_runner::Event::Navigate("/people/7".into()))
        .unwrap();
    assert_eq!(state(&runner)["derives"]["current"]["url"], "/");
    for source in [
        SOURCE.replace(
            "action followLink(url: string)",
            "action followLink(url: number)",
        ),
        SOURCE.replace("navigate=followLink", "navigate=followLink(\"/\")"),
        SOURCE
            .replace("navigate=followLink", "navigate=home")
            .replace(
                "button appearance=\"none\" id=\"back\"",
                "button appearance=\"none\" navigate=home id=\"back\"",
            ),
    ] {
        assert!(contract::compile(&source).is_err());
    }
}

#[test]
fn a_route_is_a_child_of_its_root_or_of_a_tabpanel_and_nowhere_else() {
    // @ref LLP 1038 D6, LLP 1075.003 §3.7 — where every host finds routes
    // (hn-reader F5: a route behind a centering column was ignored).
    let tabs = r#"routes nav
  tab home "/"
    item "/item"
  tab saved "/saved"
component App
  action back
    nav = back(nav)
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" display="flex" flex-direction="column"
      column flex=1 position="relative"
        each t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" top=0 right=0 bottom=0 left=0
            STACK
      row role="tablist"
        button role="tab" aria-controls="panel-home" press=pick("home")
          text "Home"
        button role="tab" aria-controls="panel-saved" press=pick("saved")
          text "Saved"
"#;
    let stack = "each e in t.stack key=e.id\n              column navigationKey=`${e.id}` position=\"absolute\" inset=0\n                text e.url";
    contract::compile(&tabs.replace("STACK", stack)).unwrap();
    // A route inside a route is shown by no host.
    let nested = stack.replace("text e.url", "column navigationKey=\"inner\"");
    let error = contract::compile(&tabs.replace("STACK", &nested)).unwrap_err();
    assert_eq!(error.id, "lower-route-place", "{error}");
    assert!(error.message.contains("inside another route"), "{error}");
    // Behind a wrapper inside a panel: the message names the path.
    let wrapped = "column\n              each e in t.stack key=e.id\n                column navigationKey=`${e.id}`";
    let error = contract::compile(&tabs.replace("STACK", wrapped)).unwrap_err();
    assert_eq!(error.id, "lower-route-place", "{error}");
    assert!(
        error
            .message
            .contains("`main > column > column > column > column`"),
        "{error}"
    );
    // A key outside any root is not a route: below a root that carries
    // none, nothing is a navigation root (LLP 1116 D6 makes the first root
    // with a key one).
    contract::compile(
        "component App\n  view\n    column\n      column navigationKey=\"a\"\n        column navigationKey=\"b\"\n",
    )
    .unwrap();
}

#[test]
fn a_root_with_a_key_and_no_back_control_named_is_the_navigation_root() {
    // @ref LLP 1116 D6, LLP 1115 D5 — `navigationBack` is optional: the
    // first root with a `navigationKey` is the navigation root, naming the
    // empty id, which no control carries, so the platform's Back is the
    // router's own (the unit-converter builder lost the native bar to it).
    let source = r#"routes nav
  home "/"
    detail "/detail"
component App
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          text e.url
"#;
    let mut r = boot(contract::compile(source).unwrap(), "/detail");
    let root = r.kernel().roots()[0];
    let props = &r.kernel().node(root).unwrap().props;
    assert_eq!(props.str(PropId::NavigationBack), Some(""));
    assert_rows(&r);
    let top = selected(&state(&r)["slots"]["nav"]).last().unwrap()["id"]
        .as_u64()
        .unwrap();
    assert!(
        r.host_back(top).unwrap().is_some(),
        "the system Back pops the router"
    );
    assert_eq!(selected(&state(&r)["slots"]["nav"]).len(), 1);
    assert_rows(&r);
    // Routes now sit where every host finds them, or are refused.
    let wrapped = source.replace(
        "      each e in stack(nav) key=e.id\n        column navigationKey",
        "      column\n        each e in stack(nav) key=e.id\n          column navigationKey",
    );
    let error = contract::compile(&wrapped).unwrap_err();
    assert_eq!(error.id, "lower-route-place", "{error}");
    // A named control is the author's, as before.
    let named = contract::compile(&source.replace(
        "main navigationKey=`${top(nav).id}`",
        "main navigationKey=`${top(nav).id}` navigationBack=\"back\"",
    ))
    .unwrap();
    let r = boot(named, "/detail");
    let root = r.kernel().roots()[0];
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::NavigationBack),
        Some("back")
    );
}

#[test]
fn a_route_scroll_names_an_element_of_the_route_that_scrolls_on_y() {
    // @ref LLP 1075.003 §3.5 — `navigationScroll` names the route's content
    // scroller by HTML id; refused only where that provably fails.
    let source = r#"routes nav
  home "/"
component App
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` NAMED position="absolute" inset=0 display="flex" flex-direction="column"
          header
            text "Title"
          BODY

component Pane
  props
    name: string
  view
    scroll id=name flex=1 min-height=0
      text "row"
"#;
    let compile = |named: &str, body: &str| {
        contract::compile(&source.replace("NAMED", named).replace("BODY", body))
    };
    let named = "navigationScroll=\"list\"";
    let accepted = [
        "scroll id=\"list\" flex=1 min-height=0\n            text \"row\"",
        "column id=\"list\" flex=1 min-height=0 overflow-y=\"auto\"",
        // CSS: a visible y beside a scrolling x computes to auto.
        "column id=\"list\" flex=1 min-height=0 overflow-x=\"hidden\"",
        // What the compiler cannot see through is given the benefit.
        "column id=e.url",
        "column id=\"list\" overflow-y=(e.url == \"/\" ? \"auto\" : \"hidden\")",
        "when e.url == \"/\"\n            scroll id=\"list\" flex=1 min-height=0",
        "Pane(name=\"list\")",
        // A computed id may name the scroller beside a static still box.
        "column id=\"list\"\n          scroll id=e.url flex=1 min-height=0",
        "scroll id=\"list\" flex=1 min-height=0 overflow-x=\"hidden\"",
    ];
    for body in accepted {
        compile(named, body).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
    // Not literal, or empty: nothing to check.
    compile("navigationScroll=e.url", "column").unwrap();
    compile("navigationScroll=\"\"", "column").unwrap();
    // No element carries the id.
    let error = compile(named, "scroll id=\"lst\" flex=1 min-height=0").unwrap_err();
    assert_eq!(error.id, "lower-route-scroll", "{error}");
    assert!(error.message.contains("names no element"), "{error}");
    // The element that does never scrolls on y.
    for body in [
        "column id=\"list\" flex=1",
        "column id=\"list\" flex=1 overflow-y=\"hidden\" overflow-x=\"auto\"",
        "column id=\"list\" overflow=\"hidden\"",
        // A `scroll` or `list` whose y a literal row stills.
        "scroll id=\"list\" flex=1 min-height=0 overflow=\"hidden\"",
        "list id=\"list\" flex=1 min-height=0 overflow-y=\"hidden\"",
    ] {
        let error = compile(named, body).unwrap_err();
        assert_eq!(error.id, "lower-route-scroll", "{body}: {error}");
        assert!(error.message.contains("never scrolls on y"), "{error}");
    }
}
