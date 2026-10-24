//! `aria-keyshortcuts`, the rule every host keeps (the web's input-glue.js,
//! the Apple hosts' `Shortcuts.swift`): a key whose chord a shown button
//! declares presses that button before the focus's `key` handlers
//! hear it — the web's capture listener at the document — and goes no
//! further, its default prevented. A focused world's canvas is such a
//! focus: it never hears the chord. A button bound to a world's action
//! (`action="buy"`) is pressed as the web's assistive click presses it, a
//! down and an up of that action that moves no focus, then its authored
//! `press` if it has one, as the web's click runs both, so a focused world
//! keeps its held keys and a shortcut and the world's own binding of the
//! same chord never both act (LLP 1132 D11). A disabled button still takes
//! its key and presses nothing, as on the web. None behind the frontmost shown
//! `aria-modal` view (gallery F22); never Enter or Space while the focus is
//! a control those keys activate (onboarding F27); a text field keeps its
//! typing, where only a Control or Meta chord, or Escape, is a shortcut.
use super::*;
use exact_runner::KeyModifiers;

/// The named keys a chord may end in (input-glue's `shortcutKeys`), beside a
/// single character and F1–F35.
const NAMED: [&str; 14] = [
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "Insert",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

/// Whether one chord as ARIA spells it (`Meta+Shift+K`, `Control+Plus`,
/// `Meta++`, `F13`) is the key `name` (`KeyboardEvent.key`) with exactly the
/// modifiers `held`: the key compared without case, as input-glue does.
pub(crate) fn chord_matches(chord: &str, name: &str, held: KeyModifiers) -> bool {
    let mut parts: Vec<&str> = chord.split('+').collect();
    let key = if chord == "+" {
        parts.clear();
        "Plus"
    } else if parts.len() >= 3 && parts[parts.len() - 2..].iter().all(|p| p.is_empty()) {
        parts.truncate(parts.len() - 2);
        "Plus"
    } else {
        parts.pop().unwrap_or("")
    };
    let key = match key {
        "Plus" => "+",
        "Space" => " ",
        k => k,
    };
    let function = key
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=35).contains(&n) && key == format!("F{n}"));
    if key.chars().count() != 1 && !NAMED.contains(&key) && !function {
        return false;
    }
    if !parts
        .iter()
        .all(|p| matches!(*p, "Meta" | "Control" | "Alt" | "Shift"))
    {
        return false;
    }
    let has = |m: &str| parts.contains(&m);
    has("Meta") == held.meta
        && has("Control") == held.ctrl
        && has("Alt") == held.alt
        && has("Shift") == held.shift
        && key.to_lowercase() == name.to_lowercase()
}

impl<D: DataSource> Presenter<D> {
    /// The button whose declared chord takes the key `name` with the
    /// modifiers held now, pressed (a repeat is not a press): true when one
    /// took it, and the key goes no further.
    pub(crate) fn shortcut(&mut self, name: &str, repeat: bool, now_ms: f64) -> bool {
        let held = self.modifiers();
        let kernel = self.host.kernel();
        let focus = self.focus.and_then(|id| kernel.node(id));
        let editing = focus.is_some_and(|n| {
            n.node_type == NodeType::TextInput
                || matches!(
                    n.props.str(PropId::AccessibilityRole),
                    Some("textbox" | "searchbox" | "combobox")
                )
        });
        if editing && name != "Escape" && !held.meta && !held.ctrl {
            return false;
        }
        let declared = self.shortcut_buttons(name);
        // Shown: a box, not hidden or inert by its route or an ancestor.
        let Some(button) = declared.into_iter().find(|&id| {
            self.host.route_visibility(id) == (false, false) && self.box_of(id).is_some()
        }) else {
            return false;
        };
        let disabled = self
            .host
            .kernel()
            .node(button)
            .is_none_or(|n| n.props.bool(PropId::Disabled) == Some(true));
        if !repeat && !disabled {
            self.press_shortcut(button, now_ms);
        }
        true
    }

    /// The buttons that declare a chord taking the key `name` with the
    /// modifiers held now, in tree order: within the frontmost shown
    /// `aria-modal` view and what an alert allows, and for a plain Enter or
    /// Space only the focused one, whose own activation it is.
    pub(super) fn shortcut_buttons(&mut self, name: &str) -> Vec<ViewId> {
        let held = self.modifiers();
        let kernel = self.host.kernel();
        let plain = !(held.meta || held.ctrl || held.alt || held.shift);
        let activates = plain
            && matches!(name, "Enter" | " ")
            && self.focus.is_some_and(|id| {
                kernel.node(id).is_some_and(|n| {
                    exact_kernel::ControlKind::of(n.node_type, n.props)
                        == Some(exact_kernel::ControlKind::Button)
                        || n.props.str(PropId::AccessibilityRole) == Some("button")
                }) || self
                    .host
                    .runner()
                    .handlers_of(id)
                    .contains(&EventKind::Press)
            });
        let preorder = self.host.preorder();
        // The frontmost shown `aria-modal` view: the last in tree order.
        let modal = preorder.iter().rev().copied().find(|&id| {
            kernel
                .node(id)
                .is_some_and(|n| n.props.bool(PropId::AccessibilityModal) == Some(true))
                && self.host.route_visibility(id) == (false, false)
                && self.display.allows(kernel, id)
        });
        preorder
            .into_iter()
            .filter(|&id| self.alert_allows(id))
            .filter(|&id| {
                kernel.node(id).is_some_and(|n| {
                    n.props
                        .str(PropId::AccessibilityKeyShortcuts)
                        .is_some_and(|chords| {
                            chords
                                .split_whitespace()
                                .any(|c| chord_matches(c, name, held))
                        })
                        && (exact_kernel::ControlKind::of(n.node_type, n.props)
                            == Some(exact_kernel::ControlKind::Button)
                            || n.props.str(PropId::AccessibilityRole) == Some("button"))
                })
            })
            .filter(|&id| !(activates && self.focus != Some(id)))
            .filter(|&id| {
                modal.is_none_or(|m| {
                    let mut at = Some(id);
                    while let Some(n) = at {
                        if n == m {
                            return true;
                        }
                        at = kernel.node(n).and_then(|n| n.parent);
                    }
                    false
                })
            })
            .collect()
    }

    /// Press `button` as its shortcut does: a control's assistive down and
    /// up, then its authored `press`, as the web's click runs both.
    pub(super) fn press_shortcut(&mut self, button: ViewId, now_ms: f64) {
        self.note_press(button, true);
        if self.control_target(button) == Some(button) {
            let (x, y, _, _) = self.rect_of(button).unwrap_or_default();
            for phase in ["down", "up"] {
                let contact = crate::surfaces::controls::ASSISTIVE_CONTACT;
                self.control_input(button, phase, x, y, contact, now_ms);
            }
            // Then its authored `press`, as the web's click runs both.
            if self
                .host
                .runner()
                .handlers_of(button)
                .contains(&EventKind::Press)
            {
                self.activate_with(button, now_ms, false);
            }
        } else {
            self.activate_with(button, now_ms, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::chord_matches;
    use exact_runner::KeyModifiers;

    fn held(meta: bool, ctrl: bool, alt: bool, shift: bool) -> KeyModifiers {
        KeyModifiers {
            meta,
            ctrl,
            alt,
            shift,
        }
    }

    #[test]
    fn chords_are_arias_spelling_matched_as_the_web_matches_them() {
        let none = held(false, false, false, false);
        assert!(chord_matches(
            "Control+V",
            "v",
            held(false, true, false, false)
        ));
        assert!(!chord_matches("Control+V", "v", none));
        assert!(!chord_matches(
            "Control+V",
            "v",
            held(false, true, false, true)
        ));
        assert!(chord_matches(
            "Meta++",
            "+",
            held(true, false, false, false)
        ));
        assert!(chord_matches(
            "Control+Plus",
            "+",
            held(false, true, false, false)
        ));
        assert!(chord_matches("+", "+", none));
        assert!(chord_matches("F13", "F13", none));
        assert!(chord_matches("F24", "F24", none));
        assert!(!chord_matches("F36", "F36", none));
        assert!(chord_matches(
            "Shift+ArrowDown",
            "ArrowDown",
            held(false, false, false, true)
        ));
        assert!(chord_matches("Space", " ", none));
        assert!(!chord_matches("Hyper+K", "k", none));
        assert!(!chord_matches("Volume", "Volume", none));
    }
}
