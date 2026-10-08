//! Access hatches in the Apple host's Rust (LLP 1075.003.000.001): what the
//! plan says of each word decides how a hatched node reaches Swift.

use crate::host::NoData;
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;

/// LLP 1075.003.000.001 §4.3: the plan carries each hatch word with the
/// platforms that handle it. A word this platform is not given reaches Swift
/// as `hatchOff`, an ordinary node no hatch is called for; one it is given,
/// and every word of a plan that lists none, reaches it as `hatch`.
#[test]
fn a_hatch_word_the_plan_does_not_give_this_platform_reaches_swift_unhatched() {
    let src = r#"component App
  view
    column
      column hatch="here" testId="here"
      column hatch="elsewhere" testId="elsewhere"
"#;
    let mine = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "ios"
    };
    let boot = |plan: &exact_plan::Plan| {
        Host::boot(
            &plan.encode(),
            NoData,
            Box::new(MonospaceMeasurer::default()),
            402.0,
            874.0,
        )
        .unwrap()
        .1
    };
    // No rows: only the module can say, and both words stay hatched.
    let bare = contract::compile(src).unwrap();
    let first = boot(&bare);
    assert!(first.contains(r#""hatch":"here""#) && first.contains(r#""hatch":"elsewhere""#));
    assert!(!first.contains("hatchOff"), "{first}");
    // The rows a bake writes from app.json: `here` is this platform's, `elsewhere` another's.
    let mut plan = contract::compile(src).unwrap();
    plan.add_hatch("here", exact_plan::Plan::hatch_platform_bit(mine));
    plan.add_hatch("elsewhere", exact_plan::Plan::hatch_platform_bit("android"));
    assert!(plan.handles_hatch("here", mine) && !plan.handles_hatch("elsewhere", mine));
    assert!(
        !plan.handles_hatch("undeclared", mine),
        "a word the rows lack is no platform's"
    );
    let first = boot(&exact_plan::Plan::decode(&plan.encode()).unwrap());
    assert!(first.contains(r#""hatch":"here""#), "{first}");
    assert!(
        first.contains(r#""hatchOff":"elsewhere""#) && !first.contains(r#""hatch":"elsewhere""#),
        "{first}"
    );
}
