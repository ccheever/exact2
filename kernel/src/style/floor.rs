//! A size or padding lowered to the engine clamped at zero, for a row with no
//! negative values (CSS Values 4 §9.1): a math function's result clamps there.

use super::{Dimension, Env};
use taffy::prelude::{length, percent};

impl Dimension {
    /// The engine value for a [`super::calc::nonnegative_row`] size, clamped at zero.
    pub(crate) fn floor_taffy(self, env: &Env) -> taffy::style::Dimension {
        self.floor_lpa(env).into()
    }

    /// [`Self::floor_taffy`] for a minimum or maximum size.
    pub(crate) fn floor_lpa(self, env: &Env) -> taffy::style::LengthPercentageAuto {
        match self.resolve(env) {
            Dimension::Points(v) => length(v.max(0.0)),
            Dimension::Percent(p) => percent(p.max(0.0) / 100.0),
            Dimension::Calc(p, v) => {
                taffy::style::LengthPercentageAuto::calc(super::calc_handle_in(p, v, true, 0.0))
            }
            Dimension::Compare(c) => {
                taffy::style::LengthPercentageAuto::calc(super::tree_handle(c, true, 0.0))
            }
            _ => self.to_lpa(env),
        }
    }

    /// [`Self::floor_taffy`] for a padding, plus `added` points after the
    /// clamp.
    pub(crate) fn floor_lp(self, env: &Env, added: f32) -> taffy::style::LengthPercentage {
        match self.resolve(env) {
            Dimension::Points(v) => length(v.max(0.0) + added),
            Dimension::Percent(p) if added == 0.0 => percent(p.max(0.0) / 100.0),
            Dimension::Percent(p) => taffy::style::LengthPercentage::calc(super::calc_handle_in(
                p.max(0.0),
                0.0,
                false,
                added,
            )),
            Dimension::Calc(p, v) => {
                taffy::style::LengthPercentage::calc(super::calc_handle_in(p, v, true, added))
            }
            Dimension::Compare(c) => {
                taffy::style::LengthPercentage::calc(super::tree_handle(c, true, added))
            }
            _ => length(added),
        }
    }
}
