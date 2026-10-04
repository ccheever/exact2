//! An app-local Win32 presenter: Contract state and kernel frames become real HWNDs.
use exact_kernel::{Kernel, Offer, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{ControlValue, Event, Runner, Viewport};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const NEW: usize = 4001;
const OPEN: usize = 4002;
const SAVE: usize = 4003;
const SAVE_AS: usize = 4004;
const ABOUT: usize = 4005;
const EXIT: usize = 4006;
type Queued = (u32, usize, isize, Option<ControlValue>);
thread_local! {
    // A WndProc can reenter during SendMessage. It queues work instead of borrowing App.
    static QUEUE: RefCell<Vec<Queued>> = const { RefCell::new(Vec::new()) };
    static SYNCING: Cell<bool> = const { Cell::new(false) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND if !SYNCING.get() => {
            let text = if (w >> 16) & 0xffff == EN_CHANGE as usize && l != 0 {
                Some(ControlValue::Text(
                    unsafe { window_text(l as HWND) }.replace("\r\n", "\n"),
                ))
            } else if w & 0xffff == 109 && (w >> 16) & 0xffff == BN_CLICKED as usize {
                Some(ControlValue::Checked(
                    unsafe { SendMessageW(l as HWND, BM_GETCHECK, 0, 0) } != 0,
                ))
            } else {
                None
            };
            QUEUE.with_borrow_mut(|q| q.push((msg, w, l, text)));
        }
        WM_SIZE | WM_CLOSE => QUEUE.with_borrow_mut(|q| q.push((msg, w, l, None))),
        WM_DPICHANGED => {
            let r = unsafe { &*(l as *const RECT) };
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            QUEUE.with_borrow_mut(|q| q.push((WM_SIZE, 0, 0, None)));
        }
        WM_DESTROY => unsafe { PostQuitMessage(0) },
        WM_CTLCOLORSTATIC => {
            unsafe {
                SetBkMode(w as HDC, TRANSPARENT as i32);
                SetTextColor(w as HDC, GetSysColor(COLOR_WINDOWTEXT));
            }
            return unsafe { GetSysColorBrush(COLOR_WINDOW) } as isize;
        }
        WM_GETMINMAXINFO => {
            let info = unsafe { &mut *(l as *mut MINMAXINFO) };
            let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.;
            info.ptMinTrackSize = POINT {
                x: (760. * scale) as i32,
                y: (710. * scale) as i32,
            };
        }
        _ => return unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
    0
}

struct Control {
    hwnd: HWND,
    command: usize,
}

struct App {
    runner: Runner<()>,
    hwnd: HWND,
    controls: BTreeMap<&'static str, Control>,
    font: HFONT,
    heading_font: HFONT,
    editor_font: HFONT,
    dpi: f32,
    path: Option<PathBuf>,
}

impl App {
    fn boot(hwnd: HWND) -> Self {
        let runner = Runner::boot(
            Plan::decode(PLAN).unwrap(),
            (),
            Kernel::with_monospace(),
            Viewport::sized(980., 780.),
            "/",
        )
        .unwrap();
        Self {
            runner,
            hwnd,
            controls: BTreeMap::new(),
            font: null_mut(),
            heading_font: null_mut(),
            editor_font: null_mut(),
            dpi: 1.,
            path: None,
        }
    }

    fn node(&self, id: &str) -> u32 {
        let key = self
            .runner
            .kernel()
            .find_by_id(id)
            .into_iter()
            .next()
            .unwrap();
        self.runner.kernel().node_by_key(key).unwrap().id
    }

    fn text(&self, name: &str) -> String {
        self.runner
            .slot(name)
            .map_or(String::new(), |v| v.text().to_string())
    }

    fn dirty(&self) -> bool {
        self.runner.slot("dirty") == Some(&Value::Bool(true))
    }

    fn event(&mut self, id: &str, event: Event) {
        self.runner.dispatch(self.node(id), event).unwrap();
    }

    fn input(&mut self, id: &str, text: String) {
        self.event(id, Event::Input(ControlValue::Text(text)));
    }

    unsafe fn create(&mut self) {
        SYNCING.set(true);
        let specs = [
            ("heading", "STATIC", 0),
            ("subtitle", "STATIC", 0),
            ("samples-label", "STATIC", 0),
            (
                "samples",
                "LISTBOX",
                LBS_NOTIFY as u32 | WS_VSCROLL | WS_BORDER | WS_TABSTOP,
            ),
            ("new", "BUTTON", WS_TABSTOP),
            ("open", "BUTTON", WS_TABSTOP),
            ("save", "BUTTON", WS_TABSTOP),
            ("font-label", "STATIC", 0),
            (
                "font",
                "COMBOBOX",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP,
            ),
            ("pin", "BUTTON", BS_AUTOCHECKBOX as u32 | WS_TABSTOP),
            ("title-label", "STATIC", 0),
            ("title", "EDIT", ES_AUTOHSCROLL as u32 | WS_TABSTOP),
            (
                "body",
                "EDIT",
                (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32 | WS_VSCROLL | WS_TABSTOP,
            ),
            ("status", "msctls_statusbar32", SBARS_SIZEGRIP),
        ];
        for (i, (id, class, style)) in specs.into_iter().enumerate() {
            let command = 100 + i;
            let ex = if class == "EDIT" || class == "COMBOBOX" {
                WS_EX_CLIENTEDGE
            } else {
                0
            };
            let control = unsafe {
                CreateWindowExW(
                    ex,
                    wide(class).as_ptr(),
                    wide("").as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    0,
                    0,
                    1,
                    1,
                    self.hwnd,
                    command as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                )
            };
            assert!(!control.is_null(), "cannot create {class}");
            self.controls.insert(
                id,
                Control {
                    hwnd: control,
                    command,
                },
            );
        }
        let samples = self.controls["samples"].hwnd;
        for name in ["Welcome", "Today's checklist", "Scratchpad"] {
            unsafe {
                SendMessageW(samples, LB_ADDSTRING, 0, wide(name).as_ptr() as isize);
            }
        }
        unsafe {
            SendMessageW(samples, LB_SETCURSEL, 0, 0);
        }
        let combo = self.controls["font"].hwnd;
        for name in ["Segoe UI", "Consolas", "Georgia"] {
            unsafe {
                SendMessageW(combo, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
            }
        }
        unsafe {
            SendMessageW(combo, CB_SETCURSEL, 0, 0);
            SendMessageW(self.controls["body"].hwnd, EM_SETLIMITTEXT, 1_000_000, 0);
            SendMessageW(self.controls["title"].hwnd, EM_SETLIMITTEXT, 500, 0);
        }
        SYNCING.set(false);
        unsafe {
            self.sync();
            SetFocus(self.controls["body"].hwnd);
        }
    }

    unsafe fn fonts(&mut self) {
        let previous = [self.font, self.heading_font, self.editor_font];
        self.font = unsafe { font("Segoe UI", 16. * self.dpi, 400) };
        self.heading_font = unsafe { font("Segoe UI", 26. * self.dpi, 600) };
        let index = unsafe { SendMessageW(self.controls["font"].hwnd, CB_GETCURSEL, 0, 0) };
        let family = match index {
            1 => "Consolas",
            2 => "Georgia",
            _ => "Segoe UI",
        };
        self.editor_font = unsafe { font(family, 17. * self.dpi, 400) };
        for (&id, c) in &self.controls {
            let f = match id {
                "heading" => self.heading_font,
                "body" => self.editor_font,
                _ => self.font,
            };
            unsafe {
                SendMessageW(c.hwnd, WM_SETFONT, f as usize, 1);
            }
        }
        for f in previous {
            if !f.is_null() {
                unsafe {
                    DeleteObject(f);
                }
            }
        }
    }

    unsafe fn sync(&mut self) {
        SYNCING.set(true);
        let scale = unsafe { GetDpiForWindow(self.hwnd) } as f32 / 96.;
        if self.font.is_null() || scale != self.dpi {
            self.dpi = scale;
            unsafe {
                self.fonts();
            }
        }
        let mut r = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut r);
        }
        let root = self.runner.roots()[0];
        self.runner
            .kernel_mut()
            .compute_layout(
                root,
                Offer::definite(r.right as f32 / self.dpi, r.bottom as f32 / self.dpi),
            )
            .unwrap();
        for (&id, c) in &self.controls {
            let n = self.runner.kernel().node(self.node(id)).unwrap();
            let f = n.frame;
            let h = if id == "font" { 170. } else { f.height };
            unsafe {
                MoveWindow(
                    c.hwnd,
                    (f.x * self.dpi) as i32,
                    (f.y * self.dpi) as i32,
                    (f.width * self.dpi) as i32,
                    (h * self.dpi) as i32,
                    1,
                );
            }
            if matches!(id, "samples" | "font" | "status") {
                continue;
            }
            let label = if id == "pin" {
                "Keep window on top".to_string()
            } else if id == "title" || id == "body" {
                n.props
                    .get(PropId::Value)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                let own = n.props.get(PropId::Text).and_then(|v| v.as_str());
                own.map(str::to_string).unwrap_or_else(|| {
                    n.children()
                        .into_iter()
                        .filter_map(|child| self.runner.kernel().node(child))
                        .filter_map(|child| child.props.get(PropId::Text).and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
            };
            let label = label.replace('\n', "\r\n");
            if unsafe { window_text(c.hwnd) } != label {
                unsafe {
                    SetWindowTextW(c.hwnd, wide(&label).as_ptr());
                }
            }
        }
        let pinned = self.runner.slot("pinned") == Some(&Value::Bool(true));
        unsafe {
            SendMessageW(
                self.controls["pin"].hwnd,
                BM_SETCHECK,
                usize::from(pinned),
                0,
            );
            SetWindowPos(
                self.hwnd,
                if pinned { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let body = self.text("body");
        let status = format!(
            "  {}  |  {} words  |  {} characters  |  {}",
            if self.dirty() {
                "Unsaved changes"
            } else {
                "Ready"
            },
            body.split_whitespace().count(),
            body.chars().count(),
            self.path
                .as_ref()
                .map_or("No file yet".to_string(), |p| p.display().to_string())
        );
        unsafe {
            SendMessageW(
                self.controls["status"].hwnd,
                SB_SETTEXTW,
                0,
                wide(&status).as_ptr() as isize,
            );
            SetWindowTextW(
                self.hwnd,
                wide(&format!(
                    "{}{} — Windows Desk · Exact2",
                    if self.dirty() { "* " } else { "" },
                    self.text("title")
                ))
                .as_ptr(),
            );
        }
        SYNCING.set(false);
    }

    unsafe fn confirm_replace(&mut self) -> bool {
        if !self.dirty() {
            return true;
        }
        match unsafe {
            MessageBoxW(
                self.hwnd,
                wide("Save your changes before continuing?").as_ptr(),
                wide("Windows Desk").as_ptr(),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            )
        } {
            IDYES => unsafe { self.save(false) },
            IDNO => true,
            _ => false,
        }
    }

    unsafe fn dialog(&self, save: bool) -> Option<PathBuf> {
        let mut filename = [0u16; 32768];
        if save {
            let suggestion = self
                .path
                .as_ref()
                .map_or("Document.txt".to_string(), |p| p.display().to_string());
            for (to, from) in filename.iter_mut().zip(suggestion.encode_utf16()) {
                *to = from;
            }
        }
        let filter = wide("Text documents (*.txt;*.md)\0*.txt;*.md\0All files\0*.*\0");
        let extension = wide("txt");
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.hwnd,
            lpstrFile: filename.as_mut_ptr(),
            nMaxFile: filename.len() as u32,
            lpstrFilter: filter.as_ptr(),
            nFilterIndex: 1,
            lpstrDefExt: extension.as_ptr(),
            Flags: OFN_EXPLORER
                | OFN_NOCHANGEDIR
                | OFN_PATHMUSTEXIST
                | if save {
                    OFN_OVERWRITEPROMPT
                } else {
                    OFN_FILEMUSTEXIST
                },
            ..Default::default()
        };
        let ok = unsafe {
            if save {
                GetSaveFileNameW(&mut ofn)
            } else {
                GetOpenFileNameW(&mut ofn)
            }
        };
        if ok == 0 {
            return None;
        }
        let end = filename.iter().position(|v| *v == 0).unwrap();
        Some(PathBuf::from(String::from_utf16_lossy(&filename[..end])))
    }

    unsafe fn save(&mut self, choose: bool) -> bool {
        let path = if choose || self.path.is_none() {
            unsafe { self.dialog(true) }
        } else {
            self.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        match std::fs::write(&path, self.text("body").replace('\n', "\r\n")) {
            Ok(()) => {
                self.path = Some(path);
                self.event("save", Event::Press);
                true
            }
            Err(e) => {
                unsafe {
                    self.error(&e.to_string());
                }
                false
            }
        }
    }

    unsafe fn error(&self, error: &str) {
        unsafe {
            MessageBoxW(
                self.hwnd,
                wide(error).as_ptr(),
                wide("Windows Desk").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    unsafe fn command(&mut self, w: usize, l: isize, snapshot: Option<ControlValue>) {
        let command = w & 0xffff;
        let notification = (w >> 16) & 0xffff;
        let id = self
            .controls
            .iter()
            .find(|(_, c)| c.command == command)
            .map(|(&id, _)| id);
        match id {
            Some("title" | "body") if notification == EN_CHANGE as usize => {
                let id = id.unwrap();
                let value = match snapshot {
                    Some(ControlValue::Text(text)) => text,
                    _ => unsafe { window_text(l as HWND) }.replace("\r\n", "\n"),
                };
                self.input(id, value);
            }
            Some("samples") if notification == LBN_SELCHANGE as usize => {
                let selected =
                    unsafe { SendMessageW(self.controls["samples"].hwnd, LB_GETCURSEL, 0, 0) };
                if unsafe { self.confirm_replace() } {
                    self.event(
                        match selected {
                            1 => "checklist",
                            2 => "scratch",
                            _ => "welcome",
                        },
                        Event::Press,
                    );
                    self.path = None;
                }
            }
            Some("font") if notification == CBN_SELCHANGE as usize => unsafe {
                self.fonts();
            },
            Some("pin") if notification == BN_CLICKED as usize => {
                let checked = match snapshot {
                    Some(ControlValue::Checked(checked)) => checked,
                    _ => {
                        (unsafe { SendMessageW(self.controls["pin"].hwnd, BM_GETCHECK, 0, 0) }) != 0
                    }
                };
                self.event("pin", Event::Change(ControlValue::Checked(checked)));
            }
            Some("new" | "open" | "save") if notification == BN_CLICKED as usize => {
                let menu = match id.unwrap() {
                    "new" => NEW,
                    "open" => OPEN,
                    _ => SAVE_AS,
                };
                unsafe {
                    self.command(menu, 0, None);
                }
            }
            None => match command {
                NEW => {
                    if unsafe { self.confirm_replace() } {
                        self.event("new", Event::Press);
                        self.path = None;
                    }
                }
                OPEN => {
                    if unsafe { self.confirm_replace() } {
                        if let Some(path) = unsafe { self.dialog(false) } {
                            match std::fs::read_to_string(&path) {
                                Ok(body) if body.len() <= 1_000_000 => {
                                    let title = path
                                        .file_stem()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned();
                                    self.input("title", title);
                                    self.input("body", body.replace("\r\n", "\n"));
                                    self.event("save", Event::Press);
                                    self.path = Some(path);
                                }
                                Ok(_) => unsafe {
                                    self.error(
                                        "Please choose a UTF-8 text file smaller than 1 MB.",
                                    );
                                },
                                Err(e) => unsafe {
                                    self.error(&format!(
                                        "Could not open this UTF-8 text file: {e}"
                                    ));
                                },
                            }
                        }
                    }
                }
                SAVE | SAVE_AS => unsafe {
                    self.save(command == SAVE_AS);
                },
                ABOUT => unsafe {
                    MessageBoxW(self.hwnd,
                    wide("Windows Desk\n\nBuilt with Exact2.\n\nContract: document state, actions, and layout.\nWin32: menus, list, edit controls, font picker, checkbox, dialogs, and status bar.\n\nNo browser or WebView.").as_ptr(),
                    wide("About Windows Desk").as_ptr(), MB_OK | MB_ICONINFORMATION);
                },
                EXIT if unsafe { self.confirm_replace() } => unsafe {
                    DestroyWindow(self.hwnd);
                },
                _ => {}
            },
            _ => {}
        }
        if unsafe { IsWindow(self.hwnd) } != 0 {
            unsafe {
                self.sync();
            }
        }
    }

    unsafe fn smoke(&mut self) {
        // Exercise actual HWND notifications through the same queue as a user.
        unsafe {
            SetActiveWindow(self.hwnd);
            SendMessageW(self.controls["body"].hwnd, EM_SETSEL, 0, -1);
            SendMessageW(
                self.controls["body"].hwnd,
                EM_REPLACESEL,
                1,
                wide("Native editor round trip: café 🪟\r\nsecond line").as_ptr() as isize,
            );
        }
        unsafe {
            self.drain();
        }
        assert_eq!(
            self.text("body"),
            "Native editor round trip: café 🪟\nsecond line"
        );
        assert!(self.dirty());
        unsafe {
            SendMessageW(self.controls["pin"].hwnd, BM_CLICK, 0, 0);
            self.drain();
        }
        assert_eq!(self.runner.slot("pinned"), Some(&Value::Bool(true)));
        unsafe {
            SendMessageW(self.controls["pin"].hwnd, BM_CLICK, 0, 0);
            self.drain();
        }
        let path =
            std::env::temp_dir().join(format!("exact2-windows-desk-{}.txt", std::process::id()));
        self.path = Some(path.clone());
        assert!(unsafe { self.save(false) });
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Native editor round trip: café 🪟\r\nsecond line"
        );
        std::fs::remove_file(path).unwrap();
        assert!(!self.dirty());
        self.event("checklist", Event::Press);
        self.path = None;
        unsafe {
            self.sync();
        }
        assert_eq!(
            unsafe { window_text(self.controls["title"].hwnd) },
            "Today's checklist"
        );
        unsafe {
            SendMessageW(self.controls["font"].hwnd, CB_SETCURSEL, 1, 0);
            self.fonts();
        }
        unsafe {
            SetWindowPos(
                self.hwnd,
                null_mut(),
                0,
                0,
                1100,
                820,
                SWP_NOMOVE | SWP_NOZORDER,
            );
            self.drain();
        }
        self.event("welcome", Event::Press);
        unsafe {
            SendMessageW(self.controls["font"].hwnd, CB_SETCURSEL, 0, 0);
            self.fonts();
            self.sync();
        }
        let proof = std::env::temp_dir().join("exact2-windows-desk-smoke.txt");
        std::fs::write(proof, "PASS: native EDIT -> Contract state -> HWND; Unicode; checkbox/topmost; save bytes; sample selection; font; resize\n").unwrap();
    }

    unsafe fn drain(&mut self) {
        loop {
            let pending = QUEUE.with_borrow_mut(std::mem::take);
            if pending.is_empty() {
                break;
            }
            for (msg, w, l, snapshot) in pending {
                match msg {
                    WM_COMMAND => unsafe {
                        self.command(w, l, snapshot);
                    },
                    WM_SIZE if w != SIZE_MINIMIZED as usize => unsafe {
                        self.sync();
                    },
                    WM_CLOSE if unsafe { self.confirm_replace() } => unsafe {
                        DestroyWindow(self.hwnd);
                    },
                    _ => {}
                }
            }
        }
    }
}

unsafe fn window_text(hwnd: HWND) -> String {
    let mut buf = vec![0u16; unsafe { GetWindowTextLengthW(hwnd) } as usize + 1];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..len as usize])
}

unsafe fn font(name: &str, size: f32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -(size.round() as i32),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide(name).as_ptr(),
        )
    }
}

pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let common = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&common);
        let instance = GetModuleHandleW(null());
        let class = wide("Exact2WindowsDesk");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&wc), 0);
        let menu = CreateMenu();
        let file = CreatePopupMenu();
        for (id, label) in [
            (NEW, "&New\tCtrl+N"),
            (OPEN, "&Open...\tCtrl+O"),
            (SAVE, "&Save\tCtrl+S"),
            (SAVE_AS, "Save &As...\tCtrl+Shift+S"),
            (EXIT, "E&xit"),
        ] {
            AppendMenuW(file, MF_STRING, id, wide(label).as_ptr());
        }
        AppendMenuW(menu, MF_POPUP, file as usize, wide("&File").as_ptr());
        let help = CreatePopupMenu();
        AppendMenuW(help, MF_STRING, ABOUT, wide("&About Windows Desk").as_ptr());
        AppendMenuW(menu, MF_POPUP, help as usize, wide("&Help").as_ptr());
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Windows Desk · Exact2").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1040,
            860,
            null_mut(),
            menu,
            instance,
            null(),
        );
        assert!(!hwnd.is_null());
        let mut app = App::boot(hwnd);
        app.create();
        let keys = [
            ACCEL {
                fVirt: FVIRTKEY | FCONTROL,
                key: b'N' as u16,
                cmd: NEW as u16,
            },
            ACCEL {
                fVirt: FVIRTKEY | FCONTROL,
                key: b'O' as u16,
                cmd: OPEN as u16,
            },
            ACCEL {
                fVirt: FVIRTKEY | FCONTROL,
                key: b'S' as u16,
                cmd: SAVE as u16,
            },
            ACCEL {
                fVirt: FVIRTKEY | FCONTROL | FSHIFT,
                key: b'S' as u16,
                cmd: SAVE_AS as u16,
            },
        ];
        let accelerator = CreateAcceleratorTableW(keys.as_ptr(), keys.len() as i32);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        app.drain();
        if std::env::args().any(|a| a == "--smoke") {
            app.smoke();
        }
        let args: Vec<_> = std::env::args_os().collect();
        if let Some(index) = args.iter().position(|a| a == "--capture") {
            capture(hwnd, &PathBuf::from(&args[index + 1]));
        }
        if std::env::args().any(|a| a == "--smoke-exit") {
            app.smoke();
            DestroyWindow(hwnd);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if TranslateAcceleratorW(hwnd, accelerator, &msg) == 0
                && IsDialogMessageW(hwnd, &msg) == 0
            {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            app.drain();
        }
        DestroyAcceleratorTable(accelerator);
        for f in [app.font, app.heading_font, app.editor_font] {
            DeleteObject(f);
        }
    }
}

unsafe fn capture(hwnd: HWND, path: &std::path::Path) {
    let mut r = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut r);
    }
    let width = r.right - r.left;
    let height = r.bottom - r.top;
    let screen = unsafe { GetWindowDC(hwnd) };
    let dc = unsafe { CreateCompatibleDC(screen) };
    let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
    let old = unsafe { SelectObject(dc, bitmap) };
    assert_ne!(unsafe { PrintWindow(hwnd, dc, 2) }, 0);
    unsafe {
        SelectObject(dc, old);
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: 40,
            biWidth: width,
            biHeight: height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    assert_ne!(
        unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height as u32,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        },
        0
    );
    let mut bytes = Vec::with_capacity(54 + pixels.len());
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 24]);
    bytes.extend_from_slice(&pixels);
    std::fs::write(path, bytes).unwrap();
    unsafe {
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(hwnd, screen);
    }
}
