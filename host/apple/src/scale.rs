//! The display's scale as a layout input (LLP 1115.000 D10.3): UIKit's
//! `displayScale`, AppKit's `backingScaleFactor`, which a hairline
//! (`-exact-hairline`) is one device pixel of. The presenter says it after
//! boot and when its view moves to another display, beside the page's
//! facts and the root font size.
use super::*;

/// A kernel for a session of this host, after the host has said which
/// platform's looks it shows (LLP 1115.002 D6): the looks Exact draws itself
/// are rows of the kernel's schema with a column a platform, and a plan's
/// rows read theirs as they are set. The Mac's are its own; iOS's column is
/// tvOS's, visionOS's and watchOS's too. Said for the process, before any plan boots.
pub(crate) fn kernel(measurer: Box<dyn TextMeasurer>) -> Kernel {
    use exact_kernel::style::metrics::{set_column, Column};
    set_column(if cfg!(target_os = "macos") {
        Column::Mac
    } else {
        Column::Ios
    });
    Kernel::new(measurer)
}

impl<D: DataSource> Host<D> {
    /// The root font size `rem` follows (LLP 1069.000 D3): a relayout in
    /// one commit, no resource asked again; the same size commits nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> String {
        match self.runner.set_root_font_size(px) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("root font size: {e:?}"))),
        }
    }

    /// Tell the runner the display's scale: a relayout in one commit where a
    /// row is a hairline, nothing where none is.
    pub fn set_display_scale(&mut self, scale: f64) -> String {
        match self.runner.set_display_scale(scale) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("display scale: {e:?}"))),
        }
    }
}

/// Explicit inputs for a headless boot. Presented sessions that tell these
/// after boot retain their existing first-layout defaults (LLP 1126 D8).
#[derive(Clone, Copy, Default)]
pub(crate) struct LaunchLayout {
    pub root_font_size: Option<f32>,
    pub display_scale: Option<f32>,
}
impl LaunchLayout {
    pub const NONE: Self = Self {
        root_font_size: None,
        display_scale: None,
    };
    pub fn prepare(self, kernel: &mut Kernel) -> Result<(), HostError> {
        if let Some(size) = self.root_font_size {
            kernel
                .set_root_font_size(size)
                .map_err(|e| HostError::Runner(RunnerError::Kernel(e)))?;
        }
        if let Some(scale) = self.display_scale {
            kernel
                .set_display_scale(scale)
                .map_err(|e| HostError::Runner(RunnerError::Kernel(e)))?;
        }
        Ok(())
    }
}
