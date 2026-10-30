//! Product-window qualification, run in the signed-in desktop after building Shelf.
//! @ref LLP 1149 S2.1, D7, D9; LLP 1115.004 D13.
use std::{
    fs::{self, File},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::{Dwm::DwmFlush, Gdi::*},
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};

fn out() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ink-s2");
    fs::create_dir_all(&path).unwrap();
    path
}

fn until(label: &str, mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < end, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn unlocked() -> bool {
    use windows::Win32::{Foundation::HANDLE, System::StationsAndDesktops::*};
    unsafe {
        let Ok(desktop) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return false;
        };
        let mut name = [0u16; 64];
        let result = GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            size_of_val(&name) as u32,
            None,
        );
        CloseDesktop(desktop).unwrap();
        result.is_ok()
            && String::from_utf16_lossy(
                &name[..name.iter().position(|n| *n == 0).unwrap_or(name.len())],
            ) == "Default"
    }
}

struct Shelf {
    child: Child,
    hwnd: HWND,
    log: PathBuf,
}

impl Shelf {
    fn launch(
        name: &str,
        exe: &str,
        painter: &str,
        size: &str,
        frames: Option<u32>,
        field: bool,
    ) -> Self {
        Self::launch_fault(name, exe, painter, size, frames, field, None)
    }

    fn launch_fault(
        name: &str,
        exe: &str,
        painter: &str,
        size: &str,
        frames: Option<u32>,
        field: bool,
        fault: Option<&str>,
    ) -> Self {
        let log = out().join(format!("{name}.log"));
        let file = File::create(&log).unwrap();
        let mut command = Command::new(exe);
        command
            .env(
                "EXACT_INK_DEVICE",
                match painter {
                    "wgpu" => "wgpu",
                    "warp" => "warp",
                    _ => "d3d11",
                },
            )
            .env(
                "EXACT_PAINTER",
                if matches!(painter, "wgpu" | "warp") {
                    "ink"
                } else {
                    painter
                },
            )
            .env("EXACT_SIZE", size)
            .env("EXACT_AGENT_WINDOW_READY", "library-count")
            .env(
                "EXACT_AGENT_STORAGE",
                format!("ink-s2-{name}-{}", std::process::id()),
            )
            .env("EXACT_AGENT_STORAGE_FRESH", "1")
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_DISPLAY")
            .env_remove("EXACT_INK_RICHEDIT")
            .env_remove("EXACT_AGENT_INK_FAULT")
            .env_remove("EXACT_AGENT_WINDOW_TAP")
            .env_remove("EXACT_AGENT_WINDOW_FRAMES")
            .stdout(Stdio::from(file.try_clone().unwrap()))
            .stderr(Stdio::from(file));
        if let Some(fault) = fault {
            command.env("EXACT_AGENT_INK_FAULT", fault);
        }
        if let Some(frames) = frames {
            command.env("EXACT_AGENT_WINDOW_FRAMES", frames.to_string());
        }
        if field {
            command.env("EXACT_AGENT_WINDOW_TAP", "search");
        }
        let launched = Instant::now();
        let child = command.spawn().unwrap();
        println!("S2_PROCESS {name} pid={} exe={exe}", child.id());
        let mut app = Self {
            child,
            hwnd: HWND::default(),
            log,
        };
        if fault == Some("slow-devices") {
            until("late-device window is visible", || {
                app.alive();
                app.hwnd = window_for(app.child.id());
                !app.hwnd.is_invalid() && app.client().right > 0 && app.client().bottom > 0
            });
            let image = app.capture_pixels();
            image
                .save_png(out().join("slow-devices-before.png"))
                .unwrap();
            println!(
                "S2_LATE_DEVICES capture_after_spawn_ms={:.3} first_present={}",
                launched.elapsed().as_secs_f64() * 1000.,
                app.text().contains("first present returned")
            );
        }
        until("Shelf's complete first screen", || {
            app.alive();
            let text = app.text();
            text.contains("process to first-screen compositor sync")
                || text.contains("complete first screen submitted (probe off)")
        });
        until("Shelf HWND", || {
            app.hwnd = window_for(app.child.id());
            !app.hwnd.is_invalid()
        });
        app
    }

    fn open(name: &str, field: bool) -> Self {
        // Coordinates below are physical pixels, including under a changed DPI.
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let exe = std::env::var("INK_SHELF").expect("INK_SHELF names the built Shelf.exe");
        let painter = if std::env::var("EXACT_INK_DEVICE").as_deref() == Ok("wgpu") {
            "wgpu"
        } else {
            "ink"
        };
        let mut app = Self::launch(name, &exe, painter, "1024x720", None, field);
        app.place(1024, 720);
        app
    }

    fn alive(&mut self) {
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "Shelf exited: {}",
            self.text()
        );
    }

    fn text(&self) -> String {
        fs::read_to_string(&self.log).unwrap()
    }

    fn client(&self) -> RECT {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut rect).unwrap();
        }
        rect
    }

    fn place(&mut self, width: i32, height: i32) {
        let mut outer = RECT {
            right: width,
            bottom: height,
            ..Default::default()
        };
        // The old outer-minus-client extent can retain the previous DPI's
        // nonclient metrics during WM_DPICHANGED. Use the HWND's current DPI.
        // https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-adjustwindowrectexfordpi
        unsafe {
            AdjustWindowRectExForDpi(
                &mut outer,
                WINDOW_STYLE(GetWindowLongPtrW(self.hwnd, GWL_STYLE) as u32),
                !GetMenu(self.hwnd).is_invalid(),
                WINDOW_EX_STYLE(GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32),
                GetDpiForWindow(self.hwnd),
            )
            .unwrap();
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                80,
                80,
                outer.right - outer.left,
                outer.bottom - outer.top,
                SWP_SHOWWINDOW,
            )
            .unwrap();
            let _ = SetForegroundWindow(self.hwnd);
        }
        until("window resize and paint", || {
            self.alive();
            let rect = self.client();
            rect.right == width
                && rect.bottom == height
                && self
                    .text()
                    .lines()
                    .rev()
                    .find(|s| s.starts_with("exact-windows: frame "))
                    .is_some_and(|s| s.contains(&format!("{width}x{height}, scale")))
        });
    }

    fn drag_size(&mut self, width: i32, height: i32) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        fn button(flags: MOUSE_EVENT_FLAGS) {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            };
            assert_eq!(unsafe { SendInput(&[input], size_of::<INPUT>() as i32) }, 1);
        }
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                button(MOUSEEVENTF_LEFTUP);
            }
        }
        for horizontal in [true, false] {
            let mut outer = RECT::default();
            unsafe {
                GetWindowRect(self.hwnd, &mut outer).unwrap();
            }
            let client = self.client();
            let (x, y, delta) = if horizontal {
                (
                    outer.right - 2,
                    (outer.top + outer.bottom) / 2,
                    width - client.right,
                )
            } else {
                (
                    (outer.left + outer.right) / 2,
                    outer.bottom - 2,
                    height - client.bottom,
                )
            };
            unsafe {
                SetCursorPos(x, y).unwrap();
            }
            button(MOUSEEVENTF_LEFTDOWN);
            let release = Release;
            for step in 1..=8 {
                unsafe {
                    SetCursorPos(
                        x + if horizontal { delta * step / 8 } else { 0 },
                        y + if horizontal { 0 } else { delta * step / 8 },
                    )
                    .unwrap();
                }
                let target = if horizontal {
                    client.right
                } else {
                    client.bottom
                } + delta * step / 8;
                until("drag reaches the requested size", || {
                    self.alive();
                    let rect = self.client();
                    if horizontal {
                        rect.right == target
                    } else {
                        rect.bottom == target
                    }
                });
            }
            drop(release);
        }
        until("dragged extent painted", || {
            self.alive();
            self.text()
                .lines()
                .rev()
                .find(|s| s.starts_with("exact-windows: frame "))
                .is_some_and(|s| s.contains(&format!("{width}x{height}, scale")))
        });
    }

    fn capture(&mut self, name: &str) -> tiny_skia::Pixmap {
        // DwmFlush synchronizes presents, but does not finish opening or restore
        // animations. Wait for three stable desktop captures before comparing.
        let mut previous = self.capture_pixels();
        let mut stable = 0;
        until("desktop animation settles", || {
            let next = self.capture_pixels();
            let score = if (previous.width(), previous.height()) == (next.width(), next.height()) {
                band(&previous, &next)
            } else {
                (f64::INFINITY, 1.)
            };
            stable = if score.0 < 0.02 && score.1 < 0.0001 {
                stable + 1
            } else {
                0
            };
            previous = next;
            stable >= 3
        });
        assert!(
            previous.data().chunks_exact(4).any(|p| p[0] < 32)
                && previous.data().chunks_exact(4).any(|p| p[0] > 224),
            "Shelf capture must contain its text and background"
        );
        previous
            .save_png(out().join(format!("{name}.png")))
            .unwrap();
        previous
    }

    fn capture_pixels(&mut self) -> tiny_skia::Pixmap {
        self.alive();
        let mut clicked = false;
        until("Shelf is unobscured", || {
            self.alive();
            unsafe {
                let _ = SetForegroundWindow(self.hwnd);
                if GetForegroundWindow() != self.hwnd
                    && !GetForegroundWindow().is_invalid()
                    && !clicked
                {
                    // Settings can own the last input after a DPI change.
                    // Shelf's empty lower-right client margin cannot turn
                    // repeated activation into a title-bar double-click.
                    use windows::Win32::UI::Input::KeyboardAndMouse::*;
                    SetWindowPos(
                        self.hwnd,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
                    )
                    .unwrap();
                    let rect = self.client();
                    let mut point = POINT {
                        x: rect.right - 24,
                        y: rect.bottom - 24,
                    };
                    assert!(ClientToScreen(self.hwnd, &mut point).as_bool());
                    SetCursorPos(point.x, point.y).unwrap();
                    let events = [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP].map(|flags| INPUT {
                        r#type: INPUT_MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dwFlags: flags,
                                ..Default::default()
                            },
                        },
                    });
                    assert_eq!(SendInput(&events, size_of::<INPUT>() as i32), 2);
                    clicked = true;
                }
                GetForegroundWindow() == self.hwnd
            }
        });
        until("desktop compositor is available", || {
            self.alive();
            unsafe { DwmFlush().is_ok() }
        });
        let rect = self.client();
        let (w, h) = (rect.right, rect.bottom);
        assert!(w > 0 && h > 0);
        let mut origin = POINT::default();
        // Capture the compositor's desktop pixels, not a new renderer readback.
        unsafe {
            assert!(ClientToScreen(self.hwnd, &mut origin).as_bool());
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut data = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut data, None, 0).unwrap();
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            let result = BitBlt(
                dc,
                0,
                0,
                w,
                h,
                Some(screen),
                origin.x,
                origin.y,
                SRCCOPY | CAPTUREBLT,
            );
            let mut bytes =
                std::slice::from_raw_parts(data.cast::<u8>(), (w * h * 4) as usize).to_vec();
            SelectObject(dc, previous);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            result.unwrap();
            for p in bytes.chunks_exact_mut(4) {
                p.swap(0, 2);
                p[3] = 255;
            }
            tiny_skia::Pixmap::from_vec(
                bytes,
                tiny_skia::IntSize::from_wh(w as u32, h as u32).unwrap(),
            )
            .unwrap()
        }
    }

    fn close(mut self) -> String {
        unsafe {
            PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        }
        until("Shelf closes", || self.child.try_wait().unwrap().is_some());
        assert!(self.child.wait().unwrap().success(), "{}", self.text());
        let log = self.text();
        assert!(
            log.contains("frames; mean paint"),
            "missing frame log: {log}"
        );
        log
    }
}

impl Drop for Shelf {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn window_for(pid: u32) -> HWND {
    struct Search {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let search = unsafe { &mut *(data.0 as *mut Search) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == search.pid && unsafe { IsWindowVisible(hwnd).as_bool() } {
            search.hwnd = hwnd;
            return false.into();
        }
        true.into()
    }
    let mut search = Search {
        pid,
        hwnd: HWND::default(),
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&mut search as *mut Search) as isize));
    }
    search.hwnd
}

fn band(a: &tiny_skia::Pixmap, b: &tiny_skia::Pixmap) -> (f64, f64) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut sum = 0u64;
    let mut over = 0usize;
    for (a, b) in a.data().chunks_exact(4).zip(b.data().chunks_exact(4)) {
        let delta = (0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap();
        sum += u64::from(delta);
        over += usize::from(delta > 32);
    }
    let n = (a.width() * a.height()) as f64;
    (sum as f64 / n, over as f64 / n)
}

fn fresh_comparison(name: &str, mut app: Shelf) {
    app.place(1024, 720);
    // Wait on agreement with the independently launched frame, not a fixed delay.
    let after = app.capture(&format!("{name}-after"));
    let log = app.close();
    assert!(
        !log.contains("device lost") && !log.contains("resize:"),
        "{log}"
    );
    let mut fresh = Shelf::open(&format!("{name}-fresh"), false);
    let mut score = (f64::INFINITY, 1.);
    until("journey agrees with fresh launch", || {
        let image = fresh.capture(&format!("{name}-fresh"));
        score = band(&after, &image);
        score.0 < 5. && score.1 < 0.06
    });
    println!("S2_BAND {name} mean={} over32={}", score.0, score.1);
    fresh.close();
}

fn fallback_launch(fault: &str) -> Shelf {
    let exe = std::env::var("INK_SHELF").unwrap();
    Shelf::launch_fault(fault, &exe, "ink", "1024x720", None, false, Some(fault))
}

fn assert_picture(image: &tiny_skia::Pixmap) {
    let dark = image
        .data()
        .chunks_exact(4)
        .filter(|p| p[..3].iter().all(|n| *n < 80))
        .count();
    let light = image
        .data()
        .chunks_exact(4)
        .filter(|p| p[..3].iter().all(|n| *n > 200))
        .count();
    assert!(
        dark > 500 && light > 500,
        "blank client: dark={dark}, light={light}"
    );
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_exhausted_devices_exit_with_both_refusals() {
    let exe = std::env::var("INK_SHELF").unwrap();
    for fault in ["no-devices", "no-dxbc"] {
        let log = out().join(format!("terminal-{fault}.log"));
        let file = File::create(&log).unwrap();
        let mut child = Command::new(&exe)
            .env("EXACT_PAINTER", "ink")
            .env("EXACT_INK_DEVICE", "d3d11")
            .env("EXACT_AGENT_WINDOW_READY", "library-count")
            .env("EXACT_AGENT_INK_FAULT", fault)
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_DISPLAY")
            .stdout(Stdio::from(file.try_clone().unwrap()))
            .stderr(Stdio::from(file))
            .spawn()
            .unwrap();
        println!("S2_PROCESS terminal-{fault} pid={} exe={exe}", child.id());
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!(
                    "exhausted devices left a blank window: {}",
                    fs::read_to_string(&log).unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let text = fs::read_to_string(log).unwrap();
        assert!(!status.success(), "{text}");
        assert!(
            text.contains("no composition device: hardware:") && text.contains("; WARP:"),
            "{text}"
        );
        assert!(!text.contains("first-screen"), "{text}");
    }
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_both_late_devices_measure_the_first_pixel_trade() {
    let mut app = fallback_launch("slow-devices");
    let text = app.text();
    for line in text.lines().filter(|line| {
        line.contains("first present") || line.contains("first-screen") || line.contains("prepared")
    }) {
        println!("S2_LATE_DEVICES {line}");
    }
    assert_picture(&app.capture("slow-devices"));
    app.close();
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_hardware_recovers_while_startup_warp_is_pending() {
    let mut app = fallback_launch("slow-warp-recovery");
    until("hardware recovered with startup WARP pending", || {
        app.alive();
        let text = app.text();
        let loss = text.find("injected hardware loss with WARP pending");
        loss.is_some_and(|at| {
            text[at..]
                .lines()
                .rev()
                .find(|line| line.starts_with("exact-windows: frame "))
                .is_some_and(|line| line.contains("painter ink"))
        })
    });
    let text = app.text();
    assert!(text.contains("injected pending startup WARP"), "{text}");
    assert!(
        !text.contains("injected startup WARP finished"),
        "hardware waited for WARP: {text}"
    );
    assert_picture(&app.capture("pending-warp-hardware-recovery"));
    app.close();
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_device_preparation_overlaps_slow_configuration() {
    let app = fallback_launch("slow-config");
    let text = app.close();
    let ready = text.find("device worker at").unwrap();
    let config = text.find("boot config-ready").unwrap();
    assert!(
        ready < config,
        "device preparation waited for config: {text}"
    );
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_quit_does_not_join_a_pending_shell_call() {
    let app = fallback_launch("slow-jump-list");
    until("jump list worker started", || {
        app.text().contains("jump-list-started")
    });
    assert!(!app.text().contains("jump-list-ready"));
    let began = Instant::now();
    app.close();
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "exit joined the pending Shell call"
    );
}

fn warp_boot(fault: &str) {
    let mut app = fallback_launch(fault);
    until("WARP paints the first screen", || {
        app.alive();
        let log = app.text();
        log.contains("painter ink") && log.contains("warp true")
    });
    let image = app.capture(fault);
    assert_picture(&image);
    let log = app.close();
    assert!(log.contains("injected"), "{log}");
    assert!(!log.contains("hardware recovery"), "{log}");
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_hardware_open_refusal_uses_warp() {
    warp_boot("hardware-open");
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_hardware_attach_refusal_uses_warp() {
    warp_boot("hardware-attach");
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_slow_preparation_displays_cpu_before_hardware() {
    let mut app = fallback_launch("slow-hardware");
    let first = app.text();
    assert!(first.contains("painter cpu"), "{first}");
    assert!(
        first.contains("warp true") && !first.contains("warp false"),
        "WARP must present before hardware is ready: {first}"
    );
    assert_picture(&app.capture("slow-hardware-cpu"));
    until("prepared hardware takes over", || {
        app.alive();
        app.text().contains("painter ink")
    });
    assert_picture(&app.capture("slow-hardware-ink"));
    app.close();
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_refused_warp_waits_for_prepared_hardware() {
    let mut app = fallback_launch("slow-hardware-no-warp");
    let first = app.text();
    assert!(
        first.contains("WARP: device: injected device refusal"),
        "{first}"
    );
    assert!(
        !first.contains("painter cpu"),
        "no CPU frame was presented: {first}"
    );
    assert!(first.contains("painter ink"), "{first}");
    assert_picture(&app.capture("refused-warp-hardware"));
    app.close();
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_unavailable_mica_still_presents() {
    let mut app = fallback_launch("mica");
    assert_picture(&app.capture("mica-unavailable"));
    let log = app.close();
    assert!(log.contains("painter ink"), "{log}");
    assert_eq!(log.matches("Mica unavailable").count(), 1, "{log}");
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_startup_series() {
    let after = std::env::var("INK_SHELF").unwrap();
    for size in ["1024x720", "1920x1080"] {
        for round in 0..10 {
            let arms = [(&after, "cpu", "cpu"), (&after, "ink", "after")];
            for step in 0..arms.len() {
                let (exe, painter, arm) = arms[(round + step) % arms.len()];
                let name = format!("startup-{size}-{round}-{arm}");
                let mut app = Shelf::launch(&name, exe, painter, size, Some(120), false);
                until("bounded startup run finishes", || {
                    app.child.try_wait().unwrap().is_some()
                });
                assert!(app.child.wait().unwrap().success(), "{}", app.text());
                if arm == "after" {
                    let log = app.text();
                    let first = log
                        .lines()
                        .find(|s| s.starts_with("exact-windows: frame 1 at "))
                        .unwrap();
                    assert!(first.contains("painter ink"), "{first}");
                    assert!(
                        !log.contains("migration") && !log.contains("warp true"),
                        "{log}"
                    );
                }
                let log = app.text();
                assert!(
                    log.find("first present returned").unwrap()
                        < log.find("boot jump-list-started").unwrap(),
                    "{log}"
                );
                println!("S2_STARTUP {name}\n{log}");
            }
        }
    }
}

#[test]
#[ignore = "NucBox desktop; same startup series without the synchronization probe"]
fn shelf_startup_intervals() {
    std::env::set_var("EXACT_AGENT_WINDOW_PROBE", "off");
    shelf_startup_series();
    std::env::remove_var("EXACT_AGENT_WINDOW_PROBE");
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_resize() {
    let mut app = Shelf::open("resize", false);
    app.capture("resize-before");
    for (w, h) in [(800, 600), (1280, 900), (1920, 1080), (1024, 720)] {
        app.drag_size(w, h);
        app.capture(&format!("resize-{w}x{h}"));
    }
    fresh_comparison("resize", app);
}

#[test]
#[ignore = "NucBox desktop; INK_SHELF names built Shelf.exe"]
fn shelf_minimize_restore() {
    let mut app = Shelf::open("minimize", false);
    app.capture("minimize-before");
    unsafe {
        let _ = ShowWindowAsync(app.hwnd, SW_MINIMIZE);
    }
    until("minimized", || {
        app.alive();
        unsafe { IsIconic(app.hwnd).as_bool() }
    });
    unsafe {
        let _ = ShowWindowAsync(app.hwnd, SW_RESTORE);
    }
    until("restored", || {
        app.alive();
        unsafe { !IsIconic(app.hwnd).as_bool() }
    });
    fresh_comparison("minimize", app);
}

#[test]
#[ignore = "NucBox desktop; system caret blink enabled; INK_SHELF names Shelf.exe"]
fn shelf_idle_caret() {
    let mut app = Shelf::open("caret", true);
    let thread = unsafe { GetWindowThreadProcessId(app.hwnd, None) };
    let mut gui = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    until("native caret in Shelf's focused field", || {
        app.alive();
        unsafe { GetGUIThreadInfo(thread, &mut gui).is_ok() && gui.hwndCaret == app.hwnd }
    });
    // Softbuffer patches the caret without logging a whole frame.
    let first = app.capture("caret-first");
    let mut second = None;
    let mut changes = 0;
    let mut previous = first.clone();
    let mut changed_pixels = 0;
    until("three idle caret transitions", || {
        let image = app.capture_pixels();
        let changed: Vec<_> = previous
            .data()
            .chunks_exact(4)
            .zip(image.data().chunks_exact(4))
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert!(
            changed.len() < 2000,
            "idle blink changed {} pixels",
            changed.len()
        );
        for &i in &changed {
            let x = (i as u32 % image.width()) as i32;
            let y = (i as u32 / image.width()) as i32;
            assert!(
                x >= gui.rcCaret.left - 1
                    && x <= gui.rcCaret.right + 1
                    && y >= gui.rcCaret.top - 1
                    && y <= gui.rcCaret.bottom + 1,
                "idle change outside the caret at {x},{y}"
            );
        }
        if !changed.is_empty() {
            changed_pixels = changed.len();
            second.get_or_insert_with(|| image.clone());
            previous = image;
            changes += 1;
        }
        changes >= 3
    });
    let second = second.unwrap();
    second.save_png(out().join("caret-second.png")).unwrap();
    println!("S2_CARET transitions={changes} changed_pixels={changed_pixels}");
    app.close();
    let mut fresh = Shelf::open("caret-fresh", true);
    let image = fresh.capture("caret-fresh");
    let score = [band(&first, &image), band(&second, &image)]
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    assert!(score.0 < 5. && score.1 < 0.06, "{score:?}");
    println!("S2_BAND caret mean={} over32={}", score.0, score.1);
    fresh.close();
}

#[test]
#[ignore = "NucBox desktop; operator cycles Settings scaling 100%, 150%, 100%, 150%, 100%"]
fn shelf_dpi_roundtrip() {
    let mut app = Shelf::open("dpi", false);
    assert_eq!(unsafe { GetDpiForWindow(app.hwnd) }, 96);
    fs::write(out().join("dpi-waiting"), app.child.id().to_string()).unwrap();
    for pass in ["150", "roundtrip-150"] {
        until("Settings display scaling 150%", || {
            app.alive();
            (unsafe { GetDpiForWindow(app.hwnd) == 144 })
                && app
                    .text()
                    .lines()
                    .rev()
                    .find(|s| s.starts_with("exact-windows: frame "))
                    .is_some_and(|s| s.contains(", scale 1.5,"))
        });
        unsafe {
            let _ = SetForegroundWindow(app.hwnd);
        }
        app.place(1024, 720);
        let changed = app.capture(&format!("dpi-{pass}"));
        let mut fresh = Shelf::open(&format!("dpi-fresh-{pass}"), false);
        assert_eq!(unsafe { GetDpiForWindow(fresh.hwnd) }, 144);
        let image = fresh.capture(&format!("dpi-fresh-{pass}"));
        let score = band(&changed, &image);
        println!("S2_BAND dpi-{pass} mean={} over32={}", score.0, score.1);
        assert!(
            score.0 < 5. && score.1 < 0.06,
            "150% differs from fresh launch: {score:?}"
        );
        fresh.close();
        fs::write(out().join(format!("dpi-at-{pass}")), "return to 100%").unwrap();
        until("Settings display scaling 100%", || {
            app.alive();
            (unsafe { GetDpiForWindow(app.hwnd) == 96 })
                && app
                    .text()
                    .lines()
                    .rev()
                    .find(|s| s.starts_with("exact-windows: frame "))
                    .is_some_and(|s| s.contains(", scale 1,"))
        });
        app.place(1024, 720);
        assert_eq!((app.client().right, app.client().bottom), (1024, 720));
        println!("S2_DPI after-{pass} client=1024x720 dpi=96");
        app.capture(&format!("dpi-after-{pass}-100"));
        fs::write(out().join(format!("dpi-after-{pass}")), "at 100%").unwrap();
    }
    println!("S2_DPI 96 -> 144 -> 96 -> 144 -> 96");
    fresh_comparison("dpi", app);
}

#[test]
#[ignore = "NucBox desktop; forces a real GPU TDR with dxcap -forcetdr"]
fn shelf_device_loss() {
    let wgpu = std::env::var("EXACT_INK_DEVICE").as_deref() == Ok("wgpu");
    let mut app = Shelf::launch_fault(
        "device-loss",
        &std::env::var("INK_SHELF").unwrap(),
        if wgpu { "wgpu" } else { "ink" },
        "1024x720",
        None,
        false,
        (!wgpu).then_some("mica-recovery"),
    );
    app.capture("device-loss-before");
    let mut reset = Command::new("dxcap.exe").arg("-forcetdr").spawn().unwrap();
    println!("S2_PROCESS dxcap pid={}", reset.id());
    assert!(
        reset.wait().unwrap().success(),
        "dxcap could not force a TDR"
    );
    // Force a changed extent so an otherwise idle app observes the removed device.
    app.place(1025, 720);
    if std::env::var("EXACT_INK_DEVICE").as_deref() == Ok("wgpu") {
        until("wgpu loss presents the CPU fallback", || {
            app.alive();
            app.text().contains("painting on the CPU from here")
                && app
                    .text()
                    .lines()
                    .rev()
                    .find(|s| s.starts_with("exact-windows: frame "))
                    .is_some_and(|s| s.contains("painter cpu"))
        });
        let after = app.capture("device-loss-cpu");
        app.close();
        let mut fresh = Shelf::open("device-loss-fresh", false);
        fresh.place(1025, 720);
        let score = band(&after, &fresh.capture("device-loss-fresh"));
        assert!(score.0 < 5. && score.1 < 0.06, "{score:?}");
        println!("S2_BAND device-loss mean={} over32={}", score.0, score.1);
        fresh.close();
        return;
    }
    until("CPU fallback presented through composition", || {
        app.alive();
        app.text()
            .contains("CPU recovery frame presented through composition")
    });
    app.capture("device-loss-cpu");
    until("hardware recovered after backoff", || {
        app.alive();
        let text = app.text();
        let loss = text.rfind("painting on the CPU");
        loss.is_some_and(|offset| text[offset..].contains("painter ink"))
    });
    let recovered = app.text();
    assert!(
        recovered.contains("injected Mica refusal during device recovery"),
        "{recovered}"
    );
    assert!(
        recovered.contains("Mica retry 1/3 after present: restored"),
        "{recovered}"
    );
    assert!(recovered.contains("WARP composition"));
    let presented = recovered
        .find("CPU recovery frame presented through composition")
        .unwrap();
    let mut frames = recovered[presented..]
        .lines()
        .filter(|s| s.starts_with("exact-windows: frame "));
    let cpu = frames.find(|s| s.contains("painter cpu")).unwrap();
    let ink = frames.find(|s| s.contains("painter ink")).unwrap();
    let at = |line: &str| {
        line.split_once(" at ")
            .unwrap()
            .1
            .split_once(" ms,")
            .unwrap()
            .0
            .parse::<f64>()
            .unwrap()
    };
    let backoff = at(ink) - at(cpu);
    assert!(
        backoff >= 250.,
        "recovered before the retry deadline: {backoff} ms"
    );
    println!("S2_RECOVERY cpu_to_ink_ms={backoff}");
    // Retain the expected loss log separately; the ordinary comparison rejects losses.
    fs::write(out().join("device-loss-recovery.log"), &recovered).unwrap();
    let after = app.capture("device-loss-recovered");
    app.close();
    let mut fresh = Shelf::open("device-loss-fresh", false);
    fresh.place(1025, 720);
    let image = fresh.capture("device-loss-fresh");
    let score = band(&after, &image);
    assert!(score.0 < 5. && score.1 < 0.06, "{score:?}");
    println!("S2_BAND device-loss mean={} over32={}", score.0, score.1);
    fresh.close();
}

#[test]
#[ignore = "NucBox desktop; a person must unlock after LockWorkStation"]
fn shelf_lock_unlock() {
    assert!(unlocked(), "desktop must start unlocked");
    let mut app = Shelf::open("lock", false);
    app.capture("lock-before");
    let mut lock = Command::new("rundll32.exe")
        .arg("user32.dll,LockWorkStation")
        .spawn()
        .unwrap();
    println!("S2_PROCESS lock pid={}", lock.id());
    assert!(lock.wait().unwrap().success());
    until("locked input desktop", || {
        app.alive();
        !unlocked()
    });
    fs::write(out().join("lock-waiting"), "Unlock the NucBox to continue").unwrap();
    until("person unlocks the desktop", || {
        app.alive();
        unlocked()
    });
    fresh_comparison("lock", app);
}

#[test]
#[ignore = "NucBox desktop; sleep and a 15-second wake timer; unlock if Windows requires it"]
fn shelf_sleep_wake() {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, LUID},
            Security::*,
            System::{Power::*, Threading::*},
        },
    };
    fn wake_display() {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        unsafe {
            assert_ne!(
                SetThreadExecutionState(ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED).0,
                0
            );
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 1,
                        dwFlags: MOUSEEVENTF_MOVE,
                        ..Default::default()
                    },
                },
            };
            assert_eq!(SendInput(&[input], size_of::<INPUT>() as i32), 1);
        }
    }
    let screen = || unsafe { [GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)] };
    wake_display();
    until("physical desktop before sleep", || screen() == [5120, 1440]);
    let mut app = Shelf::open("sleep", false);
    app.capture("sleep-before");
    unsafe {
        // SetSuspendState requires SeShutdownPrivilege in the caller's token.
        // https://learn.microsoft.com/windows/win32/api/powrprof/nf-powrprof-setsuspendstate
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token).unwrap();
        let mut luid = LUID::default();
        LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid).unwrap();
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None).unwrap();
        let error = GetLastError();
        CloseHandle(token).unwrap();
        assert_ne!(error, ERROR_NOT_ALL_ASSIGNED, "sleep privilege unavailable");
        let timer = CreateWaitableTimerW(None, true, PCWSTR::null()).unwrap();
        SetWaitableTimer(timer, &(-150_000_000i64), 0, None, None, true).unwrap();
        let started = Instant::now();
        let suspended = SetSuspendState(false, false, false);
        CloseHandle(timer).unwrap();
        assert!(
            suspended,
            "Windows refused sleep: {}",
            std::io::Error::last_os_error()
        );
        println!(
            "S2_SLEEP returned_after_ms={}",
            started.elapsed().as_millis()
        );
    }
    until("unlocked desktop after wake", || {
        app.alive();
        unlocked()
    });
    // A resume timer wakes the system, not necessarily its physical display.
    wake_display();
    until("physical desktop after wake", || {
        app.alive();
        screen() == [5120, 1440]
    });
    fresh_comparison("sleep", app);
}
