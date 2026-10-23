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
    assert!(out.tokens.iter().all(|t| t.name.value == "ink" || t.value.value == "2px"));
}
