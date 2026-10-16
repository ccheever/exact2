//! The reader's own hatches, beside the app's Rust ones (@ref LLP 1115.001.000
//! D4): on Android the Canvas reader runs the app's Kotlin hatches
//! (`modules/android/*.kt`), so every moment a Rust hatch hears is told to
//! it too, and what its handles ask comes back here by the node's number.
//!
//! The moments are words the host puts on the request channel
//! (`Request::Hatch`), each led by its scope:
//!
//! - 1 the app and 3 a node: the moment (0 built, 1 changed, 2 ended), the
//!   node's number, its box (four floats as bits, the document's points),
//!   its word and authored `id` as strings, then the count of its `data-*`
//!   words and each name and value. The app's moment has no node, box, word
//!   or id (all zero or empty) and carries the root's `data-*` words.
//! - 5 a frame ticket's tick: the ticket, the session clock (an `f64`'s
//!   bits, high word first) and the commit's sequence number.
//! - 6 an `after` that fired: the ticket.
//! - 7 input an observer heard: the ticket and the node, then 1 and a
//!   pointer's phase (0 down, 1 move, 2 up, 3 cancel), its point and its
//!   point in the node (floats as bits) and whether Exact handled it; or 2
//!   and a key's `key` and `code` as strings, then whether it went down,
//!   repeats, and was handled.
//!
//! What the reader asks ([`act`]) is a Rust hatch's every call: an act on a
//! node, a frame ticket, an `after`, an observer, a stop, the diagnostics,
//! and a view of its own at a node's box, which the painter names to the
//! reader where the node is (`NativeKind::Hatch`).

use super::{App, Diagnostics, Element, Frame, Input, Module, Phase, Rect, Shared, Window};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// What a ticket of the reader's is for.
enum Mine {
    Frames,
    After,
    Heard(u32),
}

thread_local! {
    /// The moments told since the host last took them.
    static TOLD: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// Each hatched node that is in the tree, by its number: what an act
    /// from the reader names.
    static NODES: RefCell<BTreeMap<u32, Element>> = const { RefCell::new(BTreeMap::new()) };
    /// The session's registrations, which the reader's tickets are made in.
    static SHARED: RefCell<Option<Rc<Shared>>> = const { RefCell::new(None) };
    /// The reader's tickets.
    static MINE: RefCell<BTreeMap<u64, Mine>> = const { RefCell::new(BTreeMap::new()) };
    /// The nodes whose box shows a view of the reader's hatches, and how
    /// many times each has been given one.
    static HOSTED: RefCell<BTreeMap<u32, u32>> = const { RefCell::new(BTreeMap::new()) };
}

/// The app's module, and the reader told the same moments after it.
pub(crate) struct Beside(pub(crate) Box<dyn Module>);

impl Beside {
    pub(crate) fn new(module: Box<dyn Module>, shared: Rc<Shared>) -> Beside {
        SHARED.with(|s| *s.borrow_mut() = Some(shared));
        Beside(module)
    }
}

fn mine(token: u64, what: fn(&Mine) -> bool) -> bool {
    MINE.with(|m| m.borrow().get(&token).is_some_and(what))
}

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
        if ended {
            HOSTED.with(|h| h.borrow_mut().remove(&element.node()));
        }
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
        if mine(token, |m| matches!(m, Mine::Frames)) {
            let bits = frame.now.to_bits();
            words(&[
                5,
                token as i32,
                (bits >> 32) as i32,
                bits as i32,
                frame.seq as i32,
            ]);
        }
    }
    fn fired(&mut self, token: u64) {
        self.0.fired(token);
        if mine(token, |m| matches!(m, Mine::After)) {
            MINE.with(|m| m.borrow_mut().remove(&token));
            words(&[6, token as i32]);
        }
    }
    fn heard(&mut self, token: u64, input: &Input) {
        self.0.heard(token, input);
        let Some(node) = MINE.with(|m| match m.borrow().get(&token) {
            Some(Mine::Heard(node)) => Some(*node),
            _ => None,
        }) else {
            return;
        };
        TOLD.with(|told| {
            let out = &mut *told.borrow_mut();
            out.extend([7, token as i32, node as i32]);
            let bits = |f: f32| f.to_bits() as i32;
            match input {
                Input::Pointer {
                    phase,
                    x,
                    y,
                    local,
                    handled,
                } => {
                    let phase = match phase {
                        Phase::Down => 0,
                        Phase::Move => 1,
                        Phase::Up => 2,
                        Phase::Cancel => 3,
                    };
                    out.extend([1, phase, bits(*x), bits(*y), bits(local.0), bits(local.1)]);
                    out.push(i32::from(*handled));
                }
                Input::Key {
                    key,
                    code,
                    down,
                    repeat,
                    handled,
                } => {
                    out.push(2);
                    string(out, key);
                    string(out, code);
                    out.extend([*down, *repeat, *handled].map(i32::from));
                }
            }
        });
    }
    fn forget(&mut self) {
        self.0.forget();
        NODES.with(|nodes| nodes.borrow_mut().clear());
        MINE.with(|m| m.borrow_mut().clear());
        HOSTED.with(|h| h.borrow_mut().clear());
    }
}

fn moment(new: bool, ended: bool) -> i32 {
    match (new, ended) {
        (_, true) => 2,
        (true, false) => 0,
        (false, false) => 1,
    }
}

fn words(w: &[i32]) {
    TOLD.with(|told| told.borrow_mut().extend_from_slice(w));
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

/// How many times `node`'s box has been given a view of the reader's
/// hatches, while it shows one: what the painter names it with.
pub(crate) fn hosted(node: u32) -> Option<u32> {
    HOSTED.with(|h| h.borrow().get(&node).copied())
}

/// What the reader's hatches ask, for `node` (0 for the module's own):
///
/// | kind | asks | `text` | answer |
/// |---|---|---|---|
/// | 1–4 | `click`, `focus`, `blur`, `input` | `input`'s value | 1 |
/// | 5 | a frame ticket | | its ticket |
/// | 6 | an `after` | its ms | its ticket |
/// | 7 | to observe the node's input | | its ticket |
/// | 8 | to stop a ticket | the ticket | 1 |
/// | 9–12 | `log`, `count`, `measure`, `publish` | the line; or a name, a newline and the number or JSON | 1 |
/// | 13, 14 | to host a view at the node's box, or no longer | | how many times it has been given one; 1 |
///
/// An act on a node is queued as a Rust hatch's is, and run between turns.
/// -1 when the node is not a hatched node in the tree, or the ask is not
/// one of these.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn act(node: u32, kind: i32, text: Option<&str>) -> i64 {
    let element = NODES.with(|nodes| nodes.borrow().get(&node).cloned());
    let shared = SHARED.with(|s| s.borrow().clone());
    let (Some(shared), text) = (shared, text.unwrap_or_default()) else {
        return -1;
    };
    let register = |token: u64, what: Mine| {
        MINE.with(|m| m.borrow_mut().insert(token, what));
        token as i64
    };
    let diagnostics = || match &element {
        Some(e) => e.diagnostics(),
        None => Diagnostics {
            shared: shared.clone(),
            scope: Rc::from("module"),
            node: 0,
        },
    };
    let named = || text.split_once('\n').unwrap_or((text, ""));
    match (kind, &element) {
        (1, Some(e)) => e.click(),
        (2, Some(e)) => e.focus(),
        (3, Some(e)) => e.blur(),
        (4, Some(e)) => e.input(text),
        (5, _) => return register(shared.frames(), Mine::Frames),
        (6, _) => return register(shared.after(text.parse().unwrap_or(0.0)), Mine::After),
        (7, Some(e)) => return register(shared.observe(&e.node), Mine::Heard(node)),
        (8, _) => {
            let Ok(token) = text.parse::<u64>() else {
                return -1;
            };
            if MINE.with(|m| m.borrow_mut().remove(&token)).is_none() {
                return -1;
            }
            shared.stop(token);
        }
        (9, _) => diagnostics().log(text),
        (10, _) => diagnostics().count(named().0, named().1.parse().unwrap_or(1)),
        (11, _) => diagnostics().measure(named().0, named().1.parse().unwrap_or(0.0)),
        (12, _) => diagnostics().publish(named().0, named().1),
        (13, Some(_)) => {
            return HOSTED.with(|h| {
                let mut h = h.borrow_mut();
                let n = h.entry(node).or_insert(0);
                *n += 1;
                i64::from(*n)
            })
        }
        (14, _) => {
            HOSTED.with(|h| h.borrow_mut().remove(&node));
        }
        _ => return -1,
    }
    1
}
