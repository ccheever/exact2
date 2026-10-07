//! The root font size (LLP 1069.000 D3): the host reads it from the
//! platform — Dynamic Type on iOS, the browser's root size on the web, 16
//! elsewhere — and the app may set its own over it, as `:root { font-size }`
//! sets one over the browser's setting. It is layout, not a fact: no
//! resource is asked again, and the commit only re-resolves `rem`/`em` and
//! what inherits the root's size.
use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, LayoutError};
use exact_plan::Value;

/// The app's `setRootFontSize` and the host's size beneath it, which the
/// host keeps setting while the app's stands and which `"medium"` restores.
#[derive(Clone, Copy, Default)]
pub(crate) struct RootFont {
    app: Option<f64>,
    host: f64,
}

/// What the kernel takes: finite and above 0 as the f32 it stores.
fn fits(px: f64) -> bool {
    let size = px as f32;
    size.is_finite() && size > 0.0
}

impl<D: DataSource> Runner<D> {
    /// The root font size in CSS pixels, as last set; 16 until a host says.
    pub fn root_font_size(&self) -> f64 {
        self.kernel.root_font_size() as f64
    }

    /// The host's own root font size, beneath any the app set: what a new
    /// runner of the same launch is told again.
    pub fn host_root_font_size(&self) -> f64 {
        match self.root_font.app {
            Some(_) => self.root_font.host,
            None => self.root_font_size(),
        }
    }

    /// Set the host's root font size and commit what it moves, in one
    /// commit; the same size again commits nothing, and so does any size
    /// while the app's `setRootFontSize` stands (it is kept for when the
    /// app hands the size back). A size that is not finite and positive is
    /// journaled and refused, changing nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> Result<Option<CommitReceipt>, RunnerError> {
        if !fits(px) {
            self.log(format!(
                "root font size {} refused",
                exact_num::Shortest(px)
            ));
            return Err(RunnerError::Kernel(LayoutError::InvalidRootFontSize.into()));
        }
        if self.root_font.app.is_some() {
            self.root_font.host = px;
            return Ok(None);
        }
        let previous = self.kernel.root_font_size();
        if !self
            .kernel
            .set_root_font_size(px as f32)
            .map_err(RunnerError::Kernel)?
        {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "root font size");
        if result.is_err() {
            // The kernel applies the size with the next commit; put back
            // the one the tree was laid out at.
            let _ = self.kernel.set_root_font_size(previous);
        }
        result.map(Some)
    }

    /// The app's `setRootFontSize(px)`, or `("medium")` for the host's
    /// size again, inside the commit of the action that called it: the
    /// kernel lays out at the size in that same commit. `false`, and a
    /// journal line, for anything else, which changes nothing.
    pub(super) fn app_root_font_size(&mut self, args: &[Value]) -> bool {
        let app = match args {
            [v] if v.as_str() == Some("medium") => None,
            [Value::Number(px)] if fits(*px) => Some(*px),
            _ => {
                let mut said = String::new();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        said.push_str(", ");
                    }
                    crate::agent::untyped_json(a, &mut said);
                }
                self.log(format!("setRootFontSize({said}) refused: the root font size is a number of px above 0, or \"medium\""));
                return false;
            }
        };
        if self.root_font.app.is_none() {
            self.root_font.host = self.root_font_size();
        }
        self.root_font.app = app;
        let px = app.unwrap_or(self.root_font.host);
        // Checked by `fits`, or the host's, which the kernel took before.
        let _ = self.kernel.set_root_font_size(px as f32);
        true
    }
}
