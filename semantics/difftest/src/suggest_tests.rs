use super::*;
use crate::{observe, oracle::Oracle, script};
use std::path::PathBuf;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn unreached_corpus_writes_filter_or_widen_observed_candidates() {
    for (fixture, false_line, replacement) in [
        ("frame", "only writes n", Some("only writes hidden n")),
        ("then", "expect state phase in \"idle\" \"ready\"", None),
        ("parameter", "expect state mode in \"idle\" \"ready\"", None),
        ("stays", "expect state ready stays true", None),
        ("whole", "expect state n whole", None),
    ] {
        let file = root().join(format!(
            "semantics/corpus/properties/suggest-{fixture}.contract"
        ));
        let source = std::fs::read_to_string(&file).unwrap();
        let plan = contract::compile_path(&file).unwrap();
        let mut suggestions = Suggestions::new(&plan, &file);
        for t in contract::tests(&source).unwrap() {
            let s = script::test_case(
                &file.display().to_string(),
                &source,
                Some(&file),
                &t,
                Some(&plan),
            )
            .unwrap();
            let (lines, _) = observe::run(
                plan.clone(),
                Oracle::for_case(&plan, &s.case),
                &s.case.events,
            );
            suggestions.observe(&lines);
        }
        let before = suggestions.observed_blocks(&[]).join("\n");
        assert!(
            before.lines().any(|l| l.trim() == false_line),
            "{fixture}: {before}"
        );
        let (blocks, receipt) = suggestions.blocks(&[], &file).unwrap();
        let after = blocks.join("\n");
        assert!(
            !after.lines().any(|l| l.trim() == false_line),
            "{fixture}: {after}"
        );
        if let Some(replacement) = replacement {
            assert!(
                after.lines().any(|l| l.trim() == replacement),
                "{fixture}: {after}"
            );
            assert!(receipt.widened > 0);
        } else {
            assert!(receipt.dropped.values().sum::<usize>() > 0);
        }
    }
}

#[test]
fn none_of_the_thirteen_false_d6_blocks_can_print() {
    let cases = [
        ("typetour", "after \"lock\" \"lock\"\n  only writes screen\n", Some("screen thread")),
        ("caltrain", "after \"openDeck\" \"openDeck\"\n  only writes screen\n", Some("deckLoaded deckLoads deckMessage screen")),
        ("caltrain", "after \"selectStation\" \"selectStation\"\n  only writes query screen stationId\n", Some("query screen searchFocused stationId")),
        ("shelf", "after \"openAdd\" \"openAdd\"\n  only writes nav\n", None),
        ("shelf", "after \"back\" \"back\"\n  only writes nav\n", None),
        ("shelf", "after \"tagSaidDone\" \"tagSaidDone\"\n  writes none\n", None),
        ("shelf", "always \"submitted\"\n  expect state submitted stays true\n", None),
        ("excord", "always \"wantFrom\"\n  expect state wantFrom in \"\" \"launch\" \"route\"\n", None),
        ("design-tool", "always \"pairScope\"\n  expect state pairScope in \"propose\" \"read\"\n", None),
        ("design-tool", "always \"canvasView\"\n  expect state canvasView in \"grid\" \"one\"\n", None),
        ("design-tool", "always \"editKind\"\n  expect state editKind in \"\" \"detach-param\" \"remove-token\" \"set-row\" \"set-state\"\n", None),
        ("metronome", "after clock \"clock\"\n  only writes beat\n", Some("beat now playing")),
        ("messages", "always \"flightPhase\"\n  expect state flightPhase in 0 1\n", None),
    ];
    for (app, candidate, widened) in cases {
        let file = root().join(format!("apps/{app}/app.contract"));
        let (blocks, receipt) = check_blocks(vec![candidate.into()], &[], &file).unwrap();
        let original = contract::properties(candidate).unwrap().remove(0);
        assert!(
            !blocks
                .iter()
                .any(|b| already_written(&original, &contract::properties(b).unwrap())),
            "{app}: {candidate}"
        );
        assert_eq!(receipt.widened + receipt.dropped.values().sum::<usize>(), 1);
        if let Some(names) = widened {
            assert!(
                blocks
                    .join("\n")
                    .contains(&format!("only writes {names}\n")),
                "{app}: {blocks:?}"
            );
        }
        println!(
            "D6 false {app} {:?}: {}",
            original.name,
            receipt.details.join("; ")
        );
    }
}

#[test]
fn the_twenty_one_kept_blocks_survive_without_existing_property_suppression() {
    let apps = [
        "agents",
        "caltrain",
        "excord",
        "messages",
        "metronome",
        "photo-editor",
        "reflow",
        "shelf",
        "update-lab",
        "video-player",
    ];
    let count: usize = std::thread::scope(|scope| {
        let runs: Vec<_> = apps
            .into_iter()
            .map(|app| {
                scope.spawn(move || {
                    let file = root().join(format!("apps/{app}/app.contract"));
                    let tests = file.with_extension("test.contract");
                    let text = std::fs::read_to_string(&tests).unwrap();
                    let kept: Vec<_> = contract::properties(&text)
                        .unwrap()
                        .into_iter()
                        .filter(|p| {
                            p.name.contains(" writes only ")
                                || p.name.contains(" is one of ")
                                || p.name == "bpm is a whole number"
                        })
                        .collect();
                    let checks = contract::check_suggestions_path(&file, &kept).unwrap();
                    for (p, check) in kept.iter().zip(&checks) {
                        assert_eq!(
                            *check,
                            contract::SuggestionCheck::Keep,
                            "conservative loss {app}: {}: {check:?}",
                            p.name
                        );
                    }
                    let source = std::fs::read_to_string(&file).unwrap();
                    let plan = contract::compile_path(&file).unwrap();
                    let mut suggestions = Suggestions::new(&plan, &file);
                    let mut scripted = Vec::new();
                    let mut authored = Vec::new();
                    let mut test_files: Vec<_> = std::fs::read_dir(file.parent().unwrap())
                        .unwrap()
                        .filter_map(Result::ok)
                        .map(|e| e.path())
                        .filter(|p| p.to_str().is_some_and(|p| p.ends_with(".test.contract")))
                        .collect();
                    test_files.sort();
                    for path in test_files {
                        for t in contract::tests(&std::fs::read_to_string(&path).unwrap()).unwrap()
                        {
                            authored.push(t.clone());
                            scripted.push(
                                script::test_case(
                                    &format!(
                                        "apps/{app}/{}",
                                        path.file_name().unwrap().to_str().unwrap()
                                    ),
                                    &source,
                                    Some(&file),
                                    &t,
                                    Some(&plan),
                                )
                                .unwrap(),
                            );
                        }
                    }
                    scripted.push(crate::explore(
                        &format!("apps/{app}/app.contract"),
                        &source,
                        Some(&file),
                    ));
                    for s in &scripted {
                        let (lines, _) = observe::run(
                            plan.clone(),
                            Oracle::for_case(&plan, &s.case),
                            &s.case.events,
                        );
                        suggestions.observe(&lines);
                    }
                    let tests = scripted.len() - 1;
                    let base = scripted.last().unwrap().case.clone();
                    let inputs = crate::walk::Inputs::new(&source, authored.into_iter());
                    let mut rng =
                        crate::rng::Rng::new(crate::seed_of(&format!("apps/{app}/app.contract")));
                    for i in 0..20 {
                        let observed =
                            contract::properties(&suggestions.observed_blocks(&[]).join("\n"))
                                .unwrap();
                        if kept.iter().all(|p| already_written(p, &observed)) {
                            break;
                        }
                        let prefix = if i % 2 == 1 && tests > 0 {
                            Some(&scripted[(i / 2) % tests].case)
                        } else {
                            None
                        };
                        let walk = crate::walk::generate(
                            &plan,
                            &base,
                            prefix,
                            rng.next_u64(),
                            i,
                            40,
                            &inputs,
                        );
                        suggestions.observe(&walk.observation);
                    }
                    let (blocks, _) = suggestions.blocks(&[], &file).unwrap();
                    let printed = contract::properties(&blocks.join("\n")).unwrap();
                    for p in &kept {
                        assert!(
                            already_written(p, &printed),
                            "not suggested without existing block: {app}: {}",
                            p.name
                        );
                        println!("D6 kept {app}: {}", p.name);
                    }
                    kept.len()
                })
            })
            .collect();
        runs.into_iter().map(|r| r.join().unwrap()).sum()
    });
    assert_eq!(count, 21);
}
