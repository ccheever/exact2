//! Recency-weighted least-squares velocity tracking with typed provenance.
//!
//! @ref LLP 0099#velocity-tracking

use super::*;
use std::cmp::Ordering;
use std::collections::VecDeque;

/// Position samples retained per tracked pointer.
pub const VELOCITY_SAMPLE_CAP: usize = 20;
/// Age beyond which a retained sample no longer contributes, in milliseconds.
pub const VELOCITY_HORIZON_MS: f64 = 100.0;

/// Which fit produced an estimate. Reported as provenance so a consumer can
/// tell a measurement from a fallback without re-running the math.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelocityEstimator {
    /// Fewer than two samples were retained, so no fit ran and the reported
    /// velocity is zero rather than a guess.
    InsufficientSamples,
    /// A recency-weighted linear fit. Used with exactly two samples, and as the
    /// fallback when the quadratic normal equations are ill-conditioned.
    WeightedLinear,
    /// A recency-weighted quadratic fit over three or more samples, taking the
    /// first-order coefficient as the velocity. Preferred whenever the sample
    /// geometry is well conditioned.
    WeightedQuadratic,
}

/// Everything that shaped one estimate: which fit ran, how much data it saw,
/// and under what configuration. It travels with every estimate so a flick or
/// fling decision can be explained after the fact.
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityProvenance {
    /// Which fit produced the velocity, including the case where none did.
    pub estimator: VelocityEstimator,
    /// Samples the fit ran over, after the cap and the horizon were applied.
    pub sample_count: usize,
    /// The tracker's configured horizon in milliseconds. A sample older than
    /// this behind the newest one is dropped.
    pub configured_horizon_ms: f64,
    /// Span in milliseconds between the oldest and newest retained samples.
    /// Zero when fewer than two remain, and never above the configured horizon.
    pub observed_horizon_ms: f64,
    /// The tracker's retained-sample limit. Once exceeded, the oldest sample is
    /// dropped before the horizon trim runs.
    pub sample_cap: usize,
    /// Timestamp of the oldest retained sample in milliseconds, or `None` when
    /// the tracker holds nothing.
    pub oldest_timestamp_ms: Option<f64>,
    /// Timestamp of the newest retained sample in milliseconds, or `None` when
    /// the tracker holds nothing.
    pub newest_timestamp_ms: Option<f64>,
    /// The contact set every retained sample came from, or `None` before the
    /// first sample. A tracker refuses samples from a different set rather than
    /// blending two gestures into one fit.
    pub pointer_set: Option<PointerSetIdentity>,
}

/// A velocity together with the record of how it was obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityEstimate {
    /// Estimated velocity in points per second along each axis. Zero whenever
    /// the estimator is [`VelocityEstimator::InsufficientSamples`].
    pub velocity_per_second: MotionPoint,
    /// How this estimate was produced, and from what.
    pub provenance: VelocityProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VelocitySample {
    timestamp_ms: f64,
    position: MotionPoint,
}

/// Recency-weighted least-squares velocity tracker with a deterministic
/// horizon and sample cap. A quadratic fit is used when the sample geometry is
/// well conditioned; degenerate input falls back to a weighted linear fit.
#[derive(Debug, Clone)]
pub struct VelocityTracker {
    horizon_ms: f64,
    sample_cap: usize,
    pointer_set: Option<PointerSetIdentity>,
    samples: VecDeque<VelocitySample>,
}

impl Default for VelocityTracker {
    fn default() -> Self {
        Self::new(VELOCITY_HORIZON_MS, VELOCITY_SAMPLE_CAP)
            .expect("default velocity tracker configuration is valid")
    }
}

impl VelocityTracker {
    /// Build a tracker with an explicit horizon and sample cap. `horizon_ms`
    /// must be finite, positive, and no larger than [`VELOCITY_HORIZON_MS`];
    /// `sample_cap` must be at least 2 and no larger than
    /// [`VELOCITY_SAMPLE_CAP`]. Those constants are ceilings, not defaults: a
    /// caller may narrow the window, never widen it.
    pub fn new(horizon_ms: f64, sample_cap: usize) -> Result<Self, GestureInputError> {
        if !horizon_ms.is_finite() {
            return Err(GestureInputError::NonFinite("velocity.horizon_ms"));
        }
        if horizon_ms <= 0.0 || horizon_ms > VELOCITY_HORIZON_MS {
            return Err(GestureInputError::InvalidHorizon);
        }
        if !(2..=VELOCITY_SAMPLE_CAP).contains(&sample_cap) {
            return Err(GestureInputError::InvalidCapacity);
        }
        Ok(Self {
            horizon_ms,
            sample_cap,
            pointer_set: None,
            samples: VecDeque::with_capacity(sample_cap),
        })
    }

    /// Drop every retained sample and release the pointer-set binding, so the
    /// tracker can be reused for a different contact set. The configured horizon
    /// and sample cap survive.
    pub fn clear(&mut self) {
        self.pointer_set = None;
        self.samples.clear();
    }

    /// Record one position observation, in points, at `timestamp_ms`. The first
    /// sample binds the tracker to `pointer_set`; a later sample from a
    /// different set is refused rather than blended. A repeated timestamp
    /// overwrites the previous position instead of adding a sample, and an
    /// earlier timestamp is refused outright. Retention is then trimmed to the
    /// sample cap and to the horizon, in that order.
    pub fn add_sample(
        &mut self,
        pointer_set: &PointerSetIdentity,
        position: MotionPoint,
        timestamp_ms: f64,
    ) -> Result<(), GestureInputError> {
        if !position.is_finite() || !timestamp_ms.is_finite() {
            return Err(GestureInputError::NonFinite("velocity.sample"));
        }
        match &self.pointer_set {
            Some(existing) if existing != pointer_set => {
                return Err(GestureInputError::StreamChanged)
            }
            None => self.pointer_set = Some(pointer_set.clone()),
            _ => {}
        }
        if let Some(last) = self.samples.back_mut() {
            if timestamp_ms < last.timestamp_ms {
                return Err(GestureInputError::TimestampMovedBackward);
            }
            if timestamp_ms == last.timestamp_ms {
                last.position = position;
                return Ok(());
            }
        }
        self.samples.push_back(VelocitySample {
            timestamp_ms,
            position,
        });
        while self.samples.len() > self.sample_cap {
            self.samples.pop_front();
        }
        while self.samples.len() > 1
            && self.samples.back().expect("non-empty").timestamp_ms
                - self.samples.front().expect("non-empty").timestamp_ms
                > self.horizon_ms
        {
            self.samples.pop_front();
        }
        Ok(())
    }

    /// Fit the retained samples and report the velocity in points per second
    /// with its provenance. This never fails: with fewer than two samples it
    /// returns zero and says so through the estimator. The result is a pure
    /// function of the retained samples, so identical input yields identical
    /// bits.
    pub fn estimate(&self) -> VelocityEstimate {
        let oldest = self.samples.front().map(|sample| sample.timestamp_ms);
        let newest = self.samples.back().map(|sample| sample.timestamp_ms);
        let observed_horizon_ms = match (oldest, newest) {
            (Some(oldest), Some(newest)) => newest - oldest,
            _ => 0.0,
        };
        let (velocity_per_second, estimator) = if self.samples.len() >= 3 {
            match weighted_quadratic_velocity(&self.samples) {
                Some(velocity) => (velocity, VelocityEstimator::WeightedQuadratic),
                None => (
                    weighted_linear_velocity(&self.samples).unwrap_or(MotionPoint::ZERO),
                    VelocityEstimator::WeightedLinear,
                ),
            }
        } else if self.samples.len() == 2 {
            (
                weighted_linear_velocity(&self.samples).unwrap_or(MotionPoint::ZERO),
                VelocityEstimator::WeightedLinear,
            )
        } else {
            (MotionPoint::ZERO, VelocityEstimator::InsufficientSamples)
        };
        VelocityEstimate {
            velocity_per_second,
            provenance: VelocityProvenance {
                estimator,
                sample_count: self.samples.len(),
                configured_horizon_ms: self.horizon_ms,
                observed_horizon_ms,
                sample_cap: self.sample_cap,
                oldest_timestamp_ms: oldest,
                newest_timestamp_ms: newest,
                pointer_set: self.pointer_set.clone(),
            },
        }
    }
}

fn sample_weight(index: usize, count: usize) -> f64 {
    if count <= 1 {
        1.0
    } else {
        1.0 + 3.0 * index as f64 / (count - 1) as f64
    }
}

fn weighted_linear_velocity(samples: &VecDeque<VelocitySample>) -> Option<MotionPoint> {
    let newest = samples.back()?.timestamp_ms;
    let mut sum_w = 0.0;
    let mut sum_t = 0.0;
    let mut sum_tt = 0.0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_tx = 0.0;
    let mut sum_ty = 0.0;
    for (index, sample) in samples.iter().enumerate() {
        let weight = sample_weight(index, samples.len());
        let time = (sample.timestamp_ms - newest) / 1_000.0;
        sum_w += weight;
        sum_t += weight * time;
        sum_tt += weight * time * time;
        sum_x += weight * sample.position.x;
        sum_y += weight * sample.position.y;
        sum_tx += weight * time * sample.position.x;
        sum_ty += weight * time * sample.position.y;
    }
    let denominator = sum_w * sum_tt - sum_t * sum_t;
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    Some(MotionPoint {
        x: (sum_w * sum_tx - sum_t * sum_x) / denominator,
        y: (sum_w * sum_ty - sum_t * sum_y) / denominator,
    })
}

fn weighted_quadratic_velocity(samples: &VecDeque<VelocitySample>) -> Option<MotionPoint> {
    let newest = samples.back()?.timestamp_ms;
    let mut normal = [[0.0; 3]; 3];
    let mut rhs_x = [0.0; 3];
    let mut rhs_y = [0.0; 3];
    for (index, sample) in samples.iter().enumerate() {
        let weight = sample_weight(index, samples.len());
        let time = (sample.timestamp_ms - newest) / 1_000.0;
        let basis = [1.0, time, time * time];
        for row in 0..3 {
            rhs_x[row] += weight * basis[row] * sample.position.x;
            rhs_y[row] += weight * basis[row] * sample.position.y;
            for column in 0..3 {
                normal[row][column] += weight * basis[row] * basis[column];
            }
        }
    }
    let coefficients_x = solve_three_by_three(normal, rhs_x)?;
    let coefficients_y = solve_three_by_three(normal, rhs_y)?;
    Some(MotionPoint {
        x: coefficients_x[1],
        y: coefficients_y[1],
    })
}

fn solve_three_by_three(mut matrix: [[f64; 3]; 3], mut rhs: [f64; 3]) -> Option<[f64; 3]> {
    for pivot in 0..3 {
        let best = (pivot..3).max_by(|left, right| {
            matrix[*left][pivot]
                .abs()
                .partial_cmp(&matrix[*right][pivot].abs())
                .unwrap_or(Ordering::Equal)
        })?;
        if matrix[best][pivot].abs() <= 1.0e-12 {
            return None;
        }
        matrix.swap(pivot, best);
        rhs.swap(pivot, best);
        let divisor = matrix[pivot][pivot];
        for value in matrix[pivot].iter_mut().skip(pivot) {
            *value /= divisor;
        }
        rhs[pivot] /= divisor;
        let pivot_row = matrix[pivot];
        for row in 0..3 {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            for (value, pivot_value) in matrix[row].iter_mut().zip(pivot_row.iter()).skip(pivot) {
                *value -= factor * pivot_value;
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    Some(rhs)
}
