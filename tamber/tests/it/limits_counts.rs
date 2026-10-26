//! The checker's work in token, param and dial counts (QUEUE PRIORITY, the
//! checker bound's follow-up): each walk linear in what it counts, timed at
//! the request size.

use exact_tamber::params::{laid, Param, Params};
use exact_tamber::parse;
use std::time::{Duration, Instant};

/// Params laid with their `params` lines, each found by name once: 40,000
/// params, each named by a line written in reverse (about 1 MiB of design),
/// were a cube (2,000 took 3.5 s).
#[test]
fn many_params_lay_in_moments() {
    let n = 40_000;
    let list: Vec<Param> = (0..n)
        .map(|k| Param {
            token: format!("param.x.C{k}.r"),
            full: format!("a.contract#C{k}.r"),
            kind: "length".into(),
            default: "1px".into(),
        })
        .collect();
    let params = Params {
        list,
        components: (0..n).map(|k| format!("a.contract#C{k}")).collect(),
        ..Default::default()
    };
    let mut src = String::from("tamber app\n\ntheme\n  ink = #111111\n\nparams\n");
    for k in (0..n).rev() {
        src += &format!("  C{k}.r = 2px\n");
    }
    let design = parse(&src).unwrap();
    let began = Instant::now();
    let out = laid(&design, &params);
    let took = began.elapsed();
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(out.tokens.len(), n + 1);
    assert!(out
        .tokens
        .iter()
        .all(|t| t.name.value == "ink" || t.value.value == "2px"));
}

/// A dial's probes refold only the tokens it moves: 2,000 dials beside
/// 20,000 tokens each reading one (2,000 x 2,000 took 35 s in a release
/// build, refolding every token for every dial).
#[test]
fn many_dials_probe_only_what_they_move() {
    let (d, n) = (2_000, 20_000);
    let mut src = String::from("tamber t\n\ndials\n");
    for k in 0..d {
        src += &format!("  d{k}: number = 0.1  in 0..0.5\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    for k in 0..n {
        src += &format!("  t{k} = oklch(from #3366cc calc(l + d{}) c h)\n", k % d);
    }
    let began = Instant::now();
    let folded = exact_tamber::check(&src, &crate::common::vocab());
    let took = began.elapsed();
    assert!(took < Duration::from_secs(20), "{took:?}");
    let folded = folded.unwrap_or_else(|e| panic!("{:?}", &e[..e.len().min(2)]));
    // Each token moves with its own dial, and only it.
    let t = folded.tokens.iter().find(|t| t.name == "t2001").unwrap();
    assert_eq!(t.dials, ["d1"]);
}

/// A dial's dependents are refolded in the design's order, a token's names
/// before it, as every token was: a 512-token chain the dial reads at both
/// ends still moves with it (Astra's round 1: refolded from its far end the
/// chain passed the depth bound and the dial read as inert).
#[test]
fn a_long_chain_a_dial_reads_at_both_ends_still_moves_with_it() {
    let mut src = String::from("tamber t\n\ndials\n  d: number = 0.1  in 0..1\n\ntheme\n  ink = #111111\n  t0: number = d\n");
    for k in 1..511 {
        src += &format!("  t{k}: number = t{}\n", k - 1);
    }
    src += "  t511: number = calc(t510 + d)\n";
    let folded = exact_tamber::check(&src, &crate::common::vocab());
    let folded = folded.unwrap_or_else(|e| panic!("{:?}", &e[..e.len().min(2)]));
    let t = folded.tokens.iter().find(|t| t.name == "t511").unwrap();
    assert_eq!(t.dials, ["d"]);
}

/// An ordinary re-import of a shared dial's change (every token reading it
/// changed alike) is not refused for the budget: only a batch's rounds past
/// its first are charged (Astra's round 1: a trial per token spent it).
#[test]
fn a_shared_dials_change_reimports_within_the_budget() {
    let mut src =
        String::from("tamber t\n\ndials\n  c: color = #000000\n\ntheme\n  ink = #111111\n");
    for k in 0..1000 {
        src += &format!("  surface.component{k} = c\n");
    }
    let vocab = crate::common::vocab();
    let folded = exact_tamber::check(&src, &vocab).unwrap();
    let export = exact_tamber::export::dtcg(&folded, &vocab).unwrap();
    // Every value that was the dial's black, white now.
    fn whiten(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("hex").and_then(|h| h.as_str()) == Some("#000000") {
                    o.insert("hex".into(), serde_json::json!("#ffffff"));
                    o.insert("components".into(), serde_json::json!([1.0, 1.0, 1.0]));
                }
                o.values_mut().for_each(whiten);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(whiten),
            _ => {}
        }
    }
    let mut doc: serde_json::Value = serde_json::from_str(&export).unwrap();
    whiten(&mut doc);
    let edited = doc.to_string();
    let import = exact_tamber::export::reimport_against(&edited, Some(&export), &[], &src, &vocab);
    let import = import.unwrap_or_else(|d| panic!("{:?}", &d[..d.len().min(2)]));
    assert!(
        !import.ops.is_empty() || !import.dials.is_empty(),
        "{:?}",
        &import.unmapped[..import.unmapped.len().min(2)]
    );
}

/// A design whose tokens `surface.componentK` each read the colour dial `c`,
/// and its export with each such token's value set by `colour(k)`.
fn shared_dial(n: usize, colour: impl Fn(usize) -> &'static str) -> (String, String, String) {
    let mut src =
        String::from("tamber t\n\ndials\n  c: color = #000000\n\ntheme\n  ink = #111111\n");
    for k in 0..n {
        src += &format!("  surface.component{k} = c\n");
    }
    let vocab = crate::common::vocab();
    let folded = exact_tamber::check(&src, &vocab).unwrap();
    let export = exact_tamber::export::dtcg(&folded, &vocab).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&export).unwrap();
    let contexts = doc.pointer_mut("/modifiers/mode/contexts").unwrap();
    for sets in contexts.as_object_mut().unwrap().values_mut() {
        for set in sets.as_array_mut().unwrap() {
            // (dotted names are groups: `surface`, then `componentK`)
            let Some(group) = set.get_mut("surface").and_then(|g| g.as_object_mut()) else {
                continue;
            };
            for (name, v) in group {
                let Some(k) = name
                    .strip_prefix("component")
                    .and_then(|k| k.parse::<usize>().ok())
                else {
                    continue;
                };
                let hex = colour(k);
                let c = |i: usize| {
                    u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64 / 255.0
                };
                v["$value"] = serde_json::json!({"colorSpace": "srgb", "components": [c(0), c(1), c(2)], "hex": hex});
            }
        }
    }
    (src, export, doc.to_string())
}

/// The dial decided first (theme2's ruling on the counts' round 2): 1,000
/// tokens a dial takes alike are one dial change, linear, no literal judged.
#[test]
fn a_dial_taking_its_tokens_change_judges_no_literal() {
    let (src, export, edited) = shared_dial(1000, |_| "#ffffff");
    let began = Instant::now();
    let import = exact_tamber::export::reimport_against(
        &edited,
        Some(&export),
        &[],
        &src,
        &crate::common::vocab(),
    );
    let took = began.elapsed();
    let import = import.unwrap_or_else(|d| panic!("{:?}", &d[..d.len().min(2)]));
    assert_eq!(
        import.dials.len(),
        1,
        "{:?}",
        &import.unmapped[..import.unmapped.len().min(2)]
    );
    assert!(import.unmapped.is_empty());
    assert!(took < Duration::from_secs(5), "{took:?}");
}

/// When the dial falls back (its tokens moved to different values), each
/// token's literal is judged then, one check each, charged to the budget: a
/// few land as their literals; thousands are refused in the budget's words.
#[test]
fn a_dial_falling_back_judges_each_literal_within_the_budget() {
    let (src, export, edited) = shared_dial(4, |k| if k % 2 == 0 { "#ffffff" } else { "#ff0000" });
    let import = exact_tamber::export::reimport_against(
        &edited,
        Some(&export),
        &[],
        &src,
        &crate::common::vocab(),
    )
    .unwrap();
    assert!(import.dials.is_empty());
    let set: Vec<String> = import
        .ops
        .iter()
        .filter_map(|op| match op {
            exact_tamber::Op::SetToken { name, value, .. } => Some(format!("{name}={value}")),
            _ => None,
        })
        .collect();
    // Each literal judged as before: three land, and the last, which would
    // leave the dial moving no token, is refused with the design's reason.
    assert_eq!(set.len(), 3, "{set:?} {:?}", import.unmapped);
    assert_eq!(import.unmapped.len(), 1, "{:?}", import.unmapped);
    assert!(
        import.unmapped[0].why.contains("tamber-dial-inert"),
        "{:?}",
        import.unmapped
    );
    let (src, export, edited) =
        shared_dial(3000, |k| if k % 2 == 0 { "#ffffff" } else { "#ff0000" });
    let began = Instant::now();
    let refused = exact_tamber::export::reimport_against(
        &edited,
        Some(&export),
        &[],
        &src,
        &crate::common::vocab(),
    );
    let d = refused.unwrap_err();
    assert!(d[0].message.contains("expands past"), "{d:?}");
    assert!(
        began.elapsed() < Duration::from_secs(60),
        "{:?}",
        began.elapsed()
    );
}

/// A design asked about again, unchanged, is answered from the last few
/// checked (theme2's condition (2) on the influence remainder): a client
/// asking can't make the server fold it, dial influence and all, each time.
#[test]
fn a_design_checked_again_unchanged_is_answered_from_the_last() {
    let mut src = String::from("tamber t\n\ndials\n");
    for k in 0..800 {
        src += &format!("  d{k}: number = 0.1  in 0..0.5\n");
    }
    src += "\ntheme\n  ink = #111111\n  t0 = oklch(from #3366cc calc(l + d0) c h)\n";
    for k in 1..800 {
        src += &format!("  t{k} = oklch(from t{} calc(l + d{k}) c h)\n", k - 1);
    }
    let vocab = crate::common::vocab();
    let began = Instant::now();
    let check = |src: &str| {
        exact_tamber::parse(src).and_then(|d| exact_tamber::check_design_cached(d, &vocab))
    };
    let first = check(&src).map(|f| f.tokens.len());
    let cold = began.elapsed();
    let began = Instant::now();
    for _ in 0..20 {
        assert_eq!(check(&src).map(|f| f.tokens.len()), first);
    }
    let warm = began.elapsed();
    assert!(warm < cold, "20 asks {warm:?}, the first {cold:?}");
}

/// No client operation adds a dial (theme2's condition (1) on the influence
/// remainder): every operation a client may send leaves the design's dials as
/// they were; only a hand edit (or the local token's import of a file it
/// writes) can add one, and a `restore` is no client's to send.
#[test]
fn no_client_operation_adds_a_dial() {
    use exact_tamber::{apply, Op};
    let src = "tamber t\n\naxes\n  tone = a | b\n\ndials\n  d: number = 0.1  in 0..0.5\n\ntheme\n  ink = #111111\n  t = oklch(from #3366cc calc(l + d) c h)\n";
    let vocab = crate::common::vocab();
    let dials = |text: &str| -> Vec<String> {
        exact_tamber::parse(text)
            .unwrap()
            .dials
            .iter()
            .map(|d| d.name.value.clone())
            .collect()
    };
    let ops = vec![
        Op::SetDial {
            name: "d".into(),
            value: "0.2".into(),
        },
        Op::SetToken {
            name: "ink".into(),
            when: Vec::new(),
            value: "#222222".into(),
            at: None,
            comment: None,
        },
        Op::SetToken {
            name: "ink".into(),
            when: vec![("tone".into(), "b".into())],
            value: "#333333".into(),
            at: None,
            comment: None,
        },
        Op::AddToken {
            name: "new.one".into(),
            kind: None,
            value: "d2".into(),
            meta: Vec::new(),
            whens: Vec::new(),
            comment: None,
            before: None,
        },
        Op::AddToken {
            name: "new.two".into(),
            kind: None,
            value: "#444444".into(),
            meta: Vec::new(),
            whens: Vec::new(),
            comment: None,
            before: None,
        },
        Op::RenameToken {
            from: "ink".into(),
            to: "ink2".into(),
        },
        Op::RemoveToken { name: "ink".into() },
        Op::AddAxisValue {
            axis: "tone".into(),
            value: "c".into(),
            before: None,
        },
        Op::RemoveAxisValue {
            axis: "tone".into(),
            value: "b".into(),
        },
        Op::AddAxis {
            name: "size".into(),
            values: vec!["s".into(), "m".into()],
            under_dark: false,
        },
    ];
    for op in &ops {
        // (a refused one writes nothing at all)
        if let Ok(edited) = apply(src, op, &vocab) {
            assert_eq!(dials(&edited.text), ["d"], "{op:?}");
        }
    }
}

/// A candidate whose export name differs from the design's spelling (a role
/// override's) is found again when its dial falls back (Astra's check of the
/// dial step: a lookup by the export's name panicked).
#[test]
fn a_role_overrides_literal_is_found_when_its_dial_falls_back() {
    let src = "tamber t\n\ndials\n  c: color = #000000\n\ntheme\n  ink = #111111\n  accentcolor = c\n  peer = c\n";
    let vocab = crate::common::vocab();
    let folded = exact_tamber::check(src, &vocab).unwrap();
    let export = exact_tamber::export::dtcg(&folded, &vocab).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&export).unwrap();
    let contexts = doc.pointer_mut("/modifiers/mode/contexts").unwrap();
    let mut named = Vec::new();
    for sets in contexts.as_object_mut().unwrap().values_mut() {
        for set in sets.as_array_mut().unwrap() {
            for (name, v) in set.as_object_mut().unwrap() {
                let hex = match name.as_str() {
                    "peer" => "#ff0000",
                    n if n.eq_ignore_ascii_case("accentcolor") => "#ffffff",
                    _ => continue,
                };
                named.push(name.clone());
                let c = |i: usize| {
                    u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64 / 255.0
                };
                v["$value"] = serde_json::json!({"colorSpace": "srgb", "components": [c(0), c(1), c(2)], "hex": hex});
            }
        }
    }
    assert!(named.iter().any(|n| n != "peer"), "{named:?}");
    let import =
        exact_tamber::export::reimport_against(&doc.to_string(), Some(&export), &[], src, &vocab);
    let import = import.unwrap_or_else(|d| panic!("{d:?}"));
    assert!(import.dials.is_empty(), "{:?}", import.dials);
}
