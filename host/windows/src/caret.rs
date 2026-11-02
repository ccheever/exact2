//! System caret geometry for accessibility; exact painted pixels for blinking.
use std::{
    num::NonZeroU32,
    ptr,
    sync::Arc,
    time::{Duration, Instant},
};
use tiny_skia::Pixmap;
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{CreateBitmap, DeleteObject, HBITMAP},
    UI::WindowsAndMessaging::{
        CreateCaret, DestroyCaret, GetCaretBlinkTime, SetCaretPos, ShowCaret,
        SystemParametersInfoW, SPI_GETCARETTIMEOUT,
    },
};

#[derive(Default)]
pub(crate) struct Caret {
    clock: Blink,
    native: Native,
    frame: Option<Arc<Pixmap>>,
    patch: Option<Patch>,
}

#[derive(Default)]
struct Blink {
    active: bool,
    hidden: bool,
    interval: Option<Duration>,
    timeout: Option<Duration>,
    next: Option<Instant>,
    end: Option<Instant>,
}

impl Blink {
    fn reset(&mut self, now: Instant) {
        self.hidden = false;
        self.next = self
            .active
            .then_some(self.interval)
            .flatten()
            .map(|d| now + d);
        self.end = self
            .active
            .then_some(self.timeout)
            .flatten()
            .map(|d| now + d);
    }
    fn deadline(&self) -> Option<Instant> {
        self.next
            .map(|next| self.end.map_or(next, |end| next.min(end)))
    }
    fn advance(&mut self, now: Instant) -> bool {
        if !self.deadline().is_some_and(|due| now >= due) {
            return false;
        }
        let was = self.hidden;
        if self.end.is_some_and(|end| now >= end) {
            self.hidden = false;
            self.next = None;
            self.end = None;
        } else {
            self.hidden = !self.hidden;
            self.next = self.interval.map(|d| now + d);
        }
        was != self.hidden
    }
}

impl Caret {
    pub fn visible(&self) -> bool {
        !self.clock.hidden
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.clock.deadline()
    }
    pub fn activity(&mut self, now: Instant) {
        self.clock.reset(now);
    }
    /// A full repaint replaces this picture before another blink can run.
    pub fn take_previous_frame(&mut self) -> Option<Arc<Pixmap>> {
        self.frame.take()
    }
    pub fn clear(&mut self) {
        self.clock.active = false;
        self.clock.next = None;
        self.clock.end = None;
        self.clock.hidden = false;
        self.frame = None;
        self.patch = None;
        self.native.clear();
    }

    pub fn presented(
        &mut self,
        hwnd: isize,
        rect: Option<(f32, f32, f32, f32)>,
        scale: f32,
        frame: Option<Arc<Pixmap>>,
        now: Instant,
    ) -> bool {
        let was_hidden = self.clock.hidden;
        let Some(rect) = rect else {
            self.clear();
            return was_hidden && frame.is_some();
        };
        let next = [
            (rect.0 * scale).round() as i32,
            (rect.1 * scale).round() as i32,
            (rect.2 * scale).round().max(1.) as i32,
            (rect.3 * scale).round().max(1.) as i32,
        ];
        if !self.clock.active {
            // Read the user's preferences. Never change the shared system blink rate.
            #[allow(unsafe_code)]
            unsafe {
                let ms = GetCaretBlinkTime();
                self.clock.interval =
                    (ms > 0 && ms != u32::MAX).then(|| Duration::from_millis(ms.into()));
                let mut timeout = 0_u32;
                self.clock.timeout = (SystemParametersInfoW(
                    SPI_GETCARETTIMEOUT,
                    0,
                    ptr::from_mut(&mut timeout).cast(),
                    0,
                ) != 0
                    && timeout > 0
                    && timeout != u32::MAX)
                    .then(|| Duration::from_millis(timeout.into()));
            }
            self.clock.active = true;
            self.clock.reset(now);
        }
        if self.native.rect != Some(next) {
            self.clock.reset(now);
        }
        self.native.update(hwnd, next);
        self.frame = self.clock.deadline().and(frame);
        self.patch = None;
        was_hidden && !self.clock.hidden
    }

    pub fn advance(&mut self, now: Instant) -> bool {
        self.clock.advance(now)
    }
    pub fn needs_picture(&self) -> bool {
        self.patch.is_none()
    }
    pub fn can_patch(&self) -> bool {
        self.frame.is_some() || self.patch.is_some()
    }
    /// A new picture invalidates both sides. The first idle blink renders the
    /// other side once; later blinks copy only their differing rectangle.
    pub fn compare(&mut self, other: Arc<Pixmap>) {
        self.patch = self.frame.take().and_then(|frame| {
            if self.clock.hidden {
                Patch::between(&frame, &other)
            } else {
                Patch::between(&other, &frame)
            }
        });
        if self.patch.is_none() {
            self.clock.next = None;
        }
    }
    pub fn paint(&self, target: &mut [u32], width: u32) -> Option<softbuffer::Rect> {
        let patch = self.patch.as_ref()?;
        let pixels = if self.clock.hidden {
            &patch.off
        } else {
            &patch.on
        };
        let right = patch.rect.x.checked_add(patch.rect.width.get())?;
        let bottom = patch.rect.y.checked_add(patch.rect.height.get())?;
        if right > width || u64::from(bottom) * u64::from(width) > target.len() as u64 {
            return None;
        }
        for y in 0..patch.rect.height.get() as usize {
            let start = (patch.rect.y as usize + y) * width as usize + patch.rect.x as usize;
            let source = y * patch.rect.width.get() as usize;
            let end = start + patch.rect.width.get() as usize;
            target
                .get_mut(start..end)?
                .copy_from_slice(&pixels[source..source + end - start]);
        }
        Some(patch.rect)
    }
}

struct Patch {
    rect: softbuffer::Rect,
    on: Vec<u32>,
    off: Vec<u32>,
}
impl Patch {
    fn between(on: &Pixmap, off: &Pixmap) -> Option<Self> {
        if (on.width(), on.height()) != (off.width(), off.height()) {
            return None;
        }
        let (w, h) = (on.width(), on.height());
        let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
        for (i, (a, b)) in on.pixels().iter().zip(off.pixels()).enumerate() {
            if a != b {
                let (x, y) = (i as u32 % w, i as u32 / w);
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        let rect = softbuffer::Rect {
            x: left,
            y: top,
            width: NonZeroU32::new(right.checked_sub(left)?)?,
            height: NonZeroU32::new(bottom.checked_sub(top)?)?,
        };
        let copy = |p: &Pixmap| {
            let mut pixels =
                Vec::with_capacity(rect.width.get() as usize * rect.height.get() as usize);
            for y in top..bottom {
                for x in left..right {
                    let pixel = p.pixel(x, y).unwrap();
                    pixels.push(
                        (u32::from(pixel.red()) << 16)
                            | (u32::from(pixel.green()) << 8)
                            | u32::from(pixel.blue()),
                    );
                }
            }
            pixels
        };
        Some(Self {
            rect,
            on: copy(on),
            off: copy(off),
        })
    }
}

#[derive(Default)]
struct Native {
    hwnd: HWND,
    bitmap: HBITMAP,
    rect: Option<[i32; 4]>,
}
impl Native {
    fn clear(&mut self) {
        // SAFETY: this object belongs to the window's UI thread and is cleared
        // before its HWND is destroyed. The bitmap outlives the caret using it.
        #[allow(unsafe_code)]
        unsafe {
            if !self.hwnd.is_null() {
                DestroyCaret();
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
        }
        self.hwnd = ptr::null_mut();
        self.bitmap = ptr::null_mut();
        self.rect = None;
    }
    fn update(&mut self, hwnd: isize, rect: [i32; 4]) {
        if self.rect == Some(rect) && self.hwnd == hwnd as HWND {
            return;
        }
        let [x, y, w, h] = rect;
        if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
            self.clear();
            return;
        }
        let recreate = self.hwnd != hwnd as HWND || self.rect.is_none_or(|r| r[2..] != rect[2..]);
        if recreate {
            self.clear();
        }
        // A zero monochrome bitmap XORs nothing. Windows still publishes the
        // real caret position to accessibility clients and magnifiers.
        #[allow(unsafe_code)]
        unsafe {
            if recreate {
                let zero = vec![0_u8; (w as usize).div_ceil(16) * 2 * h as usize];
                self.bitmap = CreateBitmap(w, h, 1, 1, zero.as_ptr().cast());
                if self.bitmap.is_null() || CreateCaret(hwnd as HWND, self.bitmap, w, h) == 0 {
                    self.clear();
                    return;
                }
                self.hwnd = hwnd as HWND;
                ShowCaret(self.hwnd);
            }
            SetCaretPos(x, y);
        }
        self.rect = Some(rect);
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_restarts_visible_and_stops_at_the_system_timeout() {
        let now = Instant::now();
        let mut blink = Blink {
            active: true,
            interval: Some(Duration::from_millis(500)),
            timeout: Some(Duration::from_millis(1200)),
            ..Default::default()
        };
        blink.reset(now);
        assert!(!blink.advance(now + Duration::from_millis(499)));
        assert!(blink.advance(now + Duration::from_millis(500)));
        assert!(blink.hidden);
        blink.reset(now + Duration::from_millis(600));
        assert!(!blink.hidden);
        assert_eq!(blink.deadline(), Some(now + Duration::from_millis(1100)));
        assert!(blink.advance(now + Duration::from_millis(1100)));
        assert!(blink.advance(now + Duration::from_millis(1600)));
        assert!(!blink.hidden);
        assert_eq!(blink.deadline(), Some(now + Duration::from_millis(1800)));
        assert!(!blink.advance(now + Duration::from_millis(1800)));
        assert_eq!(blink.deadline(), None);

        blink.timeout = Some(Duration::from_millis(700));
        blink.reset(now);
        assert!(blink.advance(now + Duration::from_millis(500)));
        assert!(blink.advance(now + Duration::from_millis(700)));
        assert!(!blink.hidden);
        assert_eq!(blink.deadline(), None);
        blink.interval = None;
        blink.reset(now);
        assert_eq!(blink.deadline(), None);
        assert!(!blink.advance(now + Duration::from_secs(60)));
    }

    #[test]
    fn cached_blinks_restore_exact_pixels_and_reject_a_resized_buffer() {
        let mut off = Pixmap::new(7, 5).unwrap();
        off.fill(tiny_skia::Color::from_rgba8(16, 32, 48, 255));
        let mut on = off.clone();
        for (x, y, r, g, b) in [(2, 1, 200, 30, 90), (3, 3, 40, 100, 250)] {
            on.pixels_mut()[y * 7 + x] =
                tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255).unwrap();
        }
        let mut caret = Caret {
            frame: Some(Arc::new(on.clone())),
            ..Caret::default()
        };
        caret.clock.hidden = true;
        caret.compare(Arc::new(off.clone()));
        assert!(
            caret.frame.is_none(),
            "release the full frame after caching"
        );
        assert!(!caret.needs_picture());
        assert!(caret.can_patch());
        let rgb = |p: &Pixmap| {
            p.pixels()
                .iter()
                .map(|p| {
                    (u32::from(p.red()) << 16) | (u32::from(p.green()) << 8) | u32::from(p.blue())
                })
                .collect::<Vec<_>>()
        };
        let mut pixels = rgb(&on);
        for hidden in [true, false, true, false] {
            caret.clock.hidden = hidden;
            let damage = caret.paint(&mut pixels, 7).unwrap();
            assert_eq!(
                (damage.x, damage.y, damage.width.get(), damage.height.get()),
                (2, 1, 2, 3)
            );
            assert_eq!(pixels, rgb(if hidden { &off } else { &on }));
        }
        let mut resized = vec![0_u32; 15];
        assert!(caret.paint(&mut resized, 3).is_none());
        assert_eq!(resized, vec![0; 15]);
        assert!(Patch::between(&off, &off).is_none());
        assert!(Patch::between(&off, &Pixmap::new(8, 5).unwrap()).is_none());
    }

    #[test]
    fn a_presented_caret_without_cpu_pixels_keeps_its_blink_deadline() {
        let now = Instant::now();
        let mut caret = Caret::default();
        caret.clock.active = true;
        caret.clock.interval = Some(Duration::from_millis(500));
        caret.clock.reset(now);
        // Matching geometry avoids touching the process's native caret in this test.
        caret.native.rect = Some([4, 6, 1, 16]);
        assert!(!caret.presented(0, Some((4., 6., 1., 16.)), 1., None, now));
        assert!(!caret.can_patch());
        assert_eq!(caret.deadline(), Some(now + Duration::from_millis(500)));
        assert!(caret.advance(now + Duration::from_millis(500)));
        assert!(!caret.visible());
        assert_eq!(caret.deadline(), Some(now + Duration::from_millis(1000)));
        assert!(
            !caret.advance(now + Duration::from_millis(500)),
            "expired deadline must be consumed"
        );
        assert!(!caret.presented(
            0,
            Some((4., 6., 1., 16.)),
            1.,
            None,
            now + Duration::from_millis(500),
        ));
        assert!(!caret.visible());
        assert!(caret.advance(now + Duration::from_millis(1000)));
        assert!(caret.visible());
    }
}
