//! The display: DRM/KMS with two dumb buffers and a page flip per frame,
//! and the frame loop that owns the process when there is a screen.
//!
//! @ref LLP 1015 §6; LLP 1009 D5 as LLP 1015 §7 amends it (the GPU painter
//! compiles its shaders on the first launch on a machine and reads them
//! from the cache after; the display itself is dumb buffers and a readback
//! until the KMS surface lands)
//!
//! The card's first connected connector at its preferred mode; two
//! XRGB8888 dumb buffers; the first frame by `set_crtc`, every later one by
//! a page flip whose event is waited for, which paces the loop to the
//! display's refresh. Frames are painted only when something changed:
//! input, a timer, a motion frame, an image, a reload. Needs DRM master —
//! a VT, or a card nobody else holds — and the `video` group.

#![allow(unsafe_code)]

use crate::app::Config;
use crate::input::{Input, InputEvent, Key};
use crate::presenter::Presenter;
use crate::vnc::Vnc;
use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::{
    connector, crtc, dumbbuffer::DumbBuffer, framebuffer, Device as ControlDevice, Event, Mode,
    ModeTypeFlags, PageFlipFlags,
};
use drm::Device;
use exact_runner::DataSource;
use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tiny_skia::Pixmap;

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl Device for Card {}
impl ControlDevice for Card {}

/// A KMS output with two dumb buffers.
pub struct Display {
    card: Card,
    crtc: crtc::Handle,
    connector: connector::Handle,
    mode: Mode,
    buffers: Vec<(DumbBuffer, framebuffer::Handle)>,
    front: usize,
    first: bool,
    width: u32,
    height: u32,
}

impl Display {
    /// Open the card and take its first connected connector at its
    /// preferred mode.
    pub fn open(path: &str) -> Result<Display, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("{path}: {e}"))?;
        let card = Card(file);
        // Master follows the VT; when nobody holds it, ask.
        let _ = card.acquire_master_lock();
        let res = card
            .resource_handles()
            .map_err(|e| format!("resources: {e}"))?;
        let con = res
            .connectors()
            .iter()
            .filter_map(|c| card.get_connector(*c, true).ok())
            .find(|c| c.state() == connector::State::Connected && !c.modes().is_empty())
            .ok_or_else(|| "no connected connector with a mode".to_string())?;
        let mode = con
            .modes()
            .iter()
            .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| con.modes().first())
            .copied()
            .ok_or_else(|| "no mode".to_string())?;
        let crtc = con
            .current_encoder()
            .and_then(|e| card.get_encoder(e).ok())
            .and_then(|e| e.crtc())
            .or_else(|| res.crtcs().first().copied())
            .ok_or_else(|| "no crtc".to_string())?;
        let (w, h) = mode.size();
        let (width, height) = (w as u32, h as u32);
        let mut buffers = Vec::new();
        for _ in 0..2 {
            let db = card
                .create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)
                .map_err(|e| format!("dumb buffer: {e}"))?;
            let fb = card
                .add_framebuffer(&db, 24, 32)
                .map_err(|e| format!("framebuffer: {e}"))?;
            buffers.push((db, fb));
        }
        Ok(Display {
            card,
            crtc,
            connector: con.handle(),
            mode,
            buffers,
            front: 0,
            first: true,
            width,
            height,
        })
    }

    /// The mode's size, pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The mode's refresh rate, Hz.
    pub fn refresh(&self) -> u32 {
        self.mode.vrefresh()
    }

    /// Show a frame: copied into the back buffer as XRGB, then flipped to;
    /// returns when the flip has happened (the next vblank).
    pub fn present(&mut self, frame: &Pixmap) -> Result<(), String> {
        let back = (self.front + 1) % self.buffers.len();
        let Display { card, buffers, .. } = self;
        {
            let (db, _) = &mut buffers[back];
            let pitch = db.pitch() as usize;
            let mut map = card.map_dumb_buffer(db).map_err(|e| format!("map: {e}"))?;
            copy_xrgb(frame, map.as_mut(), pitch, self.width, self.height);
        }
        let fb = buffers[back].1;
        if self.first {
            card.set_crtc(
                self.crtc,
                Some(fb),
                (0, 0),
                &[self.connector],
                Some(self.mode),
            )
            .map_err(|e| format!("set_crtc: {e}"))?;
            self.first = false;
        } else {
            card.page_flip(self.crtc, fb, PageFlipFlags::EVENT, None)
                .map_err(|e| format!("page flip: {e}"))?;
            loop {
                let events = card.receive_events().map_err(|e| format!("events: {e}"))?;
                if events.into_iter().any(|e| matches!(e, Event::PageFlip(_))) {
                    break;
                }
            }
        }
        self.front = back;
        Ok(())
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        for (db, fb) in self.buffers.drain(..) {
            let _ = self.card.destroy_framebuffer(fb);
            let _ = self.card.destroy_dumb_buffer(db);
        }
    }
}

/// Premultiplied RGBA rows into an XRGB8888 (little-endian: B, G, R, X)
/// buffer with its pitch; the page is opaque, so premultiplied is straight.
pub fn copy_xrgb(frame: &Pixmap, dst: &mut [u8], pitch: usize, width: u32, height: u32) {
    let w = frame.width().min(width) as usize;
    let h = frame.height().min(height) as usize;
    let src = frame.data();
    for y in 0..h {
        let row = &src[y * frame.width() as usize * 4..][..w * 4];
        let out = &mut dst[y * pitch..][..w * 4];
        for (s, d) in row.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 0xff;
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// The display loop: paint when something changed, present, wait for input
/// or the next timer or motion frame, repeat. Exit code.
pub fn run<D: DataSource + Default>(config: &Config, started: Instant) -> i32 {
    let mut display = match Display::open(&config.card) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "exact: no display: {e}\n  (a VT and the video group, or EXACT_AGENT=1 / EXACT_SHOT=<png> / EXACT_SMOKE=1 to run without one)"
            );
            return 1;
        }
    };
    let (pw, ph) = display.size();
    let viewport = (pw as f32 / config.scale, ph as f32 / config.scale);
    let wall = || started.elapsed().as_secs_f64() * 1000.0;
    let (mut p, error) = match Presenter::boot(
        &config.plan,
        D::default(),
        viewport,
        config.scale,
        config.assets.clone(),
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("exact: boot: {e}");
            return 1;
        }
    };
    if let Some(e) = error {
        eprintln!("exact: {e}");
    }
    let mut input = Input::open();
    let mut vnc = match config.vnc.as_deref() {
        Some(addr) => match Vnc::start(addr, pw, ph) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("exact: vnc: {e}");
                None
            }
        },
        None => None,
    };
    let mut pointer = (viewport.0 / 2.0, viewport.1 / 2.0);
    p.set_pointer(Some(pointer));
    let mut down: Option<u32> = None;
    let mut last_tick = 0.0f64;
    let mut plan_seen = config.dev_plan.as_deref().and_then(mtime);
    println!(
        "exact: {pw}x{ph} @{}Hz on {}, scale {}, {} input device(s){}, boot {:.1} ms",
        display.refresh(),
        config.card,
        config.scale,
        input.len(),
        match (&vnc, config.vnc.as_deref()) {
            (Some(_), Some("1")) => ", vnc on :5900".to_string(),
            (Some(_), Some(a)) => format!(", vnc on {a}"),
            _ => String::new(),
        },
        wall()
    );
    loop {
        if p.dirty() {
            let frame = p.frame();
            if let Err(e) = display.present(&frame) {
                eprintln!("exact: {e}");
                return 1;
            }
            if let Some(v) = &vnc {
                v.publish(Arc::new(frame));
            }
        }
        let now = wall();
        let mut timeout: i32 = -1;
        if p.host().motion() {
            timeout = 0;
        } else if p.host().has_timers() {
            timeout = (last_tick + 250.0 - now).max(0.0) as i32;
        }
        if p.images().pending() || config.dev_plan.is_some() {
            timeout = if timeout < 0 { 100 } else { timeout.min(100) };
        }
        if input.is_empty() && vnc.is_none() && timeout < 0 {
            timeout = 1000;
        }
        let mut fds = input.fds();
        if let Some(v) = &vnc {
            fds.push(v.fd());
        }
        // A reply from the executor wakes the loop like a key would.
        fds.push(p.executor_fd());
        poll(&fds, timeout);
        if let Some(e) = p.pump(wall()) {
            eprintln!("exact: {e}");
        }
        let mut events = input.read();
        if let Some(v) = vnc.as_mut() {
            events.extend(v.take_events());
        }
        for ev in events {
            match ev {
                InputEvent::Motion(dx, dy) => {
                    pointer.0 = (pointer.0 + dx / config.scale).clamp(0.0, viewport.0 - 1.0);
                    pointer.1 = (pointer.1 + dy / config.scale).clamp(0.0, viewport.1 - 1.0);
                    p.set_pointer(Some(pointer));
                }
                InputEvent::Absolute(fx, fy) => {
                    if let Some(fx) = fx {
                        pointer.0 = (fx * viewport.0).clamp(0.0, viewport.0 - 1.0);
                    }
                    if let Some(fy) = fy {
                        pointer.1 = (fy * viewport.1).clamp(0.0, viewport.1 - 1.0);
                    }
                    p.set_pointer(Some(pointer));
                }
                InputEvent::Button(true) => down = p.hit(pointer.0, pointer.1),
                InputEvent::Button(false) => {
                    let was = down.take();
                    let at = p.hit(pointer.0, pointer.1);
                    if was.is_some() && was == at {
                        p.press_at(pointer.0, pointer.1, wall());
                    }
                }
                InputEvent::Wheel(dx, dy) => p.wheel_at(pointer.0, pointer.1, dx, dy),
                InputEvent::Key(Key::Char(c)) => p.key(Some(c), false, wall()),
                InputEvent::Key(Key::Backspace) => p.key(None, true, wall()),
                InputEvent::Key(Key::Escape) => p.blur(),
                InputEvent::Key(Key::Enter) => {}
            }
        }
        let now = wall();
        if p.host().has_timers() && now - last_tick >= 250.0 {
            last_tick = now;
            if let Some(e) = p.advance(now) {
                eprintln!("exact: {e}");
            }
        }
        if p.host().motion() {
            p.tick(now);
        }
        p.poll_images();
        if let Some(path) = &config.dev_plan {
            let m = mtime(path);
            if m.is_some() && m != plan_seen {
                plan_seen = m;
                if let Ok(bytes) = std::fs::read(path) {
                    let t = Instant::now();
                    match p.reload(&bytes, D::default()) {
                        Ok(e) => println!(
                            "reloaded {} in {:.1} ms{}",
                            path.file_name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            t.elapsed().as_secs_f64() * 1000.0,
                            e.map(|e| format!(" — {e}")).unwrap_or_default()
                        ),
                        Err(e) => eprintln!("exact: reload: {e}"),
                    }
                }
            }
        }
    }
}

/// Wait for any of the descriptors to be readable, or `timeout` ms (-1
/// forever).
fn poll(fds: &[i32], timeout: i32) {
    let mut pfds: Vec<libc::pollfd> = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    // SAFETY: the array is ours and sized by its length; poll reads and
    // writes only inside it.
    unsafe {
        libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout);
    }
}
