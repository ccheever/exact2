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
/// tvOS's and visionOS's too. Said for the process, before any plan boots.
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
