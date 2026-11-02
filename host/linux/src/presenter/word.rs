//! Use the installed Rich Edit engine's Ctrl+Arrow boundaries, including its
//! language and punctuation rules. The message-only control never takes focus.
//! https://learn.microsoft.com/windows/win32/controls/em-findwordbreak
use std::cell::OnceCell;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
        UI::{
            Controls::{
                RichEdit::{EM_EXLIMITTEXT, EM_FINDWORDBREAK, EM_SETUNDOLIMIT, MSFTEDIT_CLASS},
                WB_MOVEWORDLEFT, WB_MOVEWORDRIGHT, WB_RIGHTBREAK,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, SendMessageW, SetWindowTextW, ES_MULTILINE,
                HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
            },
        },
    },
};

struct Words {
    window: HWND,
    library: HMODULE,
}
impl Words {
    fn new() -> Option<Self> {
        // SAFETY: the library is a system DLL and outlives its window. This
        // object stays in the thread-local cell on the creating UI thread.
        #[allow(unsafe_code)]
        unsafe {
            let library =
                LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
            let window = match CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                MSFTEDIT_CLASS,
                w!(""),
                WINDOW_STYLE(ES_MULTILINE as u32),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            ) {
                Ok(window) => window,
                Err(_) => {
                    let _ = FreeLibrary(library);
                    return None;
                }
            };
            SendMessageW(window, EM_SETUNDOLIMIT, Some(WPARAM(0)), None);
            SendMessageW(
                window,
                EM_EXLIMITTEXT,
                None,
                Some(LPARAM(i32::MAX as isize)),
            );
            Some(Self { window, library })
        }
    }

    fn find(&self, text: &str, at: u32, action: i32) -> Option<u32> {
        if text.contains(['\0', '\r']) {
            return None;
        }
        // Rich Edit stores a paragraph break as CR. One LF maps to one CR,
        // preserving every UTF-16 offset in the presenter's value.
        let mut wide: Vec<u16> = text
            .encode_utf16()
            .map(|c| if c == 10 { 13 } else { c })
            .collect();
        let len = u32::try_from(wide.len())
            .ok()
            .filter(|&n| n < i32::MAX as u32)?;
        wide.push(0);
        // SAFETY: the UTF-16 storage lives through the synchronous messages;
        // this thread owns the control and empties it before returning.
        #[allow(unsafe_code)]
        unsafe {
            SetWindowTextW(self.window, PCWSTR(wide.as_ptr())).ok()?;
            let found = SendMessageW(
                self.window,
                EM_FINDWORDBREAK,
                Some(WPARAM(action as usize)),
                Some(LPARAM(at.min(len) as isize)),
            )
            .0;
            let _ = SetWindowTextW(self.window, w!(""));
            u32::try_from(found).ok().map(|n| n.min(len))
        }
    }
}
impl Drop for Words {
    fn drop(&mut self) {
        // SAFETY: the window is destroyed on its creating thread before the
        // DLL that owns its window procedure is released.
        #[allow(unsafe_code)]
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = FreeLibrary(self.library);
        }
    }
}

fn with_words<R>(read: impl FnOnce(&Words) -> Option<R>) -> Option<R> {
    thread_local! { static WORDS: OnceCell<Option<Words>> = const { OnceCell::new() }; }
    WORDS.with(|words| read(words.get_or_init(Words::new).as_ref()?))
}

pub(super) fn step(text: &str, at: u32, forward: bool) -> Option<u32> {
    let action = if forward {
        WB_MOVEWORDRIGHT
    } else {
        WB_MOVEWORDLEFT
    };
    with_words(|words| words.find(text, at, action.0))
}

pub(super) fn range(text: &str, at: u32) -> Option<(u32, u32)> {
    let byte = super::field::byte_at(text, at);
    let character = text[byte..].chars().next()?;
    if character.is_whitespace() {
        let same = |c: char| c.is_whitespace() && c != '\n' && c != '\r';
        let start = if same(character) {
            text[..byte]
                .rfind(|c| !same(c))
                .map_or(0, |i| i + text[i..].chars().next().unwrap().len_utf8())
        } else {
            byte
        };
        let end = if same(character) {
            text[byte..]
                .find(|c| !same(c))
                .map_or(text.len(), |i| byte + i)
        } else {
            byte + character.len_utf8()
        };
        return Some((
            super::field::utf16_len(&text[..start]),
            super::field::utf16_len(&text[..end]),
        ));
    }
    with_words(|words| {
        let after = super::field::utf16_len(&text[..byte]) + character.len_utf16() as u32;
        let start = words.find(text, after, WB_MOVEWORDLEFT.0)?;
        let end = words.find(text, at, WB_RIGHTBREAK.0)?;
        (start <= at && end > at).then_some((start, end))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_edit_word_selection_preserves_unicode_offsets_and_excludes_spaces() {
        for (text, at, expected) in [
            ("café two", 2, (0, 4)),
            ("🦀 two", 4, (3, 6)),
            ("one  two", 3, (3, 5)),
            ("one\ntwo", 5, (4, 7)),
        ] {
            assert_eq!(range(text, at), Some(expected), "{text:?} at={at}");
        }
        assert_eq!(
            range("🦀 two", 0),
            None,
            "the caller supplies Unicode boundaries when Rich Edit declines a symbol"
        );
    }

    #[test]
    fn rich_edit_arrows_find_word_starts_and_preserve_utf16_offsets() {
        for (text, at, forward, expected) in [
            ("one two three", 0, true, 4),
            ("one two three", 4, true, 8),
            ("one two three", 13, false, 8),
            ("one two three", 8, false, 4),
            ("one two three", 13, true, 13),
            ("one two three", 0, false, 0),
            ("🦀 two", 0, true, 3),
            ("🦀 two", 6, false, 3),
            ("one\ntwo", 0, true, 3),
            ("one\ntwo", 3, true, 4),
        ] {
            assert_eq!(
                step(text, at, forward),
                Some(expected),
                "{text:?} at={at} forward={forward}"
            );
        }
        assert_eq!(step("one\0two", 0, true), None);
        assert_eq!(step("one\r\ntwo", 0, true), None);
    }

    #[test]
    fn word_shortcuts_select_delete_and_undo_through_input_events() {
        use crate::presenter::{PainterChoice, Presenter};
        use exact_kernel::PropId;
        use exact_runner::{FieldSelection, SelectionDirection};
        let source = r#"component App
  state value = "one 🦀 two"
  state heard = ""
  state blocked = false
  action edited(v: string, e: InputEvent)
    value = v
    heard = e.inputType
  action before(e: InputEvent)
    if blocked
      preventDefault()
  action block
    blocked = true
  view
    column
      textarea testId="editor" value=value input=edited beforeinput=before
      text heard testId="heard"
      button "Block" testId="block" press=block
"#;
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (400., 400.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let id = |p: &Presenter<()>, name| {
            let k = p.host.kernel();
            k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
        };
        let editor = id(&p, "editor");
        let heard = id(&p, "heard");
        let chord = |p: &mut Presenter<()>, key: &str, shift| {
            p.hardware_key("ControlLeft", "Control", true, false);
            if shift {
                p.hardware_key("ShiftLeft", "Shift", true, false);
            }
            p.hardware_key(key, key, true, false);
            p.hardware_key(key, key, false, false);
            if shift {
                p.hardware_key("ShiftLeft", "Shift", false, false);
            }
            p.hardware_key("ControlLeft", "Control", false, false);
        };
        p.set_focus(Some(editor), 0.);
        chord(&mut p, "End", false);
        chord(&mut p, "ArrowLeft", false);
        assert_eq!(p.field_selection(editor), FieldSelection::caret(7));
        chord(&mut p, "ArrowLeft", false);
        chord(&mut p, "ArrowRight", true);
        assert_eq!(
            p.field_selection(editor),
            FieldSelection {
                start: 4,
                end: 7,
                direction: SelectionDirection::Forward,
            }
        );
        chord(&mut p, "Backspace", false);
        assert_eq!(p.field_text(editor), "one two");
        assert_eq!(
            p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
            Some("deleteWordBackward")
        );
        chord(&mut p, "z", false);
        assert_eq!(p.field_text(editor), "one 🦀 two");
        assert_eq!(
            (
                p.field_selection(editor).start,
                p.field_selection(editor).end
            ),
            (4, 7)
        );
        chord(&mut p, "End", false);
        chord(&mut p, "Backspace", false);
        assert_eq!(p.field_text(editor), "one 🦀 ");
        chord(&mut p, "z", false);
        chord(&mut p, "Home", false);
        chord(&mut p, "Delete", false);
        assert_eq!(p.field_text(editor), "🦀 two");
        assert_eq!(
            p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
            Some("deleteWordForward")
        );
        p.tap(id(&p, "block")).unwrap();
        p.set_focus(Some(editor), 0.);
        chord(&mut p, "Delete", false);
        assert_eq!(
            p.field_text(editor),
            "🦀 two",
            "beforeinput can cancel word deletion"
        );
        chord(&mut p, "End", false);
        chord(&mut p, "Backspace", false);
        assert_eq!(p.field_text(editor), "🦀 two");
    }
}
