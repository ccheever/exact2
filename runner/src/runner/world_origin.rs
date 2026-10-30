//! A world's message carries what caused it (LLP 1133.022 D6, D10 item 6).
//!
//! A `postMessage` to a canvas from a commit that was not a person's or the
//! app's own carries that commit's origin in front of its text; the world
//! keeps the least privileged origin posted into it until a fresh start,
//! and marks every message it emits with it (`game/engine/src/sim/origin.rs`,
//! which reads and writes the same mark). The canvas's `message=` handler
//! then runs as a commit of that origin, so a tool that posts into a world
//! cannot start a device through the world's later emissions. The hosts
//! carry the text unchanged.

use super::{CallOrigin, Command};
use exact_plan::Value;

/// What precedes a marked message's text.
const ORIGIN_MARK: &str = "\u{1}origin:";

/// `text`'s origin and the text after its mark; `None` for an unmarked one.
/// A mark can only lower a commit's origin, never raise it: `app`, or a
/// word the runner does not know, reads as the device's.
pub(super) fn unmark(text: &str) -> Option<(CallOrigin, &str)> {
    let (word, text) = text.strip_prefix(ORIGIN_MARK)?.split_once('\u{1}')?;
    let origin = match word {
        "tool" => CallOrigin::Tool,
        "wake" => CallOrigin::Wake,
        _ => CallOrigin::Device,
    };
    Some((origin, text))
}

/// Each `postMessage` to a canvas (two arguments: an iframe's carries a
/// third) a commit of `origin` stated: its text carries the origin, unless
/// the commit is the app's.
pub(super) fn mark_world_posts(origin: CallOrigin, commands: &mut [Command]) {
    if origin == CallOrigin::App {
        return;
    }
    for command in commands
        .iter_mut()
        .filter(|c| c.name == "postMessage" && c.args.len() == 2)
    {
        if let Some(text) = command.args[0].as_str() {
            let marked = format!("{ORIGIN_MARK}{}\u{1}{text}", origin.word());
            command.args[0] = Value::str(&marked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_reads_as_its_origin_and_never_as_the_apps() {
        assert_eq!(
            unmark("\u{1}origin:tool\u{1}buzz"),
            Some((CallOrigin::Tool, "buzz"))
        );
        assert_eq!(
            unmark("\u{1}origin:wake\u{1}"),
            Some((CallOrigin::Wake, ""))
        );
        assert_eq!(
            unmark("\u{1}origin:app\u{1}x"),
            Some((CallOrigin::Device, "x"))
        );
        assert_eq!(
            unmark("\u{1}origin:root\u{1}x"),
            Some((CallOrigin::Device, "x"))
        );
        assert_eq!(unmark("buzz"), None);
        assert_eq!(unmark("\u{1}origin:tool"), None);
    }
}
