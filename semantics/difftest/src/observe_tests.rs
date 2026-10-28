use crate::{compile, observe, oracle::Oracle, script};

#[test]
fn trace_names_due_work_and_only_handlers_that_reached_the_action() {
    let scripts = script::scripted(
        "trace",
        include_str!("../../corpus/trace/tasks.contract"),
        None,
    )
    .unwrap();
    let traces: Vec<Vec<String>> = scripts
        .iter()
        .map(|s| {
            let plan = compile(&s.case).unwrap();
            let oracle = Oracle::for_case(&plan, &s.case);
            let (lines, _) = observe::run(plan, oracle, &s.case.events);
            lines
                .into_iter()
                .filter(|l| l.starts_with("ran ") || l.starts_with("outcome "))
                .collect()
        })
        .collect();
    assert_eq!(
        traces[0],
        [
            "outcome ok",
            "outcome refused",
            "ran task refuse refused",
            "outcome refused",
            "ran task refuse refused",
            "ran handler add ok",
            "outcome ok",
        ]
    );
    assert_eq!(
        traces[1],
        [
            "outcome ok",
            "outcome refused",
            "ran task refuse refused",
            "outcome refused",
            "ran task refuse refused",
            "outcome ok",
            "ran task add ok",
            "ran task add ok",
        ]
    );
    assert_eq!(
        traces[2],
        [
            "outcome ok",
            "outcome refused",
            "outcome ok",
            "outcome refused",
            "outcome ok"
        ]
    );
}

#[test]
fn queued_commits_name_the_mutation_and_then_names_the_action() {
    let scripts = script::scripted(
        "trace",
        include_str!("../../corpus/trace/queue.contract"),
        None,
    )
    .unwrap();
    let s = &scripts[0];
    let plan = compile(&s.case).unwrap();
    let oracle = Oracle::for_case(&plan, &s.case);
    let (lines, _) = observe::run(plan, oracle, &s.case.events);
    let ran: Vec<_> = lines
        .iter()
        .filter(|l| l.starts_with("ran "))
        .map(String::as_str)
        .collect();
    assert_eq!(
        ran,
        [
            "ran handler save ok",
            "ran then landed ok",
            "ran queue saved ok",
            "ran then landed ok"
        ]
    );
}

#[test]
fn land_has_no_timer_commit_and_an_unmodeled_receipt_is_marked() {
    let source = include_str!("../../corpus/properties/land.contract");
    let s = script::scripted("land", source, None).unwrap().remove(0);
    crate::expectations(&s).unwrap();
    let plan = compile(&s.case).unwrap();
    let (lines, _) = observe::run(
        plan.clone(),
        Oracle::for_case(&plan, &s.case),
        &s.case.events,
    );
    let steps = crate::chunks(&lines);
    assert!(steps[2].contains(&"ran then landed ok".into()));
    assert!(!steps[2].iter().any(|l| l.starts_with("ran task")));
    assert!(!steps[3].iter().any(|l| l.starts_with("ran ")));
    assert!(steps[4].contains(&"ran task tick ok".into()));

    let source = "component App\n  view\n    time datetime=0 format=\"timer\" testId=\"time\"\n\ntest \"tick\"\n  clock +1000\n";
    let s = script::scripted("time", source, None).unwrap().remove(0);
    let plan = compile(&s.case).unwrap();
    let (lines, _) = observe::run(
        plan.clone(),
        Oracle::for_case(&plan, &s.case),
        &s.case.events,
    );
    assert!(crate::chunks(&lines)[1].contains(&observe::OMITTED_COMMIT.into()));
    assert!(!lines.iter().any(|l| l.starts_with("ran ")));
}

#[test]
fn curried_refusal_names_the_action_without_claiming_it_ran() {
    let s = script::scripted(
        "trace",
        include_str!("../../corpus/trace/tasks.contract"),
        None,
    )
    .unwrap()
    .remove(2);
    let plan = compile(&s.case).unwrap();
    let (lines, _) = observe::run(
        plan.clone(),
        Oracle::for_case(&plan, &s.case),
        &s.case.events,
    );
    assert!(lines
        .iter()
        .any(|l| l.contains("HandlerArguments { action: \"take\"")));
    assert!(!lines.iter().any(|l| l.starts_with("ran handler take")));
}

#[test]
fn typing_uses_the_inputs_handler_before_falling_back_to_change() {
    let source = r#"component App
  state note = ""
  action edit(value)
    note = value
  view
    column
      input testId="input" value=note input=edit
      input testId="change" value=note change=edit
test "type in both fields"
  type "input" "first"
  expect state note == "first"
  type "change" "second"
  expect state note == "second"
"#;
    let s = script::scripted("input", source, None).unwrap().remove(0);
    let plan = compile(&s.case).unwrap();
    let (lines, _) = observe::run(
        plan.clone(),
        Oracle::for_case(&plan, &s.case),
        &s.case.events,
    );
    crate::expectations_on(&s, &lines).unwrap();
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.as_str() == "ran handler edit ok")
            .count(),
        2
    );
}
