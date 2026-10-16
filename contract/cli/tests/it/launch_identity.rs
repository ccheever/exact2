//! A resource whose identifying arguments change every launch (LLP 1027.005):
//! its kept answer can never be shown, and the warning names the `with` repair.

use super::warnings::warned;

#[test]
fn an_identifying_argument_of_the_launch_is_named_with_with() {
    let library = |call: &str| {
        format!("shape Time\n  epochAtZero: number\n  seed: number\n  utcOffset: number\nshape Library\n  ready: bool\nfn start(t: Time): number = t.epochAtZero\ncomponent App\n  resource time = exactTime() as shape Time\n  resource recording = saved() as shape Time\n  derive origin = time.epochAtZero\n  derive clock = time\n  state minute = 0\n  state since = 0\n  resource lib = {call} as shape Library\n  action begin\n    if minute > 0\n      since = time.epochAtZero\n  view\n    button \"x\" press=begin\n")
    };
    const KEPT: &str = "the source receives it where it did";
    const MOVED: &str = "so change `library`'s parameters to that order too";
    for (call, said, declaration, order) in [
        (
            "library(time.epochAtZero)",
            "time.epochAtZero",
            "library() with time.epochAtZero",
            KEPT,
        ),
        (
            "library(time.seed)",
            "time.seed",
            "library() with time.seed",
            KEPT,
        ),
        ("library(origin)", "origin", "library() with origin", KEPT),
        (
            "library(start(time))",
            "start(time)",
            "library() with start(time)",
            KEPT,
        ),
        ("library(since)", "since", "library() with since", KEPT),
        ("library([time.seed])", "…", "library() with …", KEPT),
        (
            "library(clock.seed)",
            "clock.seed",
            "library() with clock.seed",
            KEPT,
        ),
        (
            "library(minute, time.epochAtZero)",
            "time.epochAtZero",
            "library(minute) with time.epochAtZero",
            KEPT,
        ),
        (
            "library(time.epochAtZero) with minute",
            "time.epochAtZero",
            "library() with time.epochAtZero, minute",
            KEPT,
        ),
        (
            "library(time.epochAtZero, minute, \"all\")",
            "time.epochAtZero",
            "library(minute, \"all\") with time.epochAtZero",
            MOVED,
        ),
    ] {
        let (result, warnings) = contract::compile_warned(&library(call));
        result.unwrap();
        let ids: Vec<_> = warnings
            .iter()
            .map(|w| (w.id.as_str(), w.span.line))
            .collect();
        assert_eq!(ids, [("lower-resource-launch-identity", 15)], "{call}");
        let message = &warnings[0].message;
        assert!(
            message.contains(&format!("because `{said}` changes every launch"))
                && message.contains(&format!("(`resource lib = {declaration}`)"))
                && message.contains(order),
            "{message}"
        );
    }
    // After `with`; a time made from the epoch; what stays from one launch
    // to the next; and a field of that name on the app's own record.
    for call in [
        "library() with time.epochAtZero, time.seed",
        "library(time.epochAtZero + minute)",
        "library(time.utcOffset)",
        "library(minute)",
        "library(recording.epochAtZero)",
        "library(recording.seed)",
        "library(start(recording))",
    ] {
        assert_eq!(warned(&library(call)), [], "{call}");
    }
}
