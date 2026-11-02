//! Web Audio distance models, saved with a voice or attached to an AudioSource.
use crate::{Component, Data};

/// Web Audio's distanceModel vocabulary.
#[derive(Data, Default, Clone, Copy, Debug, PartialEq)]
pub enum DistanceModel {
    /// Inverse distance; the default, with no arbitrary cutoff.
    #[default]
    Inverse,
    /// Linear attenuation between ref_distance and max_distance.
    Linear,
    /// Distance raised to minus rolloff_factor.
    Exponential,
}

/// Distance attenuation in world units. Defaults match Web Audio's PannerNode.
/// Attach to an AudioSource entity or supply to `Play::spatial` for one voice.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    /// Attenuation equation.
    pub distance_model: DistanceModel,
    /// Distance before attenuation begins; nonnegative, default 1.
    pub ref_distance: f32,
    /// Linear model's maximum distance; positive, default 10000.
    pub max_distance: f32,
    /// Nonnegative attenuation strength, default 1. Zero disables attenuation.
    pub rolloff_factor: f32,
}
impl Default for Spatial {
    fn default() -> Self {
        Self {
            distance_model: DistanceModel::Inverse,
            ref_distance: 1.,
            max_distance: 10000.,
            rolloff_factor: 1.,
        }
    }
}
impl Spatial {
    /// Reject invalid authored or restored controls before evaluation.
    pub fn valid(self) -> bool {
        self.ref_distance.is_finite()
            && self.ref_distance >= 0.
            && self.max_distance.is_finite()
            && self.max_distance > 0.
            && self.rolloff_factor.is_finite()
            && self.rolloff_factor >= 0.
    }
    /// Web Audio attenuation. Invalid controls or distances produce silence.
    pub fn attenuation(self, distance: f32) -> f32 {
        if !self.valid() || !distance.is_finite() || distance < 0. {
            return 0.;
        }
        let reference = self.ref_distance;
        match self.distance_model {
            DistanceModel::Linear => {
                let lo = reference.min(self.max_distance);
                let hi = reference.max(self.max_distance);
                if lo == hi {
                    return 1.;
                }
                1. - self.rolloff_factor.min(1.) * (distance.clamp(lo, hi) - lo) / (hi - lo)
            }
            DistanceModel::Inverse => {
                if reference == 0. {
                    return 0.;
                }
                reference
                    / (reference + self.rolloff_factor * (distance.max(reference) - reference))
            }
            DistanceModel::Exponential => {
                if reference == 0. {
                    return 0.;
                }
                crate::math::powf(distance.max(reference) / reference, -self.rolloff_factor)
            }
        }
    }
}
