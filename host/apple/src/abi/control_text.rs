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
    /// Dynamic Type/legibility changed; preserve the safe area and other environment facts.
    pub fn control_text_changed(&mut self, hooks: Hooks) -> u32 {
        let styles = self
            .control_text
            .map(|f| crate::control_text::text_styles(f, hooks.ctx));
        let out = match (self.host.as_mut(), styles) {
            (Some(host), Some(styles)) => host.set_control_text_styles(styles),
            _ => not_booted(),
        };
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
        runner
            .kernel_mut()
            .set_env(env)
            .map_err(|e| crate::HostError::Layout(format!("control text: {e:?}")))?;
    }
    Ok(())
}
