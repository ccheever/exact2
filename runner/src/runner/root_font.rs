//! The root font size the host reads from the platform (LLP 1069.000 D3):
//! Dynamic Type on iOS, the browser's root size on the web, 16 elsewhere.
//! It is layout, not a fact: no resource is asked again, and the commit
//! only re-resolves `rem`/`em` and what inherits the root's size.
use super::{DataSource, Runner, RunnerError};
use exact_kernel::CommitReceipt;

impl<D: DataSource> Runner<D> {
    /// The root font size in CSS pixels, as last set; 16 until a host says.
    pub fn root_font_size(&self) -> f64 {
        self.kernel.root_font_size() as f64
    }

    /// Set the root font size and commit what it moves, in one commit; the
    /// same size again commits nothing. A size that is not finite and
    /// positive is journaled and refused, changing nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> Result<Option<CommitReceipt>, RunnerError> {
        let previous = self.kernel.root_font_size();
        match self.kernel.set_root_font_size(px as f32) {
            Ok(false) => return Ok(None),
            Ok(true) => {}
            Err(error) => {
                self.log(format!(
                    "root font size {} refused",
                    exact_num::Shortest(px)
                ));
                return Err(RunnerError::Kernel(error));
            }
        }
        let result = self.commit_again(Vec::new(), "root font size");
        if result.is_err() {
            // The kernel applies the size with the next commit; put back
            // the one the tree was laid out at.
            let _ = self.kernel.set_root_font_size(previous);
        }
        result.map(Some)
    }
}
