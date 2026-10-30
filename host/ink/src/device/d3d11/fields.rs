//! Window-thread editor state, drawn by Ink and drained by the presenter.
//! @ref LLP 1115.004 D7. Callbacks never borrow the presenter.
use super::{rich_edit::RichEdit, *};
use std::cell::RefCell;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct Field<'a> {
    pub id: u32,
    pub generation: u32,
    pub rect: [f32; 4],
    pub size: f32,
    pub color: [u8; 4],
    pub value: &'a str,
    pub placeholder: &'a str,
    pub selection: [u32; 2],
    pub focused: bool,
    pub multiline: bool,
    pub readonly: bool,
}

pub struct Edit {
    pub id: u32,
    pub generation: u32,
    pub value: String,
    pub selection: [u32; 2],
    pub focused: bool,
    pub composing: bool,
}

struct Editor {
    edit: RichEdit,
    generation: u32,
    rect: [f32; 4],
    focused: bool,
    composing: bool,
    changed: bool,
    seen: bool,
    value: String,
    selection: [u32; 2],
}

#[derive(Default)]
struct Fields {
    window: Option<Window>,
    device: Option<ID3D11Device>,
    editors: BTreeMap<u32, Editor>,
    focused: Option<u32>,
}
thread_local! { static FIELDS: RefCell<Fields> = RefCell::new(Fields::default()); }

pub fn enabled() -> bool {
    std::env::var("EXACT_INK_RICHEDIT").as_deref() == Ok("1")
}

pub fn clear() {
    FIELDS.with(|fields| *fields.borrow_mut() = Fields::default());
}

pub fn begin() {
    FIELDS.with(|f| {
        for editor in f.borrow_mut().editors.values_mut() {
            editor.seen = false;
        }
    });
}
pub fn end(device: &mut D3d11) -> Result<()> {
    FIELDS.with(|f| {
        let mut f = f.borrow_mut();
        let dead: Vec<_> = f
            .editors
            .iter()
            .filter_map(|(&id, e)| (!e.seen).then_some(id))
            .collect();
        for id in dead {
            if f.focused == Some(id) {
                f.focused = None;
            }
            let editor = f.editors.remove(&id).unwrap();
            device.retire_texture(editor.edit.texture())?;
        }
        Ok(())
    })
}

/// A refusal leaves the ordinary painted field in place.
pub fn paint(device: &mut D3d11, window: Window, field: &Field<'_>) -> Result<Option<TextureId>> {
    if !enabled() || field.rect[2] <= 0. || field.rect[3] <= 0. {
        return Ok(None);
    }
    FIELDS.with(|fields| {
        let mut fields = fields.borrow_mut();
        if fields.device.as_ref() != Some(&device.device) {
            *fields = Fields {
                device: Some(device.device.clone()),
                ..Default::default()
            };
        }
        fields.window = Some(window.clone());
        let size = [field.rect[2].ceil() as u32, field.rect[3].ceil() as u32];
        if fields
            .editors
            .get(&field.id)
            .is_some_and(|e| e.generation != field.generation)
        {
            let old = fields.editors.remove(&field.id).unwrap();
            device.retire_texture(old.edit.texture())?;
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = fields.editors.entry(field.id) {
            let edit = RichEdit::new(device, window, size)?;
            if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
                eprintln!(
                    "exact-windows: RichEdit field {} generation {}",
                    field.id, field.generation
                );
            }
            edit.message(WM_KILLFOCUS, 0, 0)?;
            entry.insert(Editor {
                edit,
                generation: field.generation,
                rect: field.rect,
                focused: false,
                composing: false,
                changed: false,
                seen: true,
                value: String::new(),
                selection: [0, 0],
            });
        }
        if field.focused {
            fields.focused = Some(field.id);
        } else if fields.focused == Some(field.id) {
            fields.focused = None;
        }
        let editor = fields.editors.get_mut(&field.id).unwrap();
        editor.seen = true;
        editor.rect = field.rect;
        editor.edit.resize(device, size)?;
        editor.edit.name(field.placeholder)?;
        editor.edit.configure(
            [field.rect[0].round() as i32, field.rect[1].round() as i32],
            field.size,
            field.color,
            field.multiline,
            field.readonly,
        )?;
        if !editor.composing && !editor.changed {
            if editor.value != field.value {
                editor.edit.set_text(field.value)?;
                field.value.clone_into(&mut editor.value);
            }
            if editor.selection != field.selection {
                editor.edit.select(field.selection)?;
                editor.selection = field.selection;
            }
        }
        if editor.focused != field.focused {
            editor.edit.message(
                if field.focused {
                    WM_SETFOCUS
                } else {
                    WM_KILLFOCUS
                },
                0,
                0,
            )?;
            editor.focused = field.focused;
        }
        editor.edit.draw(device).map(Some)
    })
}

pub fn caret(id: u32) -> Option<[f32; 4]> {
    FIELDS.with(|fields| {
        let fields = fields.borrow();
        let editor = fields.editors.get(&id)?;
        let [x, y, w, h] = editor.edit.caret()?;
        Some([
            editor.rect[0] + x as f32,
            editor.rect[1] + y as f32,
            w as f32,
            h as f32,
        ])
    })
}

pub fn take_edits() -> Vec<Edit> {
    FIELDS.with(|fields| {
        fields
            .borrow_mut()
            .editors
            .iter_mut()
            .filter_map(|(&id, e)| {
                if !std::mem::take(&mut e.changed) {
                    return None;
                }
                Some(Edit {
                    id,
                    generation: e.generation,
                    value: e.value.clone(),
                    selection: e.selection,
                    focused: e.focused,
                    composing: e.composing,
                })
            })
            .collect()
    })
}

pub fn identity(id: u32) -> Option<u32> {
    FIELDS.with(|f| f.borrow().editors.get(&id).map(|e| e.generation))
}
pub fn provider(
    id: u32,
    site: &windows::Win32::UI::Accessibility::IRawElementProviderWindowlessSite,
) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderSimple> {
    FIELDS.with(|f| {
        f.borrow()
            .editors
            .get(&id)
            .ok_or_else(|| Error::Invalid("field retired".into()))?
            .edit
            .provider(site)
    })
}

pub fn focused() -> bool {
    FIELDS.with(|f| f.borrow().focused.is_some())
}

pub fn message_hook() -> impl Fn(*const std::ffi::c_void) -> bool {
    message
}

/// Called by the carrier's message hook before winit translates an editor key.
/// The pointer is borrowed only during this call, as winit's hook requires.
fn message(raw: *const std::ffi::c_void) -> bool {
    if !enabled() {
        return false;
    }
    // SAFETY: the carrier passes the live MSG from EventLoopBuilder::with_msg_hook.
    let Some(message) = (unsafe { raw.cast::<MSG>().as_ref() }) else {
        return false;
    };
    FIELDS.with(|fields| {
        // Synchronous IME messages can nest a callback. The document callbacks
        // record changes only; a nested pump must not take a second mutable borrow.
        let Ok(mut fields) = fields.try_borrow_mut() else {
            return false;
        };
        if fields
            .window
            .as_ref()
            .is_none_or(|w| w.hwnd != message.hwnd)
        {
            return false;
        }
        let kind = message.message;
        let mouse = matches!(
            kind,
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_LBUTTONUP | WM_MOUSEMOVE
        );
        let point = [
            (message.lParam.0 as i16) as f32,
            ((message.lParam.0 >> 16) as i16) as f32,
        ];
        if matches!(kind, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK) {
            let hit = fields.editors.iter().find_map(|(&id, e)| {
                let [x, y, w, h] = e.rect;
                (point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h)
                    .then_some(id)
            });
            if fields.focused != hit {
                if let Some(old) = fields.focused.and_then(|id| fields.editors.get_mut(&id)) {
                    let _ = old.edit.message(WM_KILLFOCUS, 0, 0);
                    old.focused = false;
                    old.changed = true;
                }
                fields.focused = hit;
                if let Some(new) = hit.and_then(|id| fields.editors.get_mut(&id)) {
                    let _ = new.edit.message(WM_SETFOCUS, 0, 0);
                    new.focused = true;
                    new.changed = true;
                }
            }
        }
        let Some(id) = fields.focused else {
            return false;
        };
        let editor = fields.editors.get_mut(&id).unwrap();
        let ime = matches!(
            kind,
            WM_IME_STARTCOMPOSITION
                | WM_IME_COMPOSITION
                | WM_IME_ENDCOMPOSITION
                | WM_IME_CHAR
                | WM_IME_REQUEST
                | WM_IME_NOTIFY
                | WM_IME_SETCONTEXT
                | WM_IME_CONTROL
                | WM_IME_SELECT
                | WM_IME_KEYDOWN
                | WM_IME_KEYUP
        );
        let keyboard = matches!(
            kind,
            WM_KEYDOWN | WM_KEYUP | WM_CHAR | WM_SYSKEYDOWN | WM_SYSKEYUP | WM_SYSCHAR
        );
        if !mouse && !ime && !keyboard {
            return false;
        }
        if keyboard && !editor.composing && matches!(message.wParam.0, 9 | 13) {
            return false;
        }
        if mouse && kind == WM_MOUSEMOVE && message.wParam.0 & 1 == 0 {
            return false;
        }
        let lparam = if mouse {
            let x = (point[0] - editor.rect[0]).round() as i16;
            let y = (point[1] - editor.rect[1]).round() as i16;
            (x as u16 as isize) | ((y as u16 as isize) << 16)
        } else {
            message.lParam.0
        };
        if kind == WM_IME_STARTCOMPOSITION {
            editor.composing = true;
        }
        let confirming = editor.composing && keyboard && message.wParam.0 == 13;
        let handled = match editor.edit.dispatch(kind, message.wParam.0, lparam) {
            Ok(handled) => handled,
            Err(error) => {
                eprintln!("exact-windows: RichEdit input: {error}");
                return false;
            }
        };
        if kind == WM_IME_ENDCOMPOSITION {
            editor.composing = false;
        }
        if let Ok(value) = editor.edit.text() {
            editor.value = value;
        }
        if let Ok(selection) = editor.edit.selection() {
            editor.selection = selection;
        }
        editor.changed = true;
        if handled && matches!(kind, WM_KEYDOWN | WM_SYSKEYDOWN) {
            unsafe {
                let _ = TranslateMessage(message);
            }
        }
        handled || confirming
    })
}
