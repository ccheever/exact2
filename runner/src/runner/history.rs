//! The visits a native host's Forward goes to (LLP 1115.000 D5.5; LLP 1038
//! D7 as amended 2026-11-01). A browser keeps them in session history, and
//! the web host's mirror of the router finds them there; a native host has
//! no session history, so the runner keeps the same list beside the value.
//!
//! The rule is the browser's: going back puts the visits it left ahead, a
//! step forward takes the nearest one, and any other change of the stack's
//! visits drops them, as a new entry truncates session history's forward
//! tail (HTML, "Session history"). A `replace` keeps its visit, and so what
//! is ahead, as `replaceState` keeps the tail. Only the selected tab's stack
//! is history; a tab switch is a navigation of its own (D12 keeps it out of
//! native history).

use exact_route::{Entry, Router};

/// A visit's location as Forward delivers it: its url, and its fragment.
fn location(e: &Entry) -> String {
    match e.fragment.is_empty() {
        true => e.url.clone(),
        false => format!("{}#{}", e.url, e.fragment),
    }
}

fn selected(r: &Router) -> &[Entry] {
    r.tabs
        .iter()
        .find(|t| t.name == r.tab)
        .map_or(&[], |t| &t.stack)
}

/// Update `ahead`, nearest last, for a commit from `before` to `after`.
pub(super) fn step(before: Option<&Router>, after: &Router, ahead: &mut Vec<String>) {
    let Some(before) = before.filter(|b| b.tab == after.tab) else {
        ahead.clear();
        return;
    };
    let (old, new) = (selected(before), selected(after));
    let same = |a: &[Entry], b: &[Entry]| a.iter().map(|e| e.id).eq(b.iter().map(|e| e.id));
    if new.len() < old.len() && same(&old[..new.len()], new) {
        // Back, by one visit or several: the nearest is the one beneath the
        // old top's chain, so it goes last.
        ahead.extend(old[new.len()..].iter().rev().map(location));
    } else if new.len() == old.len() + 1
        && same(old, &new[..old.len()])
        && ahead.last() == new.last().map(location).as_ref()
    {
        ahead.pop();
    } else if !same(old, new) {
        ahead.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_route::Tab;

    fn entry(id: u64, url: &str) -> Entry {
        Entry {
            id,
            name: "screen".into(),
            url: url.into(),
            tab: "home".into(),
            params: Default::default(),
            fragment: String::new(),
        }
    }

    fn router(tab: &str, stack: &[(u64, &str)]) -> Router {
        Router {
            tab: tab.into(),
            tabs: ["home", "settings"]
                .map(|name| Tab {
                    name: name.into(),
                    stack: match name == tab {
                        true => stack.iter().map(|&(id, url)| entry(id, url)).collect(),
                        false => vec![entry(99, "/other")],
                    },
                })
                .to_vec(),
            next: 100,
        }
    }

    #[test]
    fn back_puts_visits_ahead_and_forward_takes_the_nearest() {
        let mut ahead = Vec::new();
        let three = router("home", &[(0, "/"), (1, "/a"), (2, "/b")]);
        let one = router("home", &[(0, "/")]);
        step(Some(&three), &one, &mut ahead);
        assert_eq!(ahead, ["/b", "/a"], "two steps back: /a is the nearest");
        // Forward is a new visit at the nearest location.
        let forward = router("home", &[(0, "/"), (3, "/a")]);
        step(Some(&one), &forward, &mut ahead);
        assert_eq!(ahead, ["/b"]);
        // A commit that leaves the stack's visits as they were keeps them.
        step(Some(&forward), &forward, &mut ahead);
        assert_eq!(ahead, ["/b"]);
    }

    #[test]
    fn any_other_navigation_drops_what_is_ahead() {
        let two = router("home", &[(0, "/"), (1, "/a")]);
        let one = router("home", &[(0, "/")]);
        for after in [
            // A push somewhere else, as a browser's new entry truncates.
            router("home", &[(0, "/"), (2, "/c")]),
            // An `open` that starts the stack again.
            router("home", &[(2, "/c")]),
            // A tab switch.
            router("settings", &[(0, "/settings")]),
        ] {
            let mut ahead = Vec::new();
            step(Some(&two), &one, &mut ahead);
            assert_eq!(ahead, ["/a"]);
            step(Some(&one), &after, &mut ahead);
            assert!(ahead.is_empty(), "{after:?}");
        }
        // A `replace` keeps the visit's id: what is ahead stays.
        let mut ahead = Vec::new();
        step(Some(&two), &one, &mut ahead);
        step(Some(&one), &router("home", &[(0, "/c")]), &mut ahead);
        assert_eq!(ahead, ["/a"]);
        let mut ahead = vec!["/a".to_string()];
        step(None, &one, &mut ahead);
        assert!(ahead.is_empty(), "a launch has nothing ahead");
    }

    #[test]
    fn a_fragment_goes_with_the_visit() {
        let mut ahead = Vec::new();
        let mut two = router("home", &[(0, "/"), (1, "/a")]);
        two.tabs[0].stack[1].fragment = "part".into();
        let one = router("home", &[(0, "/")]);
        step(Some(&two), &one, &mut ahead);
        assert_eq!(ahead, ["/a#part"]);
        step(Some(&one), &two, &mut ahead);
        assert!(ahead.is_empty());
    }
}
