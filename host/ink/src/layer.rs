//! Viewport targets reused after the frame has consumed their composites.
//! @ref LLP 1149 D2.5, D11.
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
        self.frame += 1;
        for (target, _, _) in self
            .targets
            .extract_if(.., |(_, seen, _)| self.frame.saturating_sub(*seen) > 120)
        {
            device.retire_texture(target)?;
        }
        if self.size != size {
            for (target, _, _) in self.targets.drain(..) {
                device.retire_texture(target)?;
            }
            self.size = size;
        }
        self.used = 0;
        Ok(())
    }

    pub fn trim(&mut self, device: &mut impl Device) -> Result<()> {
        for (target, _, _) in self.targets.drain(..) {
            device.retire_texture(target)?;
        }
        self.used = 0;
        device.trim()
    }

    pub fn take(&mut self, device: &mut impl Device) -> Result<TextureId> {
        self.take_sized(device, self.size)
    }

    pub fn take_sized(&mut self, device: &mut impl Device, size: [u32; 2]) -> Result<TextureId> {
        let completed = device.completed()?;
        let mut available = None;
        for i in self.used..self.targets.len() {
            if self.targets[i].2 == size && device.last_use(self.targets[i].0)? <= completed {
                available = Some(i);
                break;
            }
        }
        if let Some(i) = available {
            self.targets.swap(self.used, i);
        } else {
            self.targets
                .push((device.create_target(size[0], size[1])?, self.frame, size));
            let last = self.targets.len() - 1;
            self.targets.swap(self.used, last);
        }
        let target = &mut self.targets[self.used];
        target.1 = self.frame;
        self.used += 1;
        Ok(target.0)
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
        for _ in 0..121 {
            pool.begin(&mut d, [16, 16]).unwrap();
        }
        assert_eq!(pool.retained(), 0);
    }
}
