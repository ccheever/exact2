use super::*;
use crate::{atlas::Atlas, instance::*, layer::Pool};
use std::collections::HashMap;

#[derive(Default)]
struct Delayed {
    next: u64,
    submitted: Submission,
    done: Submission,
    textures: HashMap<TextureId, Submission>,
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
            self.textures[&id] <= self.done,
            "overwrote an in-flight atlas page"
        );
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
