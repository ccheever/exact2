//! The recipes and the patterns, answered by name (LLP 1116.021 D1): a
//! recipe (`docs/recipes/<name>.md`) prints whole, a pattern
//! (`docs/patterns/<name>`) its starter comment and how to write it, so the
//! names a screen needs are one call. A recipe or pattern added under `docs/`
//! is added here; a test holds the two lists to the directories.

macro_rules! recipe {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("../../../../docs/recipes/", $name, ".md")),
        )
    };
}

macro_rules! pattern {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../../../../docs/patterns/",
                $name,
                "/app.contract"
            )),
        )
    };
}

pub const RECIPES: &[(&str, &str)] = &[
    recipe!("after-a-write"),
    recipe!("app-json"),
    recipe!("background-transfers"),
    recipe!("background-work"),
    recipe!("chat-thread"),
    recipe!("content-design"),
    recipe!("dates-and-days"),
    recipe!("device"),
    recipe!("edit-sheet"),
    recipe!("editable-rows"),
    recipe!("error-alert"),
    recipe!("fetch-loading-error"),
    recipe!("files-and-grants"),
    recipe!("first-launch"),
    recipe!("foldables"),
    recipe!("functions"),
    recipe!("grouped-form"),
    recipe!("ios-driving"),
    recipe!("keyboard-and-focus"),
    recipe!("list-rows"),
    recipe!("local-network"),
    recipe!("money"),
    recipe!("motion"),
    recipe!("native-module"),
    recipe!("native-views"),
    recipe!("navigation-stack"),
    recipe!("nfc"),
    recipe!("number-field"),
    recipe!("numeric-readout"),
    recipe!("oauth"),
    recipe!("permissions"),
    recipe!("persisted-setting"),
    recipe!("porting-and-release"),
    recipe!("ports-and-browsers"),
    recipe!("purchases"),
    recipe!("random-and-shuffle"),
    recipe!("random-rolls"),
    recipe!("responsive-columns"),
    recipe!("rest-backend"),
    recipe!("properties"),
    recipe!("sequencer"),
    recipe!("sign-in"),
    recipe!("snapback"),
    recipe!("snapback-auth"),
    recipe!("snapback-tests"),
    recipe!("symbols"),
    recipe!("tabs"),
    recipe!("tamber"),
    recipe!("tests"),
    recipe!("theme"),
    recipe!("timer-in-a-component"),
    recipe!("timer-that-survives-relaunch"),
    recipe!("timers"),
    recipe!("usb-hid-serial"),
    recipe!("web-look"),
];

pub const PATTERNS: &[(&str, &str)] = &[
    pattern!("dashboard"),
    pattern!("list-detail"),
    pattern!("one-screen"),
    pattern!("rest"),
    pattern!("tabs"),
];

/// Grammar words a recipe is the answer to, which builders looked up by the
/// word (`sequence` for the sequencer, `keyframes` for motion; euxy port 19).
const ALIASES: &[(&str, &str)] = &[
    ("every", "timers"),
    ("after", "timers"),
    ("tolerance", "timers"),
    ("sequence", "sequencer"),
    ("keyframes", "motion"),
    ("@keyframes", "motion"),
];

/// The recipe a name looks up, as a builder may write it (`grouped-form`,
/// `grouped-form.md`, its path, a word in [`ALIASES`]): its name and its
/// whole text.
pub fn recipe(name: &str) -> Option<(&'static str, &'static str)> {
    let base = name.rsplit('/').next().unwrap_or(name);
    let stem = base.strip_suffix(".md").unwrap_or(base);
    let stem = ALIASES
        .iter()
        .find(|(a, _)| *a == stem)
        .map_or(stem, |(_, r)| r);
    RECIPES.iter().copied().find(|(n, _)| *n == stem)
}

/// A pattern's name and its summary: the first sentence of the comment its
/// `app.contract` opens with (`// Starter: …`), without the comment marks.
pub fn pattern(name: &str) -> Option<(&'static str, String)> {
    let (name, source) = PATTERNS.iter().copied().find(|(n, _)| *n == name)?;
    let comment: Vec<&str> = source
        .lines()
        .map_while(|l| l.strip_prefix("//"))
        .map(str::trim)
        .collect();
    let comment = comment.join(" ");
    let about = comment.strip_prefix("Starter: ").unwrap_or(&comment);
    let first = about.split_once(". ").map_or(about, |(s, _)| s);
    Some((name, format!("{}.", first.trim_end_matches('.'))))
}
