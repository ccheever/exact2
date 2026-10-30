//! Windowless RichEdit through Direct2D on the renderer's device.
//! @ref LLP 1149 D10; LLP 1115.004 D7; LLP 1149.001 Stage 1w.
use super::text_host::{Host, State, TextHost};
use super::*;
use windows::{
    core::{s, w, IUnknown, GUID, HRESULT},
    Win32::{
        Foundation::*,
        Graphics::Direct2D::{Common::*, *},
        System::LibraryLoader::*,
        UI::{Controls::RichEdit::*, WindowsAndMessaging::*},
    },
};

struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

pub struct RichEdit {
    services: ITextServices2,
    _host: TextHost,
    state: Rc<State>,
    context: ID2D1DeviceContext,
    bitmap: ID2D1Bitmap1,
    texture: TextureId,
    _library: Library,
    style: Cell<Option<(i32, [u8; 4], bool, bool)>>,
}
impl Drop for RichEdit {
    fn drop(&mut self) {
        unsafe {
            let _ = self.services.OnTxInPlaceDeactivate();
            self.context.SetTarget(None);
        }
    }
}
impl RichEdit {
    pub fn new(device: &mut D3d11, window: Window, size: [u32; 2]) -> Result<Self> {
        if size.contains(&0) || size.iter().any(|n| *n > 16384) {
            return Err(Error::Invalid("RichEdit size".into()));
        }
        // SAFETY: the DLL stays loaded until its services and callbacks drop.
        // The export's ABI is CreateTextServices from textserv.h. Its IIDs are
        // queried explicitly because windows-rs metadata supplies zero IIDs.
        unsafe {
            let library = Library(LoadLibraryExW(
                w!("msftedit.dll"),
                None,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )?);
            let address = GetProcAddress(library.0, s!("CreateTextServices"))
                .ok_or_else(|| Error::Device("msftedit!CreateTextServices is absent".into()))?;
            type Create = unsafe extern "system" fn(
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
                *mut *mut std::ffi::c_void,
            ) -> HRESULT;
            let create: Create = std::mem::transmute(address);
            let state = State::new(window, size);
            let host: TextHost = Host(state.clone()).into();
            let mut raw = std::ptr::null_mut();
            create(std::ptr::null_mut(), host.as_raw(), &mut raw).ok()?;
            let unknown = IUnknown::from_raw(raw);
            let mut services = std::ptr::null_mut();
            unknown
                .query(
                    &GUID::from_u128(0x8d33f741_cf58_11ce_a89d_00aa006cadc5),
                    &mut services,
                )
                .ok()?;
            let services = ITextServices2::from_raw(services);
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dxgi: IDXGIDevice = device.device.cast()?;
            let d2d = factory.CreateDevice(&dxgi)?;
            let context = d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let desc = TextureDesc::rgba(size[0], size[1]);
            let texture = setup::texture_format(&device.device, desc, DXGI_FORMAT_B8G8R8A8_UNORM)?;
            let surface: IDXGISurface = texture.raw.cast()?;
            let bitmap = context.CreateBitmapFromDxgiSurface(
                &surface,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.,
                    dpiY: 96.,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    ..Default::default()
                }),
            )?;
            let id = TextureId(device.next_texture);
            device.next_texture += 1;
            device.textures.insert(id, texture);
            let mut client = state.rect.get();
            services.OnTxInPlaceActivate(&mut client)?;
            let editor = Self {
                services,
                _host: host,
                state,
                context,
                bitmap,
                texture: id,
                _library: library,
                style: Cell::new(None),
            };
            editor.message(EM_SETEDITSTYLE, SES_USECTF as usize, SES_USECTF as isize)?;
            editor.message(WM_SETFOCUS, 0, 0)?;
            Ok(editor)
        }
    }
    pub fn message(&self, message: u32, wparam: usize, lparam: isize) -> Result<()> {
        self.dispatch(message, wparam, lparam).map(|_| ())
    }
    pub fn dispatch(&self, message: u32, wparam: usize, lparam: isize) -> Result<bool> {
        let mut answer = LRESULT(0);
        unsafe {
            let result = (self.services.vtable().base__.TxSendMessage)(
                self.services.as_raw(),
                message,
                WPARAM(wparam),
                LPARAM(lparam),
                &mut answer,
            );
            result.ok()?;
            Ok(result == S_OK)
        }
    }
    /// Configure in device pixels. The D2D target uses 96 DPI; font size and
    /// bounds carry the window scale together, including fractional scales.
    pub fn configure(
        &self,
        origin: [i32; 2],
        size: f32,
        color: [u8; 4],
        multiline: bool,
        readonly: bool,
    ) -> Result<()> {
        self.state.origin.set(POINT {
            x: origin[0],
            y: origin[1],
        });
        let style = ((size * 15.).round() as i32, color, multiline, readonly);
        if self.style.get() == Some(style) {
            return Ok(());
        }
        let bits = TXTBIT_RICHTEXT
            | TXTBIT_D2DDWRITE
            | TXTBIT_DISABLEDRAG
            | if multiline {
                TXTBIT_MULTILINE | TXTBIT_WORDWRAP
            } else {
                0
            }
            | if readonly { TXTBIT_READONLY } else { 0 };
        let changed = self.state.properties.replace(bits) ^ bits;
        unsafe {
            self.services.OnTxPropertyBitsChange(changed, bits)?;
        }
        let format = CHARFORMATW {
            cbSize: size_of::<CHARFORMATW>() as u32,
            dwMask: CFM_SIZE | CFM_COLOR,
            yHeight: (size * 15.).round() as i32,
            crTextColor: COLORREF(u32::from_le_bytes([color[0], color[1], color[2], 0])),
            ..Default::default()
        };
        self.message(
            EM_SETCHARFORMAT,
            SCF_ALL as usize,
            &format as *const _ as isize,
        )?;
        self.style.set(Some(style));
        self.state.dirty.set(true);
        Ok(())
    }
    pub fn set_text(&self, text: &str) -> Result<()> {
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        self.message(WM_SETTEXT, 0, text.as_ptr() as isize)
    }
    pub fn selection(&self) -> Result<[u32; 2]> {
        let mut range = CHARRANGE::default();
        self.message(EM_EXGETSEL, 0, &mut range as *mut _ as isize)?;
        Ok([range.cpMin.max(0) as u32, range.cpMax.max(0) as u32])
    }
    pub fn select(&self, selection: [u32; 2]) -> Result<()> {
        let range = CHARRANGE {
            cpMin: selection[0] as i32,
            cpMax: selection[1] as i32,
        };
        self.message(EM_EXSETSEL, 0, &range as *const _ as isize)
    }
    pub fn caret(&self) -> Option<[i32; 4]> {
        self.state
            .caret_visible
            .get()
            .then(|| self.state.caret.get())
    }
    pub fn dirty(&self) -> bool {
        self.state.dirty.get()
    }
    pub fn texture(&self) -> TextureId {
        self.texture
    }
    pub fn size(&self) -> [u32; 2] {
        let r = self.state.rect.get();
        [r.right as u32, r.bottom as u32]
    }
    pub fn resize(&mut self, device: &mut D3d11, size: [u32; 2]) -> Result<()> {
        if self.size() == size {
            return Ok(());
        }
        if size.contains(&0) || size.iter().any(|n| *n > 16384) {
            return Err(Error::Invalid("RichEdit size".into()));
        }
        unsafe {
            let texture = setup::texture_format(
                &device.device,
                TextureDesc::rgba(size[0], size[1]),
                DXGI_FORMAT_B8G8R8A8_UNORM,
            )?;
            let surface: IDXGISurface = texture.raw.cast()?;
            self.bitmap = self.context.CreateBitmapFromDxgiSurface(
                &surface,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.,
                    dpiY: 96.,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    ..Default::default()
                }),
            )?;
            let old = self.texture;
            self.texture = TextureId(device.next_texture);
            device.next_texture += 1;
            device.textures.insert(self.texture, texture);
            device.retire_texture(old)?;
            self.state.rect.set(RECT {
                right: size[0] as i32,
                bottom: size[1] as i32,
                ..Default::default()
            });
            self.services
                .OnTxPropertyBitsChange(TXTBIT_CLIENTRECTCHANGE, TXTBIT_CLIENTRECTCHANGE)?;
        }
        self.state.dirty.set(true);
        Ok(())
    }
    pub fn name(&self, name: &str) -> Result<()> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        self.message(EM_SETUIANAME, 0, name.as_ptr() as isize)
    }
    pub fn provider(
        &self,
        site: &windows::Win32::UI::Accessibility::IRawElementProviderWindowlessSite,
    ) -> Result<windows::Win32::UI::Accessibility::IRawElementProviderSimple> {
        unsafe {
            let mut raw = std::ptr::null_mut();
            // TextServ.h's IID; windows-rs metadata declares this interface as zero.
            self.services
                .query(
                    &GUID::from_u128(0x983e572d_20cd_460b_9104_83111592dd10),
                    &mut raw,
                )
                .ok()?;
            let access =
                windows::Win32::UI::Accessibility::IRicheditWindowlessAccessibility::from_raw(raw);
            Ok(access.CreateProvider(site)?)
        }
    }
    /// Deliver UTF-16 characters through the editor's ordinary input path.
    pub fn type_text(&self, text: &str) -> Result<()> {
        for unit in text.encode_utf16() {
            self.message(WM_CHAR, unit as usize, 1)?;
        }
        self.state.dirty.set(true);
        Ok(())
    }
    pub fn text(&self) -> Result<String> {
        let mut text = windows::core::BSTR::new();
        unsafe {
            self.services.TxGetText(&mut text)?;
        }
        let text = text.to_string();
        // RichEdit includes the document's implicit final paragraph marker.
        Ok(text
            .strip_suffix('\r')
            .unwrap_or(&text)
            .replace("\r\n", "\n")
            .replace('\r', "\n"))
    }
    /// Flush Direct2D on the same immediate device before Ink samples the image.
    pub fn draw(&self, device: &mut D3d11) -> Result<TextureId> {
        if !self.state.dirty.get() {
            return Ok(self.texture);
        }
        let rect = self.state.rect.get();
        let mut bounds = RECTL {
            left: 0,
            top: 0,
            right: rect.right,
            bottom: rect.bottom,
        };
        let mut update = rect;
        unsafe {
            self.context.SetTarget(&self.bitmap);
            self.context
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.,
            }));
            let rendered =
                self.services
                    .TxDrawD2D(&self.context, &mut bounds, &mut update, TXTVIEW_ACTIVE.0);
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None);
            rendered?;
            ended?;
        }
        let submission = device.submit()?;
        device.textures.get_mut(&self.texture).unwrap().last = submission;
        self.state.dirty.set(false);
        Ok(self.texture)
    }
}
