//! The display's scale as a layout input (LLP 1115.000 D10.3): the page's
//! `devicePixelRatio`, which a hairline (`-exact-hairline`) is one device
//! pixel of. The glue says it after boot and when the page's facts change,
//! beside the root font size.

use super::*;

impl<D: DataSource> Host<D> {
    /// Tell the runner the display's scale: a relayout in one batch where a
    /// row is a hairline, nothing where none is.
    pub fn set_display_scale(&mut self, scale: f64) -> String {
        let (receipts, error) = match self.runner.set_display_scale(scale) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("display scale: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }
}
