//! Scope lookups and slot re-typing: what an index or a narrower refresh
//! must still answer as the scans they replaced did.
use crate::{ComponentTypes, Ref, Scope, Sink, Ty, Types};
use contract_syntax::Component;

/// Run after each body: the scope `retype_written` left is the one
/// re-typing every slot gives.
pub(crate) fn all_slots_retyped(scope: &Scope, c: &Component, ct: &ComponentTypes) {
    let first = c.props.len() + c.injects.len();
    let mut all = scope.clone();
    for (i, ty) in ct.slots.iter().enumerate() {
        all.retype(first + i, ty.clone());
    }
    let types = |s: &Scope| {
        s.frames[0]
            .names
            .iter()
            .map(|n| n.2.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        types(scope),
        types(&all),
        "a slot changed that the body did not assign"
    );
}

/// Run before each caller's calls refine their callees: the declarations
/// kept up to date are the ones a fresh scope would build.
pub(crate) fn same_declarations(kept: &Scope, fresh: &Scope) {
    let names = |s: &Scope| s.frames[0].names.clone();
    assert_eq!(
        names(kept),
        names(fresh),
        "a callee's refined type was not kept"
    );
}

fn names(n: u32) -> Vec<(String, Ref, Ty)> {
    (0..n)
        .map(|i| (format!("n{i}"), Ref::Slot(i), Ty::Number))
        .collect()
}

#[test]
fn an_indexed_frame_finds_a_names_first_entry() {
    let mut frame = names(20);
    frame[17].0 = "n3".into();
    let mut scope = Scope::default();
    scope.push(frame);
    assert_eq!(scope.lookup("n3").map(|(r, _)| r), Some(Ref::Slot(3)));
    assert_eq!(scope.lookup("n19").map(|(r, _)| r), Some(Ref::Slot(19)));
    assert_eq!(scope.lookup("n17").map(|(r, _)| r), None);
}

#[test]
fn an_inner_frame_shadows_an_indexed_outer_one() {
    let mut scope = Scope::default();
    scope.push(names(20));
    let mut inner = names(20);
    for (i, entry) in inner.iter_mut().enumerate() {
        entry.1 = Ref::Param(i as u32);
    }
    scope.push(inner);
    assert_eq!(scope.lookup("n5").map(|(r, _)| r), Some(Ref::Param(5)));
    scope.push_region(Some(("n6".into(), Ref::Item(0), Ty::String)));
    assert_eq!(
        scope.lookup("n6"),
        Some((Ref::Item(0), &Ty::String)),
        "the innermost binding wins"
    );
    assert_eq!(scope.lookup("n7").map(|(r, _)| r), Some(Ref::Param(7)));
    scope.pop();
    scope.pop();
    assert_eq!(scope.lookup("n5").map(|(r, _)| r), Some(Ref::Slot(5)));
}

#[test]
fn a_region_frame_counts_depth_past_an_indexed_frame() {
    let mut scope = Scope::default();
    scope.push_region(Some(("row".into(), Ref::Item(0), Ty::String)));
    let mut frame = names(20);
    frame.push(("row".into(), Ref::Item(0), Ty::Number));
    scope
        .frames
        .push(std::sync::Arc::new(crate::frame::Frame::new(frame, true)));
    scope.push_region(Some(("cell".into(), Ref::Item(0), Ty::String)));
    assert_eq!(scope.lookup("row"), Some((Ref::Item(1), &Ty::Number)));
    assert_eq!(scope.lookup("cell"), Some((Ref::Item(0), &Ty::String)));
}

#[test]
fn a_name_added_after_a_lookup_is_found() {
    let mut scope = Scope::default();
    scope.push(names(20));
    let shared = scope.clone();
    assert_eq!(scope.lookup("late"), None);
    scope.push_name_at(0, ("late".into(), Ref::Slot(20), Ty::String));
    scope.push_name_at(0, ("n2".into(), Ref::Slot(21), Ty::String));
    assert_eq!(scope.lookup("late").map(|(r, _)| r), Some(Ref::Slot(20)));
    assert_eq!(scope.lookup("n2").map(|(r, _)| r), Some(Ref::Slot(2)));
    assert_eq!(
        shared.lookup("late"),
        None,
        "a shared frame is copied, not grown"
    );
}

#[test]
fn refreshing_only_what_changed_matches_building_again() {
    // Under test, `retype_written` asserts that the scope it leaves equals
    // one with every slot re-typed, and `calls::refine` that its kept
    // declarations equal a fresh scope's. Random components drive both
    // through writes that settle, widen and refuse a slot's type, and calls
    // that type a callee's parameter, reading both in later bodies.
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let (mut checked, mut settled, mut refined) = (0, 0, 0);
    while checked < 300 {
        // Each state is a number, a string, or an option a write settles;
        // the first is a number.
        let kinds: Vec<usize> = (0..1 + next(24))
            .map(|i| if i == 0 { 0 } else { next(3) })
            .collect();
        let of = |kind: usize, next: &mut dyn FnMut(usize) -> usize| {
            let same: Vec<usize> = (0..kinds.len()).filter(|&i| kinds[i] == kind).collect();
            same[next(same.len())]
        };
        let mut source = String::from("component App\n");
        for (i, kind) in kinds.iter().enumerate() {
            let init = ["0", "\"t\"", "none"][*kind];
            source.push_str(&format!("  state s{i} = {init}\n"));
        }
        // Callees with an untyped parameter, which calls type.
        let callees = 1 + next(4);
        for j in 0..callees {
            source.push_str(&format!(
                "  action c{j}(q)\n    s{} = q\n",
                of(0, &mut next)
            ));
        }
        for a in 0..1 + next(24) {
            source.push_str(&format!("  action a{a}(p: number)\n"));
            for _ in 0..1 + next(3) {
                let target = next(kinds.len());
                let value = match (kinds[target], next(2)) {
                    (0, 0) => "p".to_string(),
                    (0, _) => format!("s{}", of(0, &mut next)),
                    (1, _) => "\"u\"".to_string(),
                    (_, 0) => "some(p)".to_string(),
                    (_, _) => format!("s{}", of(2, &mut next)),
                };
                match next(4) {
                    0 => source.push_str(&format!("    c{}(p)\n", next(callees))),
                    _ => source.push_str(&format!("    s{target} = {value}\n")),
                }
            }
        }
        // Every option is settled and every callee called somewhere.
        source.push_str("  action last(p: number)\n");
        for (i, kind) in kinds.iter().enumerate() {
            if *kind == 2 {
                source.push_str(&format!("    s{i} = some(p)\n"));
            }
        }
        for j in 0..callees {
            source.push_str(&format!("    c{j}(p)\n"));
        }
        source.push_str("  view\n    text \"ok\"\n");
        let file = contract_syntax::parse(&source).unwrap();
        // The whole check, as a build runs it: calls between actions are
        // resolved before the component is.
        checked += 1;
        let done = match crate::check_all(&file, false, |_| false, None) {
            Ok(done) => done,
            Err(e) => panic!(
                "{source}\n{:?}",
                e.iter().map(|e| &e.message).collect::<Vec<_>>()
            ),
        };
        let ct = &done.types.components[0];
        settled += ct
            .slots
            .iter()
            .filter(|t| matches!(t, Ty::Option(inner) if **inner != Ty::Unknown))
            .count();
        refined += ct.actions[..callees]
            .iter()
            .filter(|params| params[0] != Ty::Unknown)
            .count();
    }
    // The writes did settle options, and calls did type their callees'
    // parameters.
    assert!(settled > 300, "{settled} settled slots");
    assert!(refined > 300, "{refined} refined parameters");
}

#[test]
fn a_parameter_shadows_a_state_in_a_large_component() {
    // Twenty states make the declarations frame indexed; the parameter's
    // frame is searched first all the same.
    let mut source = String::from("component App\n  state count = 0\n");
    for i in 0..20 {
        source.push_str(&format!("  state pad{i} = \"\"\n"));
    }
    source.push_str("  state n = \"text\"\n  action set(n: number)\n    count = n\n");
    source.push_str("  view\n    text \"ok\"\n");
    let file = contract_syntax::parse(&source).unwrap();
    let mut sink = Sink::default();
    let ct =
        crate::component::check_component(&file.components[0], &Types::default(), None, &mut sink);
    assert!(sink.errors.is_empty(), "{:?}", sink.errors);
    assert_eq!(ct.slots[0], Ty::Number);
}
