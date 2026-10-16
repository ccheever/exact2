//! The page's environment as the host sets it (LLP 1001 §2; LLP 1078 D4):
//! the safe-area insets and the viewport segments a fold makes, each a call
//! that re-sends the dictionaries of the nodes whose style reads the
//! environment and lays out again.

use super::{Batch, Host, Timed};
use exact_kernel::Env;
use exact_runner::DataSource;

#[cfg(test)]
#[path = "regions_tests.rs"]
mod regions_tests;

impl<D: DataSource> Host<D> {
    /// The safe-area insets changed (a boot under `viewport-fit=cover`, a
    /// rotation): the kernel's environment is set, every node whose style
    /// holds an `env()` length gets its dictionary re-sent with the new
    /// points and is laid out again; the batch carries what moved. Empty
    /// when nothing reads the insets, or they did not change.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> String {
        let env = self
            .runner
            .kernel()
            .env()
            .with_insets(top, right, bottom, left);
        self.set_environment(env)
    }

    /// The layout regions' insets (LLP 1123 D10), five regions of four
    /// edges each, top right bottom left, in [`exact_kernel::style::Region`]'s
    /// order; `None`: every region is the safe area. As for the insets, every
    /// node whose style reads the environment is re-sent and laid out again.
    pub fn set_regions(&mut self, regions: Option<[[f32; 4]; 5]>) -> String {
        let env = self.runner.kernel().env().with_regions(regions);
        self.set_environment(env)
    }

    /// The layout regions from the ABI's flat list (`exact_regions`): twenty
    /// values, region by region, each top right bottom left; anything else
    /// is none, when every region is the safe area.
    pub fn regions_from(insets: &[f32]) -> Option<[[f32; 4]; 5]> {
        (insets.len() == 20)
            .then(|| std::array::from_fn(|r| std::array::from_fn(|e| insets[r * 4 + e])))
    }

    /// The window's own size, whatever is presented in it (LLP 1075.003
    /// §9.11): what every viewport unit (`vw`, `vh`, `vmin`, `vmax` and kin) resolves
    /// against everywhere, root and every sheet, never a sheet's viewport,
    /// so no length follows a sheet's height. `None`: the layout viewport.
    pub fn set_screen(&mut self, screen: Option<(f32, f32)>) -> String {
        let env = self.runner.kernel().env().with_screen(screen);
        self.set_environment(env)
    }

    pub(super) fn set_environment(&mut self, env: Env) -> String {
        let mut batch = Batch::new();
        let error = match self.environment_into(env, &mut batch) {
            Ok(false) => None,
            Ok(true) => self.layout(&mut batch).err(),
            Err(e) => Some(e),
        };
        self.finish(batch, error)
    }

    /// `env` set, and when it changed, what reads it re-sent into `batch`,
    /// not yet laid out.
    fn environment_into(&mut self, env: Env, batch: &mut Batch) -> Result<bool, String> {
        let changed = self
            .runner
            .kernel_mut()
            .set_env(env)
            .map_err(|e| format!("insets: {e:?}"))?;
        if changed {
            self.height_targets_dirty = true;
            for id in self.preorder() {
                self.update(id, batch);
            }
            self.emit_paragraphs(batch);
        }
        Ok(changed)
    }

    /// What changed of the view at once, each part only when given: the
    /// window's size (`None` in it clears it), the safe-area insets, the
    /// layout regions and the viewport, in one batch under one layout, as
    /// the calls one by one would leave them. An iOS boot's view takes the
    /// whole window as its tab bar comes (LLP 1123 D1), which changes all
    /// four (`ExactView.fit`).
    pub fn set_view(
        &mut self,
        screen: Option<Option<(f32, f32)>>,
        insets: Option<[f32; 4]>,
        regions: Option<Option<[[f32; 4]; 5]>>,
        size: Option<(f32, f32)>,
    ) -> String {
        let mut env = self.runner.kernel().env();
        if let Some(screen) = screen {
            env = env.with_screen(screen);
        }
        if let Some([top, right, bottom, left]) = insets {
            env = env.with_insets(top, right, bottom, left);
        }
        if let Some(regions) = regions {
            env = env.with_regions(regions);
        }
        let mut batch = Batch::new();
        let changed = self.environment_into(env, &mut batch);
        let error = changed.as_ref().err().cloned();
        match size {
            Some((width, height)) => self.resize_inner(width, height, batch, error),
            None if changed == Ok(true) => {
                let laid_out = self.layout(&mut batch).err();
                self.finish(batch, laid_out)
            }
            None => self.finish(batch, error),
        }
    }

    /// The device's posture and the viewport segments a fold makes (LLP
    /// 1078 D4): the kernel's grid and `exactViewport`'s three fields in one
    /// batch — the nodes whose style reads the environment get their
    /// dictionaries re-sent and are laid out again, as for the insets, and
    /// every reader of the fields recommits. Empty when nothing changed. A
    /// malformed grid (the kernel's refusal, or a count past 255) is an
    /// error on the batch and changes nothing.
    pub fn set_segments(
        &mut self,
        posture: u32,
        cols: u32,
        rows: u32,
        rects: Vec<exact_kernel::Rect>,
    ) -> String {
        let (Ok(c), Ok(r)) = (u8::try_from(cols), u8::try_from(rows)) else {
            return self.finish(
                Batch::new(),
                Some(format!("segments: {cols}x{rows} is past the kernel's grid")),
            );
        };
        let mut batch = Batch::new();
        let restyled = match self.runner.kernel_mut().set_segments(c, r, rects) {
            Ok(changed) => changed,
            Err(e) => return self.finish(batch, Some(format!("segments: {e:?}"))),
        };
        if restyled {
            self.height_targets_dirty = true;
            for id in self.preorder() {
                self.update(id, &mut batch);
            }
            self.emit_paragraphs(&mut batch);
        }
        let fold = exact_runner::Fold {
            posture: exact_runner::Posture::from_bits(posture),
            cols,
            rows,
        };
        match self.runner.set_fold(fold) {
            Ok(Some(receipt)) => self.commit_into(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
                batch,
            ),
            Ok(None) if restyled => {
                let error = self.layout(&mut batch).err();
                self.finish(batch, error)
            }
            Ok(None) => self.finish(batch, None),
            Err(e) => self.finish(batch, Some(format!("segments: {e:?}"))),
        }
    }
}

impl<D: exact_runner::DataSource> crate::Host<D> {
    /// Change only the control text part of the existing environment and relayout.
    pub fn set_control_text_styles(&mut self, styles: exact_kernel::ControlTextStyles) -> String {
        let fonts = self.runner.kernel().env().button_fonts;
        self.measuring_traits_changed(Some((styles, fonts)))
    }
    /// The traits text and symbols are measured under changed (a scale, a
    /// content size, legibility, an appearance), with the control fonts they
    /// give when the host has them: everything is laid out again whether or
    /// not a control reads those fonts, and the auto heights running
    /// transitions target are measured again.
    pub(crate) fn measuring_traits_changed(
        &mut self,
        fonts: Option<(
            exact_kernel::ControlTextStyles,
            Option<exact_kernel::ButtonFonts>,
        )>,
    ) -> String {
        let mut env = self.runner.kernel().env().clone();
        if let Some((styles, button_fonts)) = fonts {
            env.control_text_styles = Some(styles);
            env.button_fonts = button_fonts;
        }
        self.height_targets_dirty = true;
        let mut batch = Batch::new();
        let error = match self.runner.kernel_mut().set_env(env) {
            Ok(changed) => {
                if changed {
                    for id in self.preorder() {
                        self.update(id, &mut batch);
                    }
                    self.emit_paragraphs(&mut batch);
                }
                self.layout(&mut batch).err()
            }
            Err(e) => Some(format!("control fonts: {e:?}")),
        };
        self.finish(batch, error)
    }
}
