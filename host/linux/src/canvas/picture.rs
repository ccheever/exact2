//! What a reader copies a picture from: a decoded image, or a canvas's
//! rasterized drawing (Canvas 2D, LLP 1056), both premultiplied RGBA.
use crate::image::Bitmap;
use std::sync::{Arc, Weak};
use tiny_skia::Pixmap;

/// A picture the reader fetches by id ([`super::CanvasHost::image`]).
#[derive(Clone)]
pub enum Picture {
    /// A decoded image.
    Bitmap(Arc<Bitmap>),
    /// A canvas's pixels as last drawn.
    Canvas(Arc<Pixmap>),
}

impl Picture {
    /// Pixels across.
    pub fn width(&self) -> u32 {
        match self {
            Picture::Bitmap(b) => b.width(),
            Picture::Canvas(p) => p.width(),
        }
    }

    /// Pixels down.
    pub fn height(&self) -> u32 {
        match self {
            Picture::Bitmap(b) => b.height(),
            Picture::Canvas(p) => p.height(),
        }
    }

    /// The allocation it is known by.
    pub(super) fn key(&self) -> usize {
        match self {
            Picture::Bitmap(b) => Arc::as_ptr(b) as usize,
            Picture::Canvas(p) => Arc::as_ptr(p) as usize,
        }
    }

    pub(super) fn downgrade(&self) -> WeakPicture {
        match self {
            Picture::Bitmap(b) => WeakPicture::Bitmap(Arc::downgrade(b)),
            Picture::Canvas(p) => WeakPicture::Canvas(Arc::downgrade(p)),
        }
    }
}

impl AsRef<[u8]> for Picture {
    fn as_ref(&self) -> &[u8] {
        match self {
            Picture::Bitmap(b) => b.as_ref().as_ref(),
            Picture::Canvas(p) => p.data(),
        }
    }
}

/// A picture the recorder announced, held weakly: dropped by the presenter,
/// it is freed on the reader too.
pub(super) enum WeakPicture {
    Bitmap(Weak<Bitmap>),
    Canvas(Weak<Pixmap>),
}

impl WeakPicture {
    pub(super) fn alive(&self) -> bool {
        match self {
            WeakPicture::Bitmap(w) => w.strong_count() > 0,
            WeakPicture::Canvas(w) => w.strong_count() > 0,
        }
    }

    /// Whether this is `picture`'s allocation, still alive.
    pub(super) fn is(&self, picture: &Picture) -> bool {
        match (self, picture) {
            (WeakPicture::Bitmap(w), Picture::Bitmap(b)) => {
                w.upgrade().is_some_and(|live| Arc::ptr_eq(&live, b))
            }
            (WeakPicture::Canvas(w), Picture::Canvas(p)) => {
                w.upgrade().is_some_and(|live| Arc::ptr_eq(&live, p))
            }
            _ => false,
        }
    }
}
