//! Windowless RichEdit callbacks. They retain field state, never the presenter.
//! @ref LLP 1115.004 D7.1.6.
#![allow(non_snake_case, non_camel_case_types)]
use super::composition::Window;
use std::{cell::Cell, rc::Rc};
use windows::core::{IUnknown, Interface, BOOL, GUID};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::RichEdit::*, Input::Ime::*, WindowsAndMessaging::*},
};

// windows-rs 0.62 publishes a zero IID for both text hosts. Keep its ABI but
// supply the SDK identity. https://github.com/microsoft/win32metadata/issues/1952
windows_core::imp::define_interface!(
    TextHost,
    TextHost_Vtbl,
    0x13e670f5_1a5a_11cf_abeb_00aa00b65ea1
);
windows_core::imp::interface_hierarchy!(TextHost, IUnknown);
#[repr(transparent)]
pub struct TextHost_Vtbl(ITextHost2_Vtbl);
impl TextHost_Vtbl {
    pub const fn new<T: ITextHost2_Impl, const OFFSET: isize>() -> Self {
        Self(ITextHost2_Vtbl::new::<T, OFFSET>())
    }
    pub fn matches(iid: &GUID) -> bool {
        *iid == TextHost::IID || *iid == GUID::from_u128(0xc5bdd8d0_d26e_11ce_a89e_00aa006cadc5)
    }
}
impl windows_core::RuntimeName for TextHost {}

pub(super) struct State {
    pub rect: Cell<RECT>,
    pub dirty: Cell<bool>,
    pub origin: Cell<POINT>,
    pub caret: Cell<[i32; 4]>,
    pub caret_visible: Cell<bool>,
    pub properties: Cell<u32>,
    window: Window,
    character: CHARFORMATW,
    paragraph: PARAFORMAT,
}
impl State {
    pub fn new(window: Window, size: [u32; 2]) -> Rc<Self> {
        let mut character = CHARFORMATW {
            cbSize: size_of::<CHARFORMATW>() as u32,
            dwMask: CFM_FACE | CFM_SIZE | CFM_COLOR,
            yHeight: 240,
            crTextColor: COLORREF(0x202020),
            ..Default::default()
        };
        let face: Vec<_> = "Segoe UI".encode_utf16().collect();
        character.szFaceName[..face.len()].copy_from_slice(&face);
        Rc::new(Self {
            rect: Cell::new(RECT {
                right: size[0] as i32,
                bottom: size[1] as i32,
                ..Default::default()
            }),
            dirty: Cell::new(true),
            origin: Cell::new(POINT::default()),
            caret: Cell::new([0; 4]),
            caret_visible: Cell::new(false),
            properties: Cell::new(
                TXTBIT_RICHTEXT
                    | TXTBIT_MULTILINE
                    | TXTBIT_WORDWRAP
                    | TXTBIT_D2DDWRITE
                    | TXTBIT_DISABLEDRAG,
            ),
            window,
            character,
            paragraph: PARAFORMAT {
                cbSize: size_of::<PARAFORMAT>() as u32,
                dwMask: PFM_ALIGNMENT,
                wAlignment: PFA_LEFT,
                ..Default::default()
            },
        })
    }
}
#[windows_core::implement(TextHost)]
pub(super) struct Host(pub Rc<State>);

// RichEdit owns the callback pointers and supplies their output storage.
fn put<T>(p: *mut T, value: T) -> windows_core::Result<()> {
    if p.is_null() {
        return Err(windows_core::Error::from_hresult(E_POINTER));
    }
    unsafe {
        p.write(value);
    }
    Ok(())
}
impl ITextHost_Impl for Host_Impl {
    fn TxGetDC(&self) -> HDC {
        unsafe { GetDC(Some(self.0.window.hwnd)) }
    }
    fn TxReleaseDC(&self, dc: HDC) -> i32 {
        unsafe { ReleaseDC(Some(self.0.window.hwnd), dc) }
    }
    fn TxShowScrollBar(&self, _: i32, _: BOOL) -> BOOL {
        false.into()
    }
    fn TxEnableScrollBar(&self, _: SCROLLBAR_CONSTANTS, _: i32) -> BOOL {
        false.into()
    }
    fn TxSetScrollRange(&self, _: i32, _: i32, _: i32, _: BOOL) -> BOOL {
        false.into()
    }
    fn TxSetScrollPos(&self, _: i32, _: i32, _: BOOL) -> BOOL {
        false.into()
    }
    fn TxInvalidateRect(&self, _: *mut RECT, _: BOOL) {
        self.0.dirty.set(true);
    }
    fn TxViewChange(&self, _: BOOL) {
        self.0.dirty.set(true);
    }
    fn TxCreateCaret(&self, _: HBITMAP, width: i32, height: i32) -> BOOL {
        let [x, y, ..] = self.0.caret.get();
        self.0.caret.set([x, y, width.max(1), height]);
        true.into()
    }
    fn TxShowCaret(&self, show: BOOL) -> BOOL {
        self.0.caret_visible.set(show.as_bool());
        self.0.dirty.set(true);
        true.into()
    }
    fn TxSetCaretPos(&self, x: i32, y: i32) -> BOOL {
        let [_, _, w, h] = self.0.caret.get();
        self.0.caret.set([x, y, w, h]);
        true.into()
    }
    fn TxSetTimer(&self, _: u32, _: u32) -> BOOL {
        false.into()
    }
    fn TxKillTimer(&self, _: u32) {}
    fn TxScrollWindowEx(
        &self,
        _: i32,
        _: i32,
        _: *mut RECT,
        _: *mut RECT,
        _: HRGN,
        _: *mut RECT,
        _: SCROLL_WINDOW_FLAGS,
    ) {
        self.0.dirty.set(true);
    }
    fn TxSetCapture(&self, capture: BOOL) {
        unsafe {
            if capture.as_bool() {
                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.0.window.hwnd);
            } else {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
        }
    }
    fn TxSetFocus(&self) {
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(self.0.window.hwnd));
        }
    }
    fn TxSetCursor(&self, cursor: HCURSOR, _: BOOL) {
        unsafe {
            SetCursor(Some(cursor));
        }
    }
    fn TxScreenToClient(&self, point: *mut POINT) -> BOOL {
        unsafe {
            let result = ScreenToClient(self.0.window.hwnd, point);
            if result.as_bool() && !point.is_null() {
                (*point).x -= self.0.origin.get().x;
                (*point).y -= self.0.origin.get().y;
            }
            result
        }
    }
    fn TxClientToScreen(&self, point: *mut POINT) -> BOOL {
        unsafe {
            if point.is_null() {
                return false.into();
            }
            (*point).x += self.0.origin.get().x;
            (*point).y += self.0.origin.get().y;
            ClientToScreen(self.0.window.hwnd, point)
        }
    }
    fn TxActivate(&self, old: *mut i32) -> windows_core::Result<()> {
        put(old, 0)
    }
    fn TxDeactivate(&self, _: i32) -> windows_core::Result<()> {
        Ok(())
    }
    fn TxGetClientRect(&self, out: *mut RECT) -> windows_core::Result<()> {
        put(out, self.0.rect.get())
    }
    fn TxGetViewInset(&self, out: *mut RECT) -> windows_core::Result<()> {
        put(out, RECT::default())
    }
    fn TxGetCharFormat(&self, out: *const *const CHARFORMATW) -> windows_core::Result<()> {
        put(out.cast_mut(), &self.0.character)
    }
    fn TxGetParaFormat(&self, out: *const *const PARAFORMAT) -> windows_core::Result<()> {
        put(out.cast_mut(), &self.0.paragraph)
    }
    fn TxGetSysColor(&self, index: SYS_COLOR_INDEX) -> COLORREF {
        COLORREF(unsafe { GetSysColor(index) })
    }
    fn TxGetBackStyle(&self, out: *mut TXTBACKSTYLE) -> windows_core::Result<()> {
        put(out, TXTBACK_TRANSPARENT)
    }
    fn TxGetMaxLength(&self, out: *mut u32) -> windows_core::Result<()> {
        put(out, 32767)
    }
    fn TxGetScrollBars(&self, out: *mut u32) -> windows_core::Result<()> {
        put(out, 0)
    }
    fn TxGetPasswordChar(&self) -> windows_core::Result<i8> {
        Ok(0)
    }
    fn TxGetAcceleratorPos(&self, out: *mut i32) -> windows_core::Result<()> {
        put(out, -1)
    }
    fn TxGetExtent(&self, out: *mut SIZE) -> windows_core::Result<()> {
        let r = self.0.rect.get();
        put(
            out,
            SIZE {
                cx: r.right * 2540 / 96,
                cy: r.bottom * 2540 / 96,
            },
        )
    }
    fn OnTxCharFormatChange(&self, _: *const CHARFORMATW) -> windows_core::Result<()> {
        Ok(())
    }
    fn OnTxParaFormatChange(&self, _: *const PARAFORMAT) -> windows_core::Result<()> {
        Ok(())
    }
    fn TxGetPropertyBits(&self, mask: u32, out: *mut u32) -> windows_core::Result<()> {
        put(out, mask & self.0.properties.get())
    }
    fn TxNotify(&self, _: u32, _: *mut std::ffi::c_void) -> windows_core::Result<()> {
        self.0.dirty.set(true);
        Ok(())
    }
    fn TxImmGetContext(&self) -> HIMC {
        unsafe { ImmGetContext(self.0.window.hwnd) }
    }
    fn TxImmReleaseContext(&self, context: HIMC) {
        unsafe {
            let _ = ImmReleaseContext(self.0.window.hwnd, context);
        }
    }
    fn TxGetSelectionBarWidth(&self, out: *mut i32) -> windows_core::Result<()> {
        put(out, 0)
    }
}
impl ITextHost2_Impl for Host_Impl {
    fn TxIsDoubleClickPending(&self) -> BOOL {
        false.into()
    }
    fn TxGetWindow(&self, out: *mut HWND) -> windows_core::Result<()> {
        put(out, self.0.window.hwnd)
    }
    fn TxSetForegroundWindow(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn TxGetPalette(&self) -> HPALETTE {
        HPALETTE::default()
    }
    fn TxGetEastAsianFlags(&self, out: *mut i32) -> windows_core::Result<()> {
        put(out, 0)
    }
    fn TxSetCursor2(&self, cursor: HCURSOR, _: BOOL) -> HCURSOR {
        unsafe { SetCursor(Some(cursor)) }
    }
    fn TxFreeTextServicesNotification(&self) {}
    fn TxGetEditStyle(&self, _: u32, out: *mut u32) -> windows_core::Result<()> {
        put(out, 0)
    }
    fn TxGetWindowStyles(&self, style: *mut u32, extended: *mut u32) -> windows_core::Result<()> {
        put(style, 0)?;
        put(extended, 0)
    }
    fn TxShowDropCaret(&self, _: BOOL, _: HDC, _: *mut RECT) -> windows_core::Result<()> {
        Ok(())
    }
    fn TxDestroyCaret(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn TxGetHorzExtent(&self, out: *mut i32) -> windows_core::Result<()> {
        put(out, self.0.rect.get().right)
    }
}
