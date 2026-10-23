//! The root font size (LLP 1069.000 D3): the host reads it from the
//! platform — Dynamic Type on iOS, the browser's root size on the web, 16
//! times the font scale on Android, 16 elsewhere — and the app may set its
//! own over it, as `:root { font-size }` sets one over the browser's
//! setting. It is layout: the commit re-resolves `rem`/`em` and what
//! inherits the root's size, and asks no resource again, except the
//! viewport's when the host's size crosses the accessibility sizes
//! (`exactViewport().accessibilityTextSize`).
//! The display's scale is the other layout input a host reads from the
//! platform (LLP 1115.000 D10.3): a hairline is one device pixel of it.
//! A host's zoom (the Mac's View ▸ Zoom In, LLP 1021 §5.3) multiplies the
//! root size last, over the app's own as a browser's page zoom is over an
//! author's `html { font-size }`; `px` lengths stay.
use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, LayoutError};
use exact_plan::Value;

/// The app's `setRootFontSize` and the host's size beneath it, which the
/// host keeps setting while the app's stands and which `"medium"` restores.
#[derive(Clone, Copy)]
pub(crate) struct RootFont {
    app: Option<AppSize>,
    host: f64,
    /// The size the host last said, whatever the app's: what
    /// `exactViewport().accessibilityTextSize` reads.
    told: Option<f64>,
    /// The host's zoom over whichever size stands: 1 until a host says.
    zoom: f64,
}

impl Default for RootFont {
    fn default() -> Self {
        RootFont {
            app: None,
            host: 0.0,
            told: None,
            zoom: 1.0,
        }
    }
}

/// The app's size: px, which stands over the host's, or a percentage of
/// the host's (`"125%"`, CSS's percentage of `medium`), which follows it.
#[derive(Clone, Copy)]
enum AppSize {
    Px(f64),
    Of(f64),
}

impl AppSize {
    fn px(self, host: f64) -> f64 {
        match self {
            AppSize::Px(px) => px,
            AppSize::Of(ratio) => host * ratio,
        }
    }
}

/// What the kernel takes: finite and above 0 as the f32 it stores.
fn fits(px: f64) -> bool {
    let size = px as f32;
    size.is_finite() && size > 0.0
}

/// What Bold Text needs of a boot (LLP 1115.000 D12.3): the person's
/// setting, and which of the plan's font stacks are the system's (its first
/// member a generic family, not one the app names).
pub(super) fn bold_text_facts(
    kernel: &mut exact_kernel::Kernel,
    plan: &exact_plan::Plan,
    on: bool,
) {
    system_stacks(kernel, plan, &[]);
    kernel.set_bold_text(on);
}

/// Before a commit: a runtime font stack interned since the last says
/// whether it is the system's ([`system_stacks`]).
pub(super) fn runtime_stacks(
    kernel: &mut exact_kernel::Kernel,
    plan: &exact_plan::Plan,
    fonts: &crate::bridge::RuntimeFonts,
) {
    if plan.stacks.len() + fonts.names().len() != kernel.system_stack_count() {
        system_stacks(kernel, plan, fonts.names());
    }
}

/// Which font stacks are the system's ([`bold_text_facts`]): the plan's,
/// then the runtime lists a binding interned (`system-ui, sans-serif`), a
/// list whose first member is a generic family the author did not quote.
pub(super) fn system_stacks(
    kernel: &mut exact_kernel::Kernel,
    plan: &exact_plan::Plan,
    runtime: &[String],
) {
    const GENERICS: [&str; 8] = [
        "system-ui",
        "ui-sans-serif",
        "sans-serif",
        "ui-serif",
        "serif",
        "ui-monospace",
        "monospace",
        "ui-rounded",
    ];
    let compiled = plan.stacks.iter().map(|stack| {
        plan.stack_members[stack.members.start as usize..][..stack.members.len as usize]
            .first()
            .is_some_and(|m| m.kind != exact_plan::StackMemberKind::Family)
    });
    let runtime = runtime.iter().map(|list| {
        crate::bridge::runtime_family_members(list)
            .first()
            .is_some_and(|m| !m.quoted && GENERICS.contains(&m.name.to_ascii_lowercase().as_str()))
    });
    kernel.set_system_stacks(compiled.chain(runtime).collect());
}

impl<D: DataSource> Runner<D> {
    /// What a document commit tells the kernel beside its ops: the page's
    /// language and direction. And first, a runtime font stack interned
    /// since the last commit says whether it is the system's (LLP 1115.000
    /// D12.3, [`runtime_stacks`]).
    pub(super) fn document_facts(&mut self) -> (String, exact_kernel::Direction) {
        runtime_stacks(&mut self.kernel, &self.plan, &self.fonts.borrow());
        let direction = if self.direction() == "rtl" {
            exact_kernel::Direction::Rtl
        } else {
            exact_kernel::Direction::Ltr
        };
        (self.resolved_locale().to_owned(), direction)
    }

    /// The root font size in CSS pixels, as last set; 16 until a host says.
    pub fn root_font_size(&self) -> f64 {
        self.kernel.root_font_size() as f64
    }

    /// The host's own root font size, beneath any the app set: what a new
    /// runner of the same launch is told again.
    pub fn host_root_font_size(&self) -> f64 {
        // Zoomed, the host's size as told (the kernel's is f32, times the zoom).
        if self.root_font.app.is_some() || self.root_font.zoom != 1.0 {
            self.root_font.host
        } else {
            self.root_font_size()
        }
    }

    /// The host's zoom over the root size, as last set; 1 until a host says.
    pub fn root_font_zoom(&self) -> f64 {
        self.root_font.zoom
    }

    /// Set the host's zoom (LLP 1021 §5.3: the Mac's View ▸ Actual Size,
    /// Zoom In and Zoom Out): the root size, the app's own included, times
    /// `factor`, in one commit; the same zoom commits nothing. A factor, or
    /// a size it makes, that is not finite and positive is journaled and
    /// refused, changing nothing.
    pub fn set_root_font_zoom(
        &mut self,
        factor: f64,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let host = self.host_root_font_size();
        let base = self.root_font.app.map_or(host, |a| a.px(host));
        if !fits(factor) || !fits(base * factor) {
            self.log(format!(
                "root font zoom {} refused",
                exact_num::Shortest(factor)
            ));
            return Err(RunnerError::Kernel(LayoutError::InvalidRootFontSize.into()));
        }
        if factor == self.root_font.zoom {
            return Ok(None);
        }
        let (previous, zoom, told) = (
            self.kernel.root_font_size(),
            self.root_font.zoom,
            self.root_font.host,
        );
        let changed = self
            .kernel
            .set_root_font_size((base * factor) as f32)
            .map_err(RunnerError::Kernel)?;
        self.root_font.host = host;
        self.root_font.zoom = factor;
        if !changed {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "root font zoom");
        if result.is_err() {
            let _ = self.kernel.set_root_font_size(previous);
            self.root_font.zoom = zoom;
            self.root_font.host = told;
        }
        result.map(Some)
    }

    /// Set the host's root font size and commit what it moves, in one
    /// commit; the same size again commits nothing, and so does any size
    /// while the app's `setRootFontSize(px)` stands (it is kept for when the
    /// app hands the size back). Under the app's percentage the root is
    /// that share of the new size. A size that is not finite and positive,
    /// or whose share is not, is journaled and refused, changing nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> Result<Option<CommitReceipt>, RunnerError> {
        let root = match self.root_font.app {
            Some(app) => app.px(px),
            None => px,
        } * self.root_font.zoom;
        if !fits(px) || !fits(root) {
            self.log(format!(
                "root font size {} refused",
                exact_num::Shortest(px)
            ));
            return Err(RunnerError::Kernel(LayoutError::InvalidRootFontSize.into()));
        }
        // A step across the accessibility sizes re-answers the viewport's
        // readers in the same commit: the fact before this size is told.
        let told = self.root_font.told;
        let was = self.accessibility_text_size();
        let host = self.root_font.host;
        self.root_font.host = px;
        self.root_font.told = Some(px);
        let which: Vec<usize> = if was == self.accessibility_text_size() {
            Vec::new()
        } else {
            (0..self.plan.resources.len())
                .filter(|i| {
                    self.plan.str(self.plan.resources[*i].source) == crate::viewport::SOURCE
                })
                .collect()
        };
        if let Some(AppSize::Px(_)) = self.root_font.app {
            let result = self.recommit(which, "root font size");
            if result.is_err() {
                self.root_font.host = host;
                self.root_font.told = told;
            }
            return result;
        }
        let px = root;
        let previous = self.kernel.root_font_size();
        if !self
            .kernel
            .set_root_font_size(px as f32)
            .map_err(RunnerError::Kernel)?
        {
            // The layout's size already (one primed before boot): the fact
            // may still have stepped.
            let result = self.recommit(which, "root font size");
            if result.is_err() {
                self.root_font.host = host;
                self.root_font.told = told;
            }
            return result;
        }
        let result = self.commit_again(which, "root font size");
        if result.is_err() {
            // The kernel applies the size with the next commit; put back
            // the one the tree was laid out at.
            let _ = self.kernel.set_root_font_size(previous);
            self.root_font.host = host;
            self.root_font.told = told;
        }
        result.map(Some)
    }

    /// Whether the person's text size is one of the accessibility sizes, as
    /// UIKit's `isAccessibilityCategory` says (AX1 to AX5, a body of 28 or
    /// more): only where the platform's own views change at them, in the
    /// UIKit column (iOS, and tvOS, visionOS and watchOS, which share it); false
    /// elsewhere, and on the web, which has no such fact. The host's size
    /// as last told, or primed before boot.
    pub(super) fn accessibility_text_size(&self) -> bool {
        use exact_kernel::style::metrics::{column, Column};
        let host = self
            .root_font
            .told
            .unwrap_or_else(|| self.host_root_font_size());
        column() == Column::Ios && host >= 28.0
    }

    /// Say whether Bold Text is on (LLP 1115.000 D12.3): the person's
    /// setting, which weights the system font (an authored weight
    /// included) and leaves a family the app ships alone. A change commits
    /// every text again, at the weight it now means; the same setting
    /// commits nothing.
    pub fn set_bold_text(&mut self, on: bool) -> Result<Option<CommitReceipt>, RunnerError> {
        if !self.kernel.set_bold_text(on) {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "bold text");
        if result.is_err() {
            let _ = self.kernel.set_bold_text(!on);
        }
        result.map(Some)
    }

    /// The display's scale, device pixels a px, as last set; 1 until a host
    /// says (LLP 1115.000 D10.3).
    pub fn display_scale(&self) -> f64 {
        self.kernel.display_scale() as f64
    }

    /// Set the display's scale, which a hairline (`-exact-hairline`) is one
    /// device pixel of: the host's fact, told as the root font size is and
    /// again when the window moves to another display. Hairlines and borders
    /// snapped by the host relayout in one commit. Without those, or for the
    /// same scale, it commits nothing; future lengths resolve against it.
    /// A scale that is not finite and positive is journaled and refused,
    /// changing nothing.
    pub fn set_display_scale(&mut self, scale: f64) -> Result<Option<CommitReceipt>, RunnerError> {
        if !fits(scale) {
            self.log(format!(
                "display scale {} refused",
                exact_num::Shortest(scale)
            ));
            return Err(RunnerError::Kernel(LayoutError::InvalidDisplayScale.into()));
        }
        let previous = self.kernel.display_scale();
        let changed = self
            .kernel
            .set_display_scale(scale as f32)
            .map_err(RunnerError::Kernel)?;
        if !changed || !self.kernel.has_display_lengths() {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "display scale");
        if result.is_err() {
            let _ = self.kernel.set_display_scale(previous);
        }
        result.map(Some)
    }

    /// The app's own size set aside for the host's (a development host
    /// holding the root size, LLP 1122 T7): the kernel takes the host's in
    /// the next commit, and the app's latest is applied again when handed
    /// back.
    pub(super) fn drop_app_root(&mut self) -> bool {
        if self.root_font.app.take().is_none() {
            return false;
        }
        let _ = self
            .kernel
            .set_root_font_size((self.root_font.host * self.root_font.zoom) as f32);
        true
    }

    /// Whether the app's `setRootFontSize(v)` would be taken: px above 0,
    /// a percentage of the host's size above 0, or `"medium"`.
    pub(super) fn root_font_size_valid(&self, v: &Value) -> bool {
        let host = self.host_root_font_size();
        match v {
            v if v.as_str() == Some("medium") => true,
            Value::Number(px) => fits(*px),
            v => v
                .as_str()
                .and_then(exact_num::css_percent)
                .is_some_and(|r| fits(host * r)),
        }
    }

    /// The app's `setRootFontSize(px)`, `("125%")` for a share of the
    /// host's size that follows it, or `("medium")` for the host's size
    /// again, inside the commit of the action that called it: the kernel
    /// lays out at the size in that same commit. `false`, and a journal
    /// line, for anything else, which changes nothing.
    pub(super) fn app_root_font_size(&mut self, args: &[Value]) -> bool {
        let host = self.host_root_font_size();
        let app = match args {
            [v] if v.as_str() == Some("medium") => None,
            [Value::Number(px)] if fits(*px) => Some(AppSize::Px(*px)),
            [v] if v
                .as_str()
                .and_then(exact_num::css_percent)
                .is_some_and(|r| fits(host * r)) =>
            {
                v.as_str().and_then(exact_num::css_percent).map(AppSize::Of)
            }
            _ => {
                let mut said = String::new();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        said.push_str(", ");
                    }
                    crate::agent::untyped_json(a, &mut said);
                }
                self.log(format!("setRootFontSize({said}) refused: the root font size is a number of px above 0, a percentage of the host's above 0 (\"125%\"), or \"medium\""));
                return false;
            }
        };
        self.root_font.host = host;
        self.root_font.app = app;
        let px = app.map_or(host, |a| a.px(host)) * self.root_font.zoom;
        // Checked by `fits`, or the host's, which the kernel took before; a
        // zoomed size the kernel refuses leaves the size as it was.
        let _ = self.kernel.set_root_font_size(px as f32);
        true
    }
}
