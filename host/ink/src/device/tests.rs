use super::*;
use crate::{atlas::Atlas, instance::*, layer::Pool};
use std::collections::HashMap;

#[derive(Default)]
struct Delayed {
    next: u64,
    submitted: Submission,
    done: Submission,
    textures: HashMap<TextureId, Submission>,
    uploads: HashMap<TextureId, Submission>,
    retired: Vec<(TextureId, Submission)>,
}
impl Device for Delayed {
    type Buffer = ();
    fn info(&self) -> &Info {
        unimplemented!()
    }
    fn create_buffer(&self, _: u64) -> Result<()> {
        Ok(())
    }
    fn upload_buffer(&mut self, _: &(), _: u64, _: &[u8]) -> Result<Submission> {
        Ok(self.submitted)
    }
    fn create_texture(&mut self, _: TextureDesc) -> Result<TextureId> {
        self.next += 1;
        let id = TextureId(self.next);
        self.textures.insert(id, Submission(0));
        Ok(id)
    }
    fn upload_texture(
        &mut self,
        id: TextureId,
        _: [u32; 2],
        _: [u32; 2],
        _: &[u8],
    ) -> Result<Submission> {
        assert!(
            self.textures[&id] <= self.done || self.uploads.get(&id) == Some(&self.textures[&id]),
            "overwrote an in-flight atlas page"
        );
        self.submitted.0 += 1;
        self.textures.insert(id, self.submitted);
        self.uploads.insert(id, self.submitted);
        Ok(self.submitted)
    }
    fn last_use(&self, id: TextureId) -> Result<Submission> {
        Ok(self.textures[&id])
    }
    fn generate_mips(&mut self, _: TextureId) -> Result<Submission> {
        Ok(self.submitted)
    }
    fn resize_target(&mut self, id: TextureId, _: u32, _: u32) -> Result<TextureId> {
        Ok(id)
    }
    fn copy_region(
        &mut self,
        _: TextureId,
        _: [u32; 2],
        _: TextureId,
        _: [u32; 2],
        _: [u32; 2],
    ) -> Result<Submission> {
        unimplemented!()
    }
    fn render(&mut self, f: &Frame, target: TextureId, _: Option<[f64; 4]>) -> Result<Submission> {
        self.submitted.0 += 1;
        self.textures.insert(target, self.submitted);
        for b in &f.batches {
            for id in [b.bindings.path, b.bindings.glyph, b.bindings.image] {
                if id.0 != 0 {
                    self.textures.insert(id, self.submitted);
                }
            }
        }
        Ok(self.submitted)
    }
    fn readback(&mut self, _: TextureId) -> Result<Vec<u8>> {
        unimplemented!()
    }
    fn wait(&mut self, _: Submission) -> Result<()> {
        panic!("reuse must choose available storage")
    }
    fn completed(&mut self) -> Result<Submission> {
        self.retired.retain(|(id, s)| {
            if *s <= self.done {
                self.textures.remove(id);
                false
            } else {
                true
            }
        });
        Ok(self.done)
    }
    fn retire_texture(&mut self, id: TextureId) -> Result<()> {
        self.retired.push((id, self.textures[&id]));
        Ok(())
    }
    fn trim(&mut self) -> Result<()> {
        self.completed().map(|_| ())
    }
}

#[test]
fn atlas_appends_ordered_uploads_without_waiting_for_completion() {
    let mut d = Delayed::default();
    let mut atlas = Atlas::paths();
    atlas.begin(&mut d).unwrap();
    let first = atlas.insert(&mut d, vec![0], [8, 8], &[73; 64]).unwrap();
    for key in 1..32 {
        let entry = atlas.insert(&mut d, vec![key], [8, 8], &[181; 64]).unwrap();
        assert_eq!(entry.texture, first.texture);
        assert_ne!(entry.uv, first.uv);
    }
    assert_eq!(atlas.stats().entries, 32);
    assert_eq!(atlas.stats().overflow_textures, 0);
    assert_eq!(d.submitted, Submission(32));
    assert_eq!(d.done, Submission(0));
}

#[test]
fn a_delayed_device_keeps_pool_targets_and_atlas_uploads_separate() {
    let mut d = Delayed::default();
    let mut pool = Pool::default();
    pool.begin(&mut d, [8, 8]).unwrap();
    let first = pool.take(&mut d).unwrap();
    let submitted = d.render(&Frame::default(), first, Some([0.; 4])).unwrap();
    pool.begin(&mut d, [8, 8]).unwrap();
    let second = pool.take(&mut d).unwrap();
    assert_ne!(first, second, "unfinished pool target must not be reused");
    d.done = submitted;
    pool.begin(&mut d, [8, 8]).unwrap();
    let mut reused = [pool.take(&mut d).unwrap().0, pool.take(&mut d).unwrap().0];
    reused.sort();
    assert_eq!(reused, [first.0, second.0]);
    let mut atlas = Atlas::paths();
    atlas.begin(&mut d).unwrap();
    let a = atlas.insert(&mut d, vec![1], [8, 8], &[127; 64]).unwrap();
    let mut e = Encoder::new(8, 8);
    let mut i = Instance::new(Kind::Coverage, Brush::Solid, [0., 0., 8., 8.]);
    i.uv = a.uv;
    e.push(
        i,
        Bindings {
            path: a.texture,
            ..Default::default()
        },
    )
    .unwrap();
    d.render(&e.finish(), first, None).unwrap();
    atlas.begin(&mut d).unwrap();
    let b = atlas.insert(&mut d, vec![2], [8, 8], &[211; 64]).unwrap();
    assert_ne!(a.texture, b.texture);
    pool.trim(&mut d).unwrap();
    assert!(
        d.textures.contains_key(&first),
        "trim retains in-flight storage"
    );
    d.done = d.submitted;
    d.completed().unwrap();
    assert!(!d.textures.contains_key(&first));
}

#[test]
fn device_loss_retry_deadlines_advance_and_back_off() {
    let mut retry = Retry::default();
    let mut now = std::time::Instant::now();
    for ms in [250, 500, 1000, 2000, 4000, 8000, 16000, 30000, 30000] {
        assert!(retry.ready(now));
        retry.failed(now);
        let due = retry.deadline().unwrap();
        assert_eq!(due.duration_since(now).as_millis(), ms);
        assert!(!retry.ready(now));
        now = due;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn consecutive_atlas_uploads(d: &mut impl Device) {
    let mut atlas = Atlas::new([16, 8], Format::Alpha, 0, 1);
    atlas.begin(d).unwrap();
    let mut pixels = [73; 64];
    let first = atlas.insert(d, vec![0], [8, 8], &pixels).unwrap();
    pixels.fill(211);
    let second = atlas.insert(d, vec![1], [8, 8], &pixels).unwrap();
    pixels.fill(0);
    assert_eq!(first.texture, second.texture);
    let target = d.create_target(16, 8).unwrap();
    let mut encoder = Encoder::new(16, 8);
    for (x, entry) in [(0., first), (8., second)] {
        let mut instance = Instance::new(Kind::Coverage, Brush::Solid, [x, 0., 8., 8.]);
        instance.uv = entry.uv;
        encoder
            .push(
                instance,
                Bindings {
                    path: entry.texture,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    d.render(&encoder.finish(), target, Some([0.; 4])).unwrap();
    let result = d.readback(target).unwrap();
    for (i, pixel) in result.chunks_exact(4).enumerate() {
        let expected = if i % 16 < 8 { 73 } else { 211 };
        assert!(
            pixel.iter().all(|c| c.abs_diff(expected) <= 1),
            "pixel {i}: {pixel:?}"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn wgpu_draws_both_consecutive_atlas_uploads() {
    consecutive_atlas_uploads(&mut super::wgpu::Wgpu::new().unwrap());
}

#[cfg(exact_ink_metal)]
#[test]
fn metal_draws_both_consecutive_atlas_uploads() {
    consecutive_atlas_uploads(&mut super::metal::Metal::new().unwrap());
}

#[cfg(windows)]
#[test]
fn d3d11_draws_both_consecutive_atlas_uploads() {
    consecutive_atlas_uploads(&mut super::d3d11::D3d11::new().unwrap());
}

#[cfg(not(target_arch = "wasm32"))]
fn in_flight_storage(d: &mut impl Device) {
    let mut atlas = Atlas::new([16, 16], Format::Alpha, 0, 1);
    let mut pool = Pool::default();
    let outputs = [
        d.create_target(64, 64).unwrap(),
        d.create_target(64, 64).unwrap(),
    ];
    for (frame, alpha) in [73u8, 211].into_iter().enumerate() {
        atlas.begin(d).unwrap();
        pool.begin(d, [64, 64]).unwrap();
        let entry = atlas
            .insert(d, vec![frame as u32], [16, 16], &[alpha; 256])
            .unwrap();
        let layer = pool.take(d).unwrap();
        let mut encoder = Encoder::new(64, 64);
        let mut i = Instance::new(Kind::Coverage, Brush::Solid, [0., 0., 64., 64.]);
        i.uv = entry.uv;
        encoder
            .push(
                i,
                Bindings {
                    path: entry.texture,
                    ..Default::default()
                },
            )
            .unwrap();
        d.render(&encoder.finish(), layer, Some([0.; 4])).unwrap();
        d.copy_region(layer, [0, 0], outputs[frame], [0, 0], [64, 64])
            .unwrap();
        // Retire pool storage and force atlas eviction before either readback.
        pool.begin(d, [32, 32]).unwrap();
        let spare = pool.take(d).unwrap();
        d.render(&Frame::default(), spare, Some([1., 0., 0., 1.]))
            .unwrap();
        atlas.begin(d).unwrap();
        atlas
            .insert(d, vec![100 + frame as u32], [16, 16], &[19; 256])
            .unwrap();
    }
    for (output, alpha) in outputs.into_iter().zip([73u8, 211]) {
        let pixels = d.readback(output).unwrap();
        assert!(pixels
            .chunks_exact(4)
            .all(|p| p.iter().all(|c| c.abs_diff(alpha) <= 1)));
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn wgpu_preserves_submitted_frames_during_atlas_and_pool_eviction() {
    in_flight_storage(&mut super::wgpu::Wgpu::new().unwrap());
}
#[cfg(exact_ink_metal)]
#[test]
fn metal_preserves_submitted_frames_during_atlas_and_pool_eviction() {
    in_flight_storage(&mut super::metal::Metal::new().unwrap());
}
#[cfg(windows)]
#[test]
fn d3d11_preserves_submitted_frames_during_atlas_and_pool_eviction() {
    in_flight_storage(&mut super::d3d11::D3d11::new().unwrap());
}
