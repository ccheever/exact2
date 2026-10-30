//! Viewport targets reused after the frame has consumed their composites.
//! @ref LLP 1149 D2.5, D11.
use crate::counters::{self, Counter};
use crate::device::{Device, Result};
use crate::instance::TextureId;

#[derive(Default)]
pub struct Pool {
    size: [u32; 2],
    targets: Vec<(TextureId, u64, [u32; 2])>,
    frame: u64,
    used: usize,
}

impl Pool {
    pub fn used(&self) -> usize {
        self.used
    }
    pub fn retained(&self) -> usize {
        self.targets.len()
    }
    pub fn bytes(&self) -> u64 {
        self.targets
            .iter()
            .map(|(_, _, s)| u64::from(s[0]) * u64::from(s[1]) * 4)
            .sum()
    }

    pub fn begin(&mut self, device: &mut impl Device, size: [u32; 2]) -> Result<()> {
        let result = (|| {
            self.frame += 1;
            for (target, _, _) in self
                .targets
                .extract_if(.., |(_, seen, _)| self.frame.saturating_sub(*seen) > 120)
            {
                device.retire_texture(target)?;
                counters::add(Counter::TargetsRetired, 1);
            }
            if self.size != size {
                for (target, _, _) in self.targets.drain(..) {
                    device.retire_texture(target)?;
                    counters::add(Counter::TargetsRetired, 1);
                }
                self.size = size;
            }
            self.used = 0;
            Ok(())
        })();
        self.observe_ownership();
        result
    }

    pub fn trim(&mut self, device: &mut impl Device) -> Result<()> {
        let result = (|| {
            for (target, _, _) in self.targets.drain(..) {
                device.retire_texture(target)?;
                counters::add(Counter::TargetsRetired, 1);
            }
            self.used = 0;
            device.trim()
        })();
        self.observe_ownership();
        result
    }

    fn observe_ownership(&self) {
        counters::observe(|s| self.ownership(s));
    }

    fn ownership(&self, s: &mut counters::Snapshot) {
        s.gauge(Counter::TargetsRetained, self.retained() as u64);
        s.gauge(Counter::TargetBytes, self.bytes());
        s.gauge(
            Counter::TargetsReserved,
            self.used.min(self.retained()) as u64,
        );
    }

    pub fn take(&mut self, device: &mut impl Device) -> Result<TextureId> {
        self.take_sized(device, self.size)
    }

    pub fn take_sized(&mut self, device: &mut impl Device, size: [u32; 2]) -> Result<TextureId> {
        let observing = counters::enabled();
        let completed = device.completed()?;
        let mut available = None;
        let mut blocked = false;
        for i in self.used..self.targets.len() {
            if self.targets[i].2 == size {
                if device.last_use(self.targets[i].0)? <= completed {
                    available = Some(i);
                    break;
                }
                if observing {
                    blocked = true;
                    counters::add(Counter::TargetCandidatesBlocked, 1);
                }
            }
        }
        if let Some(i) = available {
            self.targets.swap(self.used, i);
        } else {
            if blocked {
                counters::add(Counter::TargetBlocked, 1);
            }
            self.targets
                .push((device.create_target(size[0], size[1])?, self.frame, size));
            if observing {
                counters::add(Counter::TargetsAllocated, 1);
            }
            let last = self.targets.len() - 1;
            self.targets.swap(self.used, last);
        }
        self.targets[self.used].1 = self.frame;
        let target = self.targets[self.used].0;
        self.used += 1;
        if observing {
            self.observe(device, completed)?;
        }
        Ok(target)
    }

    /// Observation only: no completion poll, allocation or retirement is added.
    pub fn observe(
        &self,
        device: &impl Device,
        completed: crate::device::Submission,
    ) -> Result<()> {
        counters::observe(|s| {
            self.ownership(s);
            let mut busy = 0;
            let mut idle_bytes = 0;
            for &(target, _, size) in &self.targets[self.used..] {
                if device.last_use(target)? > completed {
                    busy += 1;
                } else {
                    idle_bytes += u64::from(size[0]) * u64::from(size[1]) * 4;
                }
            }
            s.gauge(Counter::TargetsBusy, busy);
            s.gauge(
                Counter::TargetsIdle,
                (self.retained() - self.used) as u64 - busy,
            );
            s.gauge(Counter::TargetIdleBytes, idle_bytes);
            s.gauge(Counter::TargetPartitionCompleted, completed.0);
            s.gauge(
                Counter::TargetPartitionAt,
                counters::timestamp(std::time::Instant::now()) as u64,
            );
            Ok(())
        })
        .unwrap_or(Ok(()))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn unused_targets_retire_after_the_idle_window() {
        let mut d = crate::Wgpu::new().unwrap();
        let mut pool = Pool::default();
        pool.begin(&mut d, [16, 16]).unwrap();
        pool.take(&mut d).unwrap();
        assert_gauges(&pool, 1);
        pool.begin(&mut d, [16, 16]).unwrap();
        assert_gauges(&pool, 0);
        for _ in 0..121 {
            pool.begin(&mut d, [16, 16]).unwrap();
        }
        assert_eq!(pool.retained(), 0);
        assert_gauges(&pool, 0);
        pool.take(&mut d).unwrap();
        pool.begin(&mut d, [32, 32]).unwrap();
        assert_eq!(pool.retained(), 0);
        assert_gauges(&pool, 0);
        pool.take(&mut d).unwrap();
        pool.trim(&mut d).unwrap();
        assert_eq!(pool.retained(), 0);
        assert_gauges(&pool, 0);
    }

    fn assert_gauges(pool: &Pool, reserved: u64) {
        if let Some(s) = counters::snapshot() {
            assert_eq!(
                s.values[Counter::TargetsRetained as usize],
                pool.retained() as u64
            );
            assert_eq!(s.values[Counter::TargetBytes as usize], pool.bytes());
            assert_eq!(s.values[Counter::TargetsReserved as usize], reserved);
        }
    }
}
