//! The window a device's shell shows the page in, edge to edge (LLP 1115.001
//! D3.3, D3.4, D3.6): the shell tells its size and the raw insets the system
//! reports, and pads nothing itself. The presenter alone decides what they
//! mean, so no inset is applied twice:
//!
//! - **The viewport.** Under `viewport-fit="cover"` on the first root the
//!   page is laid out in the whole window and `env(safe-area-inset-*)`
//!   answers the system bars and the display cutout. Without it the page is
//!   laid out inside them and the four are zero, as a browser lays out a page
//!   that did not ask for the edges.
//! - **The keyboard.** A software keyboard ends the layout viewport at its
//!   top (`interactive-widget="resizes-content"`, the default on a phone as
//!   LLP 1116 D2 has it on iOS), where the bottom inset is the keyboard's and
//!   not the navigation bar's; the field being edited is then kept in view.
//!   `overlays-content` on the root lays nothing out again. The overlap is
//!   `layout.env["keyboard-inset-height"]` either way.
//! - **What the shell draws.** Where the page is in the window, the colour
//!   behind it and into the edges it keeps out of, and whether the scheme is
//!   dark, which the shell's bar icons follow ([`Chrome`]).
//!
//! Desktop Linux has no shell that calls this: its viewport is the display,
//! and its insets stay zero.
use super::*;

/// A window's size and the system's insets on it, in points; each inset is
/// top, right, bottom, left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Window {
    /// The window's width and height.
    pub size: (f32, f32),
    /// The status, navigation and caption bars.
    pub bars: [f32; 4],
    /// The display's cutout.
    pub cutout: [f32; 4],
    /// The software keyboard.
    pub keyboard: [f32; 4],
    /// The edges the system's own gestures start from (Back's sides, Home's
    /// bottom). An authored swipe yields the sides to them (D7.5,
    /// [`Presenter::in_gesture_edge`]).
    pub gestures: [f32; 4],
}

/// What the shell draws around the page, from [`Presenter::chrome`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chrome {
    /// Where the page is in the window: x, y, width, height, in points.
    pub frame: [f32; 4],
    /// The colour behind the page and in the edges it is kept out of (RGBA):
    /// the first root's own background where it paints an opaque one, as a
    /// browser carries the root's background over the whole canvas; else the
    /// platform's surface for the scheme (`Canvas`, LLP 1115 D2).
    pub background: [u8; 4],
    /// Whether the page's scheme is dark: the bars' icons are light then.
    pub dark: bool,
}

/// What a software keyboard does to the page (`interactive-widget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Keyboard {
    /// The layout viewport ends at its top.
    #[default]
    Resizes,
    /// Nothing is laid out again.
    Overlays,
}

/// What the presenter keeps of the window.
#[derive(Default)]
pub(super) struct State {
    window: Option<Window>,
    /// The page's frame in the window as last fitted.
    frame: Option<[f32; 4]>,
    /// The keyboard's overlap with the frame the page would have without it.
    pub(super) keyboard: f32,
    /// The field to keep in view once the page has been painted in the
    /// viewport a keyboard left it ([`Presenter::reveal_owed`]).
    reveal: Option<ViewId>,
}

impl<D: DataSource> Presenter<D> {
    /// The shell's window and its insets, whenever either changes: the page
    /// is laid out in it again ([`Presenter::fit_window`]).
    pub fn set_window(&mut self, window: Window) -> Option<String> {
        let finite = |v: &[f32]| v.iter().all(|x| x.is_finite() && *x >= 0.0);
        if !(window.size.0 > 0.0 && window.size.1 > 0.0)
            || !finite(&[window.size.0, window.size.1])
            || ![window.bars, window.cutout, window.keyboard, window.gestures]
                .iter()
                .all(|side| finite(side))
        {
            return Some(format!("window refused: {window:?}"));
        }
        self.window.window = Some(window);
        self.fit_window()
    }

    /// The window the shell last told, if it has told one.
    pub fn window(&self) -> Option<&Window> {
        self.window.window.as_ref()
    }

    /// What the first root says of the window: whether the page covers it
    /// (`viewport-fit="cover"`), and what a keyboard does.
    fn window_policy(&self) -> (bool, Keyboard) {
        let kernel = self.host.kernel();
        let root = self.host.runner().roots().first().copied();
        let prop = |id| {
            root.and_then(|root| kernel.node(root))
                .and_then(|n| n.props.str(id))
        };
        let keyboard = match prop(PropId::InteractiveWidget) {
            Some("overlays-content") => Keyboard::Overlays,
            // `resizes-visual` (the page pans under a viewport that keeps
            // its size) needs a visual viewport this presenter does not
            // have: it resizes, which keeps the field in view the same.
            _ => Keyboard::Resizes,
        };
        (prop(PropId::ViewportFit) == Some("cover"), keyboard)
    }

    /// Lay the page out in the window the shell told, as the first root's
    /// `viewport-fit` and `interactive-widget` say now. The shell's host
    /// calls it before a paint as well, since a commit may have changed
    /// either; nothing happens when the page already fits, or on a host
    /// whose shell told no window.
    pub fn fit_window(&mut self) -> Option<String> {
        let window = self.window.window?;
        let (cover, policy) = self.window_policy();
        let safe: [f32; 4] = std::array::from_fn(|i| window.bars[i].max(window.cutout[i]));
        let (mut frame, mut env) = match cover {
            true => ([0.0, 0.0, window.size.0, window.size.1], safe),
            false => (
                [
                    safe[3],
                    safe[0],
                    (window.size.0 - safe[3] - safe[1]).max(1.0),
                    (window.size.1 - safe[0] - safe[2]).max(1.0),
                ],
                [0.0; 4],
            ),
        };
        // The keyboard's top edge in the window, and how much of the frame
        // it covers.
        let top = window.size.1 - window.keyboard[2];
        let keyboard = match window.keyboard[2] > 0.0 {
            true => (frame[1] + frame[3] - top).clamp(0.0, frame[3]),
            false => 0.0,
        };
        if keyboard > 0.0 && policy == Keyboard::Resizes {
            frame[3] = (frame[3] - keyboard).max(1.0);
            // The keyboard's edge has no inset of its own (the web's reading).
            env[2] = 0.0;
        }
        let rose = keyboard > self.window.keyboard;
        self.dirty |= self.window.frame != Some(frame) || self.window.keyboard != keyboard;
        self.window.frame = Some(frame);
        self.window.keyboard = keyboard;
        let mut error = None;
        if (frame[2], frame[3]) != self.viewport {
            error = self.resize(frame[2], frame[3]);
        }
        error = error.or(self.set_safe_area(env));
        // The shell's bars take their room of the page as it is now placed.
        error = error.or(self.fit_bars());
        // The field being edited stays above the keyboard. Where a picture
        // is shown, the boxes a reveal goes by are that picture's, painted
        // before the keyboard rose: the field is kept in view once the page
        // has been painted in its new viewport.
        if let Some(field) = self.focus.filter(|_| rose) {
            match self.display.attached() {
                true => self.window.reveal = Some(field),
                false => self.keep_in_view(field),
            }
        }
        error
    }

    /// After a picture is shown: the field a keyboard rose under is kept in
    /// view by that picture's boxes, the first that have the port the
    /// keyboard left.
    // Only a host that shows pictures calls it (`display_complete`).
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "android", test)),
        allow(dead_code)
    )]
    pub(super) fn reveal_owed(&mut self) {
        if let Some(field) = self.window.reveal.take() {
            self.keep_in_view(field);
        }
    }

    /// What the shell draws around the page; on a host that told no window,
    /// the viewport at the origin.
    pub fn chrome(&self) -> Chrome {
        let dark = self.dark();
        let kernel = self.host.kernel();
        let authored = self
            .host
            .runner()
            .roots()
            .first()
            .and_then(|root| kernel.node(*root))
            .and_then(|n| {
                // The root's own accent, as the painted root reads it.
                let _accent = crate::paint::accent_scope(&n);
                n.style
                    .background_color
                    .map(|c| crate::paint::rgba(c.resolve(dark)))
            })
            .filter(|c| c[3] == 255);
        Chrome {
            frame: self
                .window
                .frame
                .unwrap_or([0.0, 0.0, self.viewport.0, self.viewport.1]),
            background: authored.unwrap_or_else(|| crate::paint::page_color(dark)),
            dark,
        }
    }

    /// Whether a contact down at `x` (viewport points) starts in a side of
    /// the window the system's own gesture starts from: Back's, on a phone
    /// with gesture navigation. An authored swipe yields that edge to it
    /// (LLP 1115.001 D7.5; LLP 1057.001 precedence rule 1), by the width the
    /// system says it is: nothing on a phone with a button for Back, and
    /// nothing on a host whose shell told no window, where no system
    /// gesture starts from a side. (UIKit and the web yield a fixed width,
    /// `exact_motion::gesture::SWIPE_EDGE`.)
    pub(super) fn in_gesture_edge(&self, x: f32) -> bool {
        let Some(window) = self.window.window else {
            return false;
        };
        let at = self.window.frame.map_or(0.0, |frame| frame[0]) + x;
        at < window.gestures[3] || at >= window.size.0 - window.gestures[1]
    }

    /// `layout.env`'s insets (LLP 1012 §1): the four safe-area insets the
    /// kernel resolves `env()` with, and the keyboard's overlap.
    pub(super) fn env_insets(&self) -> [f32; 5] {
        let env = self.host.kernel().env();
        [
            env.top,
            env.right,
            env.bottom,
            env.left,
            self.window.keyboard,
        ]
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
