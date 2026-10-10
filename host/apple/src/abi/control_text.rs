//! The UIKit control font/chrome callbacks, kept per runtime.
use super::*;
impl<D: DataSource> Bridge<D> {
    /// Register the synchronous plan-font hook used by subsequent boots,
    /// with the context it is handed back.
    pub fn set_fonts(&mut self, fonts: Option<FontsFn>, ctx: *mut c_void) {
        self.fonts = fonts;
        self.fonts_ctx = ctx;
    }

    /// Install the optional control hooks before boot; other presenters leave both absent.
    pub fn set_control_text(
        &mut self,
        text: Option<crate::control_text::ControlTextFn>,
        chrome: Option<crate::control_text::FieldChromeFn>,
    ) {
        self.control_text = text;
        self.field_chrome = chrome;
    }
    /// Install button measurement before boot; absent leaves AppKit unchanged.
    pub fn set_button_measure(&mut self, measure: Option<crate::control_text::ButtonMeasureFn>) {
        self.button_measure = measure;
    }
    /// Measuring traits (scale, content size, legibility, appearance) changed;
    /// preserve the safe area and other environment facts.
    pub fn control_text_changed(&mut self, hooks: Hooks) -> u32 {
        if let Some(revision) = &self.measure_revision {
            revision.set(revision.get().wrapping_add(1));
        }
        let styles = self
            .control_text
            .map(|f| crate::control_text::text_styles(f, hooks.ctx));
        let out = match (self.host.as_mut(), styles) {
            (Some(host), Some(styles)) => host.set_control_fonts(
                styles,
                self.control_text
                    .map(|f| crate::control_text::button_fonts(f, hooks.ctx)),
            ),
            _ => not_booted(),
        };
        self.emit(out)
    }
    /// The window's size, which every viewport unit resolves against
    /// everywhere; a nonpositive size clears it (LLP 1075.003 §9.11).
    pub fn screen(&mut self, width: f32, height: f32) -> u32 {
        let screen = (width > 0.0 && height > 0.0).then_some((width, height));
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_screen(screen));
        self.emit(out)
    }
    /// The safe-area insets changed.
    pub fn insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_insets(top, right, bottom, left));
        self.emit(out)
    }
}

/// Invalid environment answers refuse the candidate rather than silently inheriting the page.
pub(super) fn prepare<D: DataSource>(
    runner: &mut exact_runner::Runner<D>,
    callback: Option<crate::control_text::ControlTextFn>,
    ctx: *mut c_void,
) -> Result<(), crate::HostError> {
    if let Some(callback) = callback {
        let mut env = runner.kernel().env();
        env.control_text_styles = Some(crate::control_text::text_styles(callback, ctx));
        env.button_fonts = Some(crate::control_text::button_fonts(callback, ctx));
        runner
            .kernel_mut()
            .set_env(env)
            .map_err(|e| crate::HostError::Layout(format!("control text: {e:?}")))?;
    }
    Ok(())
}
