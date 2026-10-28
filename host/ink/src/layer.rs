//! Viewport targets reused after the frame has consumed their composites.
//! @ref LLP 1149 D2.5, D11.
use crate::device::{Device, Result};
use crate::instance::TextureId;

#[derive(Default)]
pub struct Pool {
    size: [u32; 2],
    targets: Vec<TextureId>,
    used: usize,
}

impl Pool {
    pub fn begin(&mut self, device: &mut impl Device, size: [u32; 2]) -> Result<()> {
        if self.size != size {
            for target in self.targets.drain(..) {
                device.retire_texture(target)?;
            }
            self.size = size;
        }
        self.used = 0;
        Ok(())
    }

    pub fn take(&mut self, device: &mut impl Device) -> Result<TextureId> {
        if self.used == self.targets.len() {
            self.targets
                .push(device.create_target(self.size[0], self.size[1])?);
        }
        let target = self.targets[self.used];
        self.used += 1;
        Ok(target)
    }
}
