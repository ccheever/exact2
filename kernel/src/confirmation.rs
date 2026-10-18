//! The shape of a confirmation (LLP 1115 D6, by Apple's HIG), in one place.
//!
//! An `alertdialog`'s rows are its text, its actions (buttons with a `press`
//! that also close it) and its cancel (a button that only closes it). With
//! explanatory text, one cancel and at most three buttons it is an **alert**:
//! `UIAlertController(.alert)`, `NSAlert`, Material's basic dialog. Anything
//! else is a chooser (an action sheet, a menu) or stays as authored. A
//! **prompt** is an alert whose rows include text fields (a rename's): its
//! fields count with its text, since what it asks explains it, and only an
//! alert takes them (`UIAlertController.addTextField`, `NSAlert`'s accessory
//! view); the Apple hosts present it, the Linux host paints it as authored.
//!
//! Three readers hold one rule here: lowering, which implies an alert-shaped
//! dialog's `closedby` (LLP 1115.000 D13.4: an alert has no outside); the
//! Linux host's top layer, which draws Android's and reads the kernel's tree
//! (`host/linux/src/presenter/top/shape.rs`); and the Apple hosts' Swift,
//! which reads its own view mirror and asks through
//! `exact_confirmation_is_alert` (`ConfirmationShape.swift`).

/// The alert shape: explanatory text, one cancel, and at most three buttons
/// with it. `texts` counts a prompt's text fields with its lines.
pub fn is_alert(texts: usize, actions: usize, cancels: usize) -> bool {
    texts > 0 && cancels == 1 && actions + cancels <= 3
}

/// An alert's words: its title is the label (`aria-label`, "Remove Book?"),
/// else its first line; its message is the rest, a line each.
pub fn alert_text(label: Option<&str>, lines: &[String]) -> (Option<String>, Option<String>) {
    let mut rest = lines;
    let title = match label.filter(|l| !l.is_empty()) {
        Some(label) => Some(label.to_string()),
        None => rest.split_first().map(|(first, others)| {
            rest = others;
            first.clone()
        }),
    };
    (title, (!rest.is_empty()).then(|| rest.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alert_is_text_one_cancel_and_at_most_three_buttons() {
        assert!(is_alert(1, 1, 1), "Remove? Remove / Cancel");
        assert!(is_alert(2, 2, 1), "Save? Save / Discard / Cancel");
        assert!(!is_alert(0, 1, 1), "no text: a chooser");
        assert!(!is_alert(1, 1, 0), "no cancel");
        assert!(!is_alert(1, 1, 2), "two cancels");
        assert!(!is_alert(1, 3, 1), "four buttons: a chooser");
        assert!(is_alert(1, 0, 1), "text and a cancel alone");
    }

    #[test]
    fn an_alerts_title_is_its_label_else_its_first_line() {
        let lines = ["This cannot be undone.".to_string(), "Really.".to_string()];
        assert_eq!(
            alert_text(Some("Delete it?"), &lines),
            (
                Some("Delete it?".into()),
                Some("This cannot be undone.\nReally.".into())
            )
        );
        assert_eq!(
            alert_text(None, &lines),
            (
                Some("This cannot be undone.".into()),
                Some("Really.".into())
            )
        );
        assert_eq!(
            alert_text(Some(""), &lines[..1]),
            (Some("This cannot be undone.".into()), None)
        );
        assert_eq!(alert_text(None, &[]), (None, None));
    }
}
