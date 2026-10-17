//! The root font size (LLP 1069.000 D3): the host reads it from the
//! platform — Dynamic Type on iOS, the browser's root size on the web, 16
//! times the font scale on Android, 16 elsewhere — and the app may set its
//! own over it, as `:root { font-size }` sets one over the browser's
//! setting. It is layout, not a fact: no resource is asked again, and the
//! commit only re-resolves `rem`/`em` and what inherits the root's size.
//! The display's scale is the other layout input a host reads from the
//! platform (LLP 1115.000 D10.3): a hairline is one device pixel of it.
use super::{DataSource, Runner, RunnerError};
use exact_kernel::{CommitReceipt, LayoutError};
use exact_plan::Value;

/// The app's `setRootFontSize` and the host's size beneath it, which the
/// host keeps setting while the app's stands and which `"medium"` restores.
#[derive(Clone, Copy, Default)]
pub(crate) struct RootFont {
    app: Option<AppSize>,
    host: f64,
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
    /// while the app's `setRootFontSize(px)` stands (it is kept for when the
    /// app hands the size back). Under the app's percentage the root is
    /// that share of the new size. A size that is not finite and positive,
    /// or whose share is not, is journaled and refused, changing nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> Result<Option<CommitReceipt>, RunnerError> {
        let root = match self.root_font.app {
            Some(app) => app.px(px),
            None => px,
        };
        if !fits(px) || !fits(root) {
            self.log(format!(
                "root font size {} refused",
                exact_num::Shortest(px)
            ));
            return Err(RunnerError::Kernel(LayoutError::InvalidRootFontSize.into()));
        }
        let host = self.root_font.host;
        if self.root_font.app.is_some() {
            self.root_font.host = px;
        }
        if let Some(AppSize::Px(_)) = self.root_font.app {
            return Ok(None);
        }
        let px = root;
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
            self.root_font.host = host;
        }
        result.map(Some)
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
    /// again when the window moves to another display. Where a row is a
    /// hairline it commits what that moves, in one commit; where none is,
    /// or it is the scale already set, it commits nothing and the next
    /// hairline written resolves against it. A scale that is not finite and
    /// positive is journaled and refused, changing nothing.
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
        if !changed || !self.kernel.has_hairlines() {
            return Ok(None);
        }
        let result = self.commit_again(Vec::new(), "display scale");
        if result.is_err() {
            let _ = self.kernel.set_display_scale(previous);
        }
        result.map(Some)
    }

    /// The app's `setRootFontSize(px)`, `("125%")` for a share of the
    /// host's size that follows it, or `("medium")` for the host's size
    /// again, inside the commit of the action that called it: the kernel
    /// lays out at the size in that same commit. `false`, and a journal
    /// line, for anything else, which changes nothing.
    pub(super) fn app_root_font_size(&mut self, args: &[Value]) -> bool {
        let host = match self.root_font.app {
            Some(_) => self.root_font.host,
            None => self.root_font_size(),
        };
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
        let px = app.map_or(host, |a| a.px(host));
        // Checked by `fits`, or the host's, which the kernel took before.
        let _ = self.kernel.set_root_font_size(px as f32);
        true
    }
}
