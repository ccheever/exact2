//! The reader's own hatches, beside the app's Rust ones (@ref LLP 1115.001.000
//! D4): on Android the Canvas reader runs the app's Kotlin hatches
//! (`modules/android/*.kt`), so every moment a Rust hatch hears is told to
//! it too, and what its handles ask comes back here by the node's number.
//!
//! The moments are words the host puts on the request channel
//! (`Request::Hatch`); a moment is the scope (1 the app, 3 a node), the
//! moment (0 built, 1 changed, 2 ended), the node's number, its box (four
//! floats as bits, the document's points), its word and authored `id` as
//! strings, then the count of its `data-*` words and each name and value.
//! The app's moment has no node, box, word or id (all zero or empty) and
//! carries the root's `data-*` words.

use super::{App, Element, Frame, Input, Module, Rect, Window};
use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    /// The moments told since the host last took them.
    static TOLD: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// Each hatched node that is in the tree, by its number: what an act
    /// from the reader names.
    static NODES: RefCell<BTreeMap<u32, Element>> = const { RefCell::new(BTreeMap::new()) };
}

/// The app's module, and the reader told the same moments after it.
pub(crate) struct Beside(pub(crate) Box<dyn Module>);

impl Module for Beside {
    fn app(&mut self, app: &App, ended: bool) {
        self.0.app(app, ended);
        tell(
            1,
            moment(app.is_new, ended),
            0,
            Rect::default(),
            "",
            "",
            &app.data,
        );
    }
    fn window(&mut self, window: &Window, ended: bool) {
        self.0.window(window, ended);
    }
    fn element(&mut self, element: &Element, ended: bool) {
        self.0.element(element, ended);
        NODES.with(|nodes| {
            let mut nodes = nodes.borrow_mut();
            if ended {
                nodes.remove(&element.node());
            } else {
                nodes.insert(element.node(), element.clone());
            }
        });
        let moment = moment(element.is_new(), ended);
        let (frame, words) = (element.frame(), element.words());
        tell(
            3,
            moment,
            element.node(),
            frame,
            element.hatch(),
            element.id(),
            &words,
        );
    }
    fn tick(&mut self, token: u64, frame: Frame) {
        self.0.tick(token, frame);
    }
    fn fired(&mut self, token: u64) {
        self.0.fired(token);
    }
    fn heard(&mut self, token: u64, input: &Input) {
        self.0.heard(token, input);
    }
    fn forget(&mut self) {
        self.0.forget();
        NODES.with(|nodes| nodes.borrow_mut().clear());
    }
}

fn moment(new: bool, ended: bool) -> i32 {
    match (new, ended) {
        (_, true) => 2,
        (true, false) => 0,
        (false, false) => 1,
    }
}

fn tell(
    scope: i32,
    moment: i32,
    node: u32,
    frame: Rect,
    word: &str,
    id: &str,
    data: &BTreeMap<String, String>,
) {
    TOLD.with(|told| {
        let out = &mut *told.borrow_mut();
        out.extend([scope, moment, node as i32]);
        out.extend([frame.x, frame.y, frame.width, frame.height].map(|f| f.to_bits() as i32));
        string(out, word);
        string(out, id);
        out.push(data.len() as i32);
        for (name, value) in data {
            string(out, name);
            string(out, value);
        }
    });
}

/// Length, then UTF-8 padded to whole words, as the request channel has a
/// string.
fn string(out: &mut Vec<i32>, s: &str) {
    out.push(s.len() as i32);
    for chunk in s.as_bytes().chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        out.push(i32::from_le_bytes(word));
    }
}

/// The moments told since the last take, if any.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn take() -> Option<Vec<i32>> {
    TOLD.with(|told| {
        let told = std::mem::take(&mut *told.borrow_mut());
        (!told.is_empty()).then_some(told)
    })
}

/// What the reader's handle for `node` asks: 1 `click`, 2 `focus`, 3 `blur`,
/// 4 `input(text)`. Queued as a Rust hatch's act is, and run between turns.
/// False when no hatched node of that number is in the tree.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn act(node: u32, kind: i32, text: Option<&str>) -> bool {
    let Some(element) = NODES.with(|nodes| nodes.borrow().get(&node).cloned()) else {
        return false;
    };
    match kind {
        1 => element.click(),
        2 => element.focus(),
        3 => element.blur(),
        4 => element.input(text.unwrap_or_default()),
        _ => return false,
    }
    true
}
