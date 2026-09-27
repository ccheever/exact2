//! The engine, held to the browser: every sample a real browser recorded
//! for the parity cases (host/web/src/parity.rs) is what the engine computes
//! for the same script, within the declared tolerance. The fixture is
//! re-recorded with `bun host/web/parity.mjs`.

#[test]
fn the_presence_timeline_uses_one_compiled_fixture() {
    let plan = contract::compile(exact_web::parity::PRESENCE_SOURCE).unwrap();
    assert!(!plan.encode().is_empty());
}

#[test]
fn the_engine_matches_the_browser_on_every_recorded_sample() {
    let fixture = include_str!("../fixtures/browser-motion.txt");
    let samples = fixture.lines().filter(|l| l.starts_with("sample ")).count();
    assert!(samples >= 100, "the fixture is recorded: {samples} samples");
    let mismatches = exact_web::parity::check(fixture);
    assert!(
        mismatches.is_empty(),
        "{} disagreements:\n  {}",
        mismatches.len(),
        mismatches.join("\n  ")
    );
}

#[test]
fn every_case_has_a_unique_name_and_a_spring_case_carries_frames() {
    let cases = exact_web::parity::cases();
    let mut names: Vec<&str> = cases.iter().map(|c| c.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), cases.len());
    let json = exact_web::parity::cases_json();
    assert!(json.contains("\"name\":\"spring\""));
    assert!(
        json.contains("\"frames\":[[1,0],"),
        "{}",
        &json[json.len() - 300..]
    );
}
