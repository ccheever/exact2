use crate::{
    compile, observe,
    oracle::Oracle,
    property,
    script::{Case, Event},
    suggest::Suggestions,
    walk,
};

fn make_case(source: &str) -> Case {
    Case {
        name: "a2-regression".into(),
        seed: Some(7),
        source: source.into(),
        events: Vec::new(),
        path: None,
        stored: false,
        prerendered: false,
        launch: Default::default(),
    }
}

#[test]
fn walk_reobserves_controls_replays_and_keeps_prefix_facts() {
    let source = r#"component App
  state opened = false
  state note = ""
  state done = false
  action open
    opened = true
  action edit(value)
    note = value
  action finish
    done = true
  view
    column
      if not opened
        button "Open" testId="open" press=open
      if opened
        input testId="note" value=note input=edit
        button "Finish" testId="finish" press=finish
"#;
    let base = make_case(source);
    let plan = compile(&base).unwrap();
    let tests = contract::tests("test \"entry\"\n  type \"note\" \"authored\"\n").unwrap();
    let inputs = walk::Inputs::new(source, tests.into_iter());
    let walk = walk::generate(&plan, &base, None, 7, 0, 40, &inputs);
    assert!(walk
        .scripted
        .case
        .events
        .contains(&Event::Tap("finish".into())));
    assert!(walk
        .scripted
        .case
        .events
        .contains(&Event::Type("note".into(), "authored".into())));
    assert_eq!(walk.scripted.case.events.len(), 40);
    for (i, event) in walk.scripted.case.events.iter().enumerate() {
        if matches!(event, Event::Tap(_) | Event::Type(..)) {
            assert_eq!(walk.scripted.case.events[i + 1], Event::InputLand);
        }
    }
    let replay = observe::run(
        plan.clone(),
        Oracle::for_case(&plan, &walk.scripted.case),
        &walk.scripted.case.events,
    )
    .0;
    assert_eq!(walk.observation, replay);
    let mut prefix = base.clone();
    prefix.events = vec![Event::Tap("open".into()), Event::InputLand];
    prefix.seed = Some(98);
    prefix.launch.time = Some(exact_runner::time::WallTime {
        epoch_at_zero: 1234.0,
        utc_offset: 0.0,
    });
    let walk = walk::generate(&plan, &base, Some(&prefix), 8, 1, 10, &inputs);
    assert!(walk.scripted.case.events.starts_with(&prefix.events));
    assert_eq!(walk.scripted.case.seed, Some(98));
    assert_eq!(
        walk.scripted.case.launch.time.unwrap().epoch_at_zero,
        1234.0
    );
    assert!(walk.rendered.contains("finish"));
}

#[test]
fn stepwise_and_whole_script_match_on_refusal_and_poison() {
    for file in [
        "trace/tasks.contract",
        "properties/never-then.contract",
        "persist/settings.contract",
    ] {
        let path = crate::leanrun::project().join("corpus").join(file);
        let source = std::fs::read_to_string(&path).unwrap();
        let mut case = make_case(&source);
        case.stored = true;
        case.events = vec![
            Event::Clock(1000.0),
            Event::Tap("poke".into()),
            Event::InputLand,
            Event::Clock(60000.0),
        ];
        let plan = compile(&case).unwrap();
        let expected = observe::run(plan.clone(), Oracle::for_case(&plan, &case), &case.events).0;
        let (mut driver, mut actual) =
            observe::Driver::new(plan.clone(), Oracle::for_case(&plan, &case));
        for event in &case.events {
            actual.extend(driver.step(event));
        }
        assert_eq!(actual, expected, "{file}");
    }
}

#[test]
fn shrinking_keeps_the_property_pair_and_its_oracle() {
    let mut case = make_case(include_str!("../../corpus/properties/holds.contract"));
    case.events = vec![
        Event::Clock(1.0),
        Event::Type("note".into(), "unused".into()),
        Event::InputLand,
        Event::Tap("add".into()),
        Event::InputLand,
        Event::TestClock(1.0),
        Event::Clock(2.0),
    ];
    let plan = compile(&case).unwrap();
    let properties =
        contract::properties("after \"add\" \"bad\"\n  expect state n == 1\n").unwrap();
    let observation = observe::run(plan.clone(), Oracle::for_case(&plan, &case), &case.events).0;
    let shrunk = crate::shrink::runner(
        &plan,
        &case,
        &observation,
        |lines| !property::evaluate(&properties, lines).passed(),
        |_| {},
    );
    assert_eq!(
        shrunk.case.events,
        [
            Event::Tap("add".into()),
            Event::InputLand,
            Event::TestClock(1.0)
        ]
    );
    assert_eq!(shrunk.kept, [3, 4, 5]);
    assert_eq!(shrunk.case.seed, case.seed);
    assert!(shrunk.candidates > 0);
}

#[test]
fn stored_variants_change_the_store_not_the_answers_and_can_falsify_at_boot() {
    let source = "component App\n  state n = 1 persist\n  resource r = query() as shape string\n  view\n    text r\n";
    let mut base = make_case(source);
    let plan = compile(&base).unwrap();
    let properties = contract::properties("always \"initial\"\n  expect state n == 1\n").unwrap();
    let mut found = false;
    for seed in 0..30 {
        base.seed = Some(seed);
        let script = crate::script::Scripted {
            case: base.clone(),
            items: Vec::new(),
            delivery: Default::default(),
        };
        let variants = crate::stored_variants(vec![script]);
        assert_eq!(variants.len(), 2);
        let empty = Oracle::for_case(&plan, &variants[0].case);
        let stored = Oracle::for_case(&plan, &variants[1].case);
        let (a, oa) = observe::run(plan.clone(), empty, &[]);
        let (b, ob) = observe::run(plan.clone(), stored, &[]);
        assert!(property::evaluate(&properties, &a).passed());
        assert_eq!(
            oa.unwrap().transcript.borrow()[0].answer.1,
            ob.unwrap().transcript.borrow()[0].answer.1
        );
        if !property::evaluate(&properties, &b).passed() {
            let shrunk = crate::shrink::runner(
                &plan,
                &variants[1].case,
                &b,
                |s| !property::evaluate(&properties, s).passed(),
                |_| {},
            );
            assert!(shrunk.case.events.is_empty());
            assert!(shrunk.case.stored);
            found = true;
            break;
        }
    }
    assert!(found, "typed stored values must be searched");
}

#[test]
fn suggestions_parse_hold_and_deduplicate_predicates_not_titles() {
    let path = crate::leanrun::project().join("corpus/properties/holds.contract");
    let source = std::fs::read_to_string(&path).unwrap();
    let mut case = make_case(&source);
    let plan = compile(&case).unwrap();
    case.events = vec![
        Event::Tap("add".into()),
        Event::InputLand,
        Event::Tap("add".into()),
        Event::InputLand,
        Event::Clock(1.0),
        Event::Clock(1.0),
    ];
    let observation = observe::run(plan.clone(), Oracle::for_case(&plan, &case), &case.events).0;
    let mut suggestions = Suggestions::new(&plan, &path);
    suggestions.observe(&observation);
    let blocks = suggestions.blocks(&[], &path).unwrap().0;
    let properties = contract::properties(&blocks.join("\n")).unwrap();
    assert!(property::evaluate(&properties, &observation).passed());
    assert!(blocks.iter().any(|b| b.contains("expect state n whole")));
    assert!(blocks.iter().any(|b| b.contains("only writes n")));
    assert!(blocks
        .iter()
        .any(|b| b.contains("after clock") && b.contains("writes none")));
    let existing =
        contract::properties("after \"add\" \"different title\"\n  only writes n\n").unwrap();
    assert!(!suggestions
        .blocks(&existing, &path)
        .unwrap()
        .0
        .iter()
        .any(|b| b.starts_with("after \"add\"")));
    for file in ["cone-clock.contract", "cone-resource.contract"] {
        let path = crate::leanrun::project()
            .join("corpus/properties")
            .join(file);
        let source = std::fs::read_to_string(&path).unwrap();
        let plan = compile(&make_case(&source)).unwrap();
        let mut suggestions = Suggestions::new(&plan, &path);
        let observations = [
            "== boot",
            "outcome ok",
            "derive outer n3ff0000000000000",
            "== clock +1",
            "outcome ok",
            "derive outer n4000000000000000",
        ]
        .map(str::to_string);
        suggestions.observe(&observations);
        assert!(!suggestions
            .blocks(&[], &path)
            .unwrap()
            .0
            .iter()
            .any(|b| b.contains("state outer")));
    }
}
