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
