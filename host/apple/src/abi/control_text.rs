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
        let fonts = self.control_text.map(|f| {
            (
                crate::control_text::text_styles(f, hooks.ctx),
                Some(crate::control_text::button_fonts(f, hooks.ctx)),
            )
        });
        let out = match self.host.as_mut() {
            Some(host) => host.measuring_traits_changed(fonts),
            None => not_booted(),
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
    /// What changed of the view at once (`exact_view`), from the input
    /// buffer's first `len` bytes, little-endian words: a mask of what is
    /// given (1 the window's size, 2 the safe-area insets, 4 the layout
    /// regions, 8 the viewport), the window's width and height (nonpositive
    /// clears it), the insets top, right, bottom and left, the viewport's
    /// width and height, the regions' count (20, or 0 when every region is
    /// the safe area) and their insets.
    pub fn view(&mut self, len: usize) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let word = |i: usize| {
            bytes
                .get(i * 4..i * 4 + 4)
                .map(|w| [w[0], w[1], w[2], w[3]])
        };
        let float = |i: usize| word(i).map(f32::from_le_bytes).unwrap_or(0.0);
        let (mask, count) = (
            word(0).map_or(0, u32::from_le_bytes),
            word(9).map_or(0, u32::from_le_bytes) as usize,
        );
        if bytes.len() != (10 + count) * 4 || (count != 0 && count != 20) {
            let error =
                "{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"view: malformed\"}";
            return self.emit(error.to_string());
        }
        let given = |bit: u32| mask & bit != 0;
        let (width, height) = (float(1), float(2));
        let screen = given(1).then(|| (width > 0.0 && height > 0.0).then_some((width, height)));
        let insets = given(2).then(|| [float(3), float(4), float(5), float(6)]);
        let size = given(8).then(|| (float(7), float(8)));
        let values: Vec<f32> = (10..10 + count).map(float).collect();
        let regions = given(4).then(|| crate::Host::<D>::regions_from(&values));
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_view(screen, insets, regions, size));
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
    /// The layout regions' insets changed (LLP 1123 D10): twenty values,
    /// or none, when every region is the safe area.
    pub fn regions(&mut self, insets: &[f32]) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.set_regions(crate::Host::<D>::regions_from(insets))
        });
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
