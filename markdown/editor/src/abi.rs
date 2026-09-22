//! Raw exports, no bindgen and no imports. Strings cross as UTF-16 in one
//! input buffer ([`mde_input`]); a mutating call answers with [`HANDLED`],
//! [`SOURCE`] and [`PLACE`] bits, and the host then reads what it needs:
//! the source ([`mde_source`]), the selection ([`mde_sel`]), the lines
//! ([`mde_view`] into [`mde_out`]) or the toolbar facts ([`mde_facts`] into
//! [`mde_text`]).

// `#[no_mangle]` is all the unsafe here: exported names, no unsafe blocks.
#![allow(unsafe_code)]

use std::cell::RefCell;

use crate::{view::flags, Change, Deco, Editor, Input, Outcome, Selected};

/// The call was the editor's: prevent the platform's default.
pub const HANDLED: u32 = 1;
/// The source changed: redraw and tell the app.
pub const SOURCE: u32 = 2;
/// Place the platform selection at the editor's.
pub const PLACE: u32 = 4;

#[derive(Default)]
struct Abi {
    editors: Vec<Option<Editor>>,
    input: Vec<u16>,
    out: Vec<u32>,
    text: Vec<u16>,
}

thread_local! {
    static ABI: RefCell<Abi> = RefCell::new(Abi::default());
}

fn bits(change: Change) -> u32 {
    HANDLED | if change.source { SOURCE } else { 0 } | if change.place { PLACE } else { 0 }
}

fn pair(a: u32, b: u32) -> Option<(u32, u32)> {
    (a != u32::MAX && b != u32::MAX).then_some((a, b))
}

/// Run `f` on editor `h` with the first `len` input code units.
fn with<R: Default>(h: u32, len: u32, f: impl FnOnce(&mut Editor, &[u16]) -> R) -> R {
    ABI.with(|abi| {
        let abi = &mut *abi.borrow_mut();
        let input = &abi.input[..(len as usize).min(abi.input.len())];
        match abi.editors.get_mut(h as usize) {
            Some(Some(editor)) => f(editor, input),
            _ => R::default(),
        }
    })
}

/// Room for `len` UTF-16 code units of input; the host writes them there.
#[no_mangle]
pub extern "C" fn mde_input(len: u32) -> *mut u16 {
    ABI.with(|abi| {
        let input = &mut abi.borrow_mut().input;
        input.clear();
        input.resize(len as usize, 0);
        input.as_mut_ptr()
    })
}

/// A new, empty editor's handle.
#[no_mangle]
pub extern "C" fn mde_new() -> u32 {
    ABI.with(|abi| {
        let editors = &mut abi.borrow_mut().editors;
        let free = editors
            .iter()
            .position(Option::is_none)
            .unwrap_or(editors.len());
        if free == editors.len() {
            editors.push(None);
        }
        editors[free] = Some(Editor::new());
        free as u32
    })
}

/// Free an editor.
#[no_mangle]
pub extern "C" fn mde_drop(h: u32) {
    ABI.with(|abi| {
        if let Some(slot) = abi.borrow_mut().editors.get_mut(h as usize) {
            *slot = None;
        }
    })
}

/// Load the input as the source; the caret goes to its end.
#[no_mangle]
pub extern "C" fn mde_load(h: u32, len: u32) {
    with(h, len, |e, text| e.load(text))
}

/// The app wrote the input as its value.
#[no_mangle]
pub extern "C" fn mde_set_value(h: u32, len: u32) -> u32 {
    with(h, len, |e, text| bits(e.set_value(text)))
}

/// The platform selection is `from..to`; `placed_*` is the selection the host
/// last placed, or `u32::MAX`. 0 ignore, 1 report, 2 place and report.
#[no_mangle]
pub extern "C" fn mde_select(h: u32, from: u32, to: u32, placed_from: u32, placed_to: u32) -> u32 {
    with(h, 0, |e, _| {
        match e.select(from, to, pair(placed_from, placed_to)) {
            Selected::Ignore => 0,
            Selected::Emit => 1,
            Selected::Place => 2,
        }
    })
}

/// Select everything.
#[no_mangle]
pub extern "C" fn mde_select_all(h: u32) -> u32 {
    with(h, 0, |e, _| bits(e.select_all()))
}

/// A `beforeinput` of `kind` (see `markup-editor.js`) with the input as its
/// data and an optional target range. Zero: let the platform do it.
#[no_mangle]
pub extern "C" fn mde_before_input(
    h: u32,
    kind: u32,
    len: u32,
    target_from: u32,
    target_to: u32,
    now: f64,
) -> u32 {
    let input = match kind {
        1 => Input::Undo,
        2 => Input::Redo,
        3 => Input::Bold,
        4 => Input::Italic,
        5 => Input::Strike,
        6 => Input::Format,
        7 => Input::Paragraph,
        8 => Input::Text,
        9 => Input::Replacement,
        10 => Input::Paste,
        11 => Input::Backward,
        12 => Input::Forward,
        13 => Input::OtherBackward,
        14 => Input::OtherForward,
        _ => Input::Other,
    };
    with(h, len, |e, data| {
        match e.before_input(input, data, pair(target_from, target_to), now) {
            Outcome::Native => 0,
            Outcome::Handled(change) => bits(change),
        }
    })
}

/// The platform changed the text to the input itself.
#[no_mangle]
pub extern "C" fn mde_reconcile(h: u32, len: u32, sel_from: u32, sel_to: u32, now: f64) -> u32 {
    with(h, len, |e, text| {
        bits(e.reconcile(text, pair(sel_from, sel_to), now))
    })
}

/// A command: the input is its name (`name_len` units) then its argument.
#[no_mangle]
pub extern "C" fn mde_command(h: u32, name_len: u32, argument_len: u32, now: f64) -> u32 {
    with(h, name_len.saturating_add(argument_len), |e, input| {
        let at = (name_len as usize).min(input.len());
        let (name, argument) = (
            String::from_utf16_lossy(&input[..at]),
            String::from_utf16_lossy(&input[at..]),
        );
        bits(e.command(&name, &argument, now))
    })
}

/// Toggle the task box of the item starting at `at`.
#[no_mangle]
pub extern "C" fn mde_toggle_task(h: u32, at: u32, now: f64) -> u32 {
    with(h, 0, |e, _| bits(e.toggle_task(at, now)))
}

/// Remove the selection's visible text after a cut.
#[no_mangle]
pub extern "C" fn mde_cut(h: u32, now: f64) -> u32 {
    with(h, 0, |e, _| bits(e.cut(now)))
}

/// The source range a copy takes, into [`mde_out`]; zero when there is none.
#[no_mangle]
pub extern "C" fn mde_copy(h: u32) -> u32 {
    let range = with(h, 0, |e, _| e.copy_range());
    ABI.with(|abi| {
        let out = &mut abi.borrow_mut().out;
        out.clear();
        out.extend(range.map(|(a, b)| [a, b]).into_iter().flatten());
        u32::from(range.is_some())
    })
}

/// 1 when a caret at `p` draws after the hidden syntax around it.
#[no_mangle]
pub extern "C" fn mde_draws_after(h: u32, p: u32) -> u32 {
    with(h, 0, |e, _| u32::from(e.draws_after(p)))
}

/// The selection's start (`which` 0) or end.
#[no_mangle]
pub extern "C" fn mde_sel(h: u32, which: u32) -> u32 {
    with(h, 0, |e, _| {
        if which == 0 {
            e.selection().0
        } else {
            e.selection().1
        }
    })
}

/// The source's code units; valid until the next call.
#[no_mangle]
pub extern "C" fn mde_source(h: u32) -> *const u16 {
    ABI.with(|abi| match abi.borrow().editors.get(h as usize) {
        Some(Some(e)) => e.source().as_ptr(),
        _ => std::ptr::null(),
    })
}

/// The source's length in code units.
#[no_mangle]
pub extern "C" fn mde_source_len(h: u32) -> u32 {
    with(h, 0, |e, _| e.source().len() as u32)
}

/// The lines into [`mde_out`]; returns its length. `[count]`, then per line
/// `start, end, heading, quote, depth, flags, deco, deco_a, deco_b, count`
/// and `count` segments of `start, end, style`, positions after `end`
/// relative to the line's start so an unchanged line reads the same.
#[no_mangle]
pub extern "C" fn mde_view(h: u32) -> u32 {
    let lines = with(h, 0, |e, _| e.lines());
    ABI.with(|abi| {
        let out = &mut abi.borrow_mut().out;
        out.clear();
        out.push(lines.len() as u32);
        for l in lines {
            let (deco, a, b) = match l.deco {
                None => (0, 0, 0),
                Some(Deco::Bullet) => (1, 0, 0),
                Some(Deco::Number(a, b)) => {
                    (2, a.saturating_sub(l.start), b.saturating_sub(l.start))
                }
                Some(Deco::Task(done)) => (3, u32::from(done), 0),
                Some(Deco::Rule) => (4, 0, 0),
            };
            let f = u32::from(
                l.flags
                    & (flags::CODE
                        | flags::COLLAPSED
                        | flags::CODE_FIRST
                        | flags::CODE_LAST
                        | flags::LIST),
            );
            out.extend([
                l.start,
                l.end,
                l.heading.into(),
                l.quote.into(),
                l.depth.into(),
                f,
                deco,
                a,
                b,
            ]);
            out.push(l.segments.len() as u32);
            for (s, e, style) in l.segments {
                out.extend([s - l.start, e - l.start, style.into()]);
            }
        }
        out.len() as u32
    })
}

/// The last u32 output.
#[no_mangle]
pub extern "C" fn mde_out() -> *const u32 {
    ABI.with(|abi| abi.borrow().out.as_ptr())
}

/// The last UTF-16 output.
#[no_mangle]
pub extern "C" fn mde_text() -> *const u16 {
    ABI.with(|abi| abi.borrow().text.as_ptr())
}

fn put_text(text: &str) -> u32 {
    ABI.with(|abi| {
        let out = &mut abi.borrow_mut().text;
        out.clear();
        out.extend(text.encode_utf16());
        out.len() as u32
    })
}

/// The `select` event payload (formats, mixed, unavailable, link; one per
/// line) into [`mde_text`]; returns its length.
#[no_mangle]
pub extern "C" fn mde_facts(h: u32) -> u32 {
    let facts = with(h, 0, |e, _| e.facts());
    let mixed = if facts.mixed { "1" } else { "0" };
    put_text(
        &[
            facts.formats.as_str(),
            mixed,
            &facts.unavailable,
            &facts.link,
        ]
        .join("\n"),
    )
}

/// The input as plain text, for "copy plain", into [`mde_text`].
#[no_mangle]
pub extern "C" fn mde_plain(len: u32) -> u32 {
    let source = ABI.with(|abi| {
        let abi = abi.borrow();
        String::from_utf16_lossy(&abi.input[..(len as usize).min(abi.input.len())])
    });
    put_text(&exact_markdown::plain(&source))
}
