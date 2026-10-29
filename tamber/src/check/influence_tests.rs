//! The dial influence found from each dial's readers is the influence a
//! refold of every token finds (theme2's condition on the checker's work in
//! dial counts): on the corpus, and on randomized designs.

use crate::{Role, Vocab};
use std::cell::Cell;

thread_local! {
    /// Refold every token for every dial, as before (the reference).
    static EVERY: Cell<bool> = const { Cell::new(false) };
}

/// Each dial's readers, or, for the reference, every token as each's.
pub(super) fn every_token(readers: &[Vec<usize>], n: usize) -> Vec<Vec<usize>> {
    match EVERY.with(Cell::get) {
        true => vec![(0..n).collect(); readers.len()],
        false => readers.to_vec(),
    }
}

fn vocab() -> Vocab {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../kernel/tables/schema.json")).unwrap();
    let mut system = true;
    let roles = schema["colors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let name = row[0].as_str().unwrap().to_string();
            system &= name.starts_with(|c: char| c.is_ascii_uppercase());
            Role {
                accent: matches!(name.as_str(), "AccentColor" | "LinkText"),
                name,
                system,
                fallback: [
                    row[4].as_str().unwrap().into(),
                    row[5].as_str().unwrap().into(),
                ],
            }
        })
        .collect();
    Vocab {
        roles,
        ..Vocab::default()
    }
}

/// Each token's dials, as the check found them; `None` when it refuses.
fn dials(src: &str, every: bool) -> Option<Vec<(String, Vec<String>)>> {
    EVERY.with(|e| e.set(every));
    let out = crate::check(src, &vocab())
        .ok()
        .map(|f| f.tokens.into_iter().map(|t| (t.name, t.dials)).collect());
    EVERY.with(|e| e.set(false));
    out
}

#[test]
fn the_corpus_moves_with_the_same_dials_either_way() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus");
    let mut judged = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "tamber") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let ours = dials(&src, false);
        assert_eq!(ours, dials(&src, true), "{}", path.display());
        judged += usize::from(ours.is_some_and(|d| d.iter().any(|(_, d)| !d.is_empty())));
    }
    assert!(
        judged > 0,
        "some corpus design has a dial that moves a token"
    );
}

#[test]
fn randomized_designs_move_with_the_same_dials_either_way() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut judged = 0;
    for _ in 0..300 {
        let (d, n) = (1 + next(5), 2 + next(12));
        let mut src = String::from("tamber t\n\naxes\n  tone = a | b\n\ndials\n");
        for k in 0..d {
            match next(3) {
                0 => src += &format!("  c{k}: color = #3366cc\n"),
                1 => src += &format!("  n{k}: number = 0.1  in 0..0.5\n"),
                _ => src += &format!("  l{k}: length = 4px  in 0..12\n"),
            }
        }
        src += "\ntheme\n";
        let dial = |k: usize, src: &str| {
            ["c", "n", "l"]
                .iter()
                .map(|p| format!("{p}{k}"))
                .find(|name| src.contains(&format!("  {name}:")))
                .unwrap()
        };
        for t in 0..n {
            let k = next(d);
            let name = dial(k, &src);
            let earlier = (t > 0).then(|| format!("t{}", next(t)));
            let value = match (name.as_bytes()[0], next(4), &earlier) {
                (b'c', 0, _) => name.clone(),
                (b'c', 1, Some(e)) => format!("color-mix(in srgb, {name} 40%, {e})"),
                (b'c', _, _) => format!("light-dark({name}, #000000)"),
                (b'n', _, _) => format!("oklch(from #3366cc calc(l + {name}) c h)"),
                (_, _, Some(e)) if next(2) == 0 => e.clone(),
                _ => "#ffffff".to_string(),
            };
            src += &format!("  t{t} = {value}\n");
            if next(3) == 0 {
                src += &format!("    when tone=b: #{:06x}\n", next(0xffffff));
            }
        }
        let ours = dials(&src, false);
        assert_eq!(ours, dials(&src, true), "{src}");
        judged += usize::from(ours.is_some_and(|d| d.iter().any(|(_, d)| !d.is_empty())));
    }
    assert!(
        judged >= 40,
        "{judged} of 300 designs checked with a dial moving a token"
    );
}
