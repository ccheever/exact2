//! GPU overrides keep Vello's atlas metadata from retaining CPU raster owners.
use crate::image::Bitmap;
use std::collections::HashMap;
use std::sync::Arc;
use vello::peniko::{Blob, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality};

const ENTRIES: usize = exact_raster::SUBSCRIPTIONS + exact_raster::COLD_ENTRIES + 2;
#[derive(Default)]
pub(super) struct ImageCache {
    entries: HashMap<usize, (Arc<Bitmap>, ImageBrush)>,
}
impl ImageCache {
    // Called after scene.reset, never while that scene can reference a retired
    // override. Live pictures and the core's <=64 cold rasters remain reusable.
    pub fn begin(&mut self, mut unregister: impl FnMut(ImageData)) {
        for (_, (_, brush)) in self
            .entries
            .extract_if(|_, (image, _)| Arc::strong_count(image) == 1)
        {
            unregister(brush.image);
        }
    }
    pub fn brush(
        &mut self,
        image: &Arc<Bitmap>,
        upload: impl FnOnce(&ImageData, &Bitmap),
    ) -> Option<ImageBrush> {
        let key = Arc::as_ptr(image) as usize;
        if self.entries.len() >= ENTRIES && !self.entries.contains_key(&key) {
            return None;
        }
        Some(
            self.entries
                .entry(key)
                .or_insert_with(|| {
                    // Same public override mechanism as Renderer::register_texture,
                    // with the premultiplied alpha declaration that our decoder needs.
                    let data = ImageData {
                        data: Blob::new(Arc::new(&[])),
                        format: ImageFormat::Rgba8,
                        alpha_type: ImageAlphaType::AlphaPremultiplied,
                        width: image.width(),
                        height: image.height(),
                    };
                    upload(&data, image);
                    (
                        image.clone(),
                        ImageBrush::new(data).with_quality(ImageQuality::Medium),
                    )
                })
                .1
                .clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_raster::{Gate, RasterSession};
    fn bitmap(session: &RasterSession) -> Arc<Bitmap> {
        let reserved = session.reserve_allocation(8 * 1024 * 1024).unwrap();
        let image = Arc::new(Bitmap::new(
            tiny_skia::Pixmap::new(2048, 1024).unwrap(),
            (2048, 1024),
            reserved.charge(),
        ));
        drop(reserved.commit(8 * 1024 * 1024).unwrap());
        image
    }
    #[test]
    fn two_cache_only_rasters_release_without_a_sixty_fifth_image() {
        let session = Gate::new().session();
        let mut cache = ImageCache::default();
        let a = bitmap(&session);
        let b = bitmap(&session);
        let a_provider = cache.brush(&a, |_, _| {}).unwrap();
        let a_renderer_copy = a_provider.clone();
        assert!(a_renderer_copy.image.data.is_empty());
        assert_eq!(
            Arc::strong_count(&a),
            2,
            "only the view and native cache own CPU pixels"
        );
        drop(a_provider);
        drop(a_renderer_copy);
        let mut scene = vello::Scene::new();
        scene.draw_image(
            &cache.brush(&a, |_, _| {}).unwrap(),
            vello::kurbo::Affine::IDENTITY,
        );
        scene.draw_image(
            &cache.brush(&b, |_, _| {}).unwrap(),
            vello::kurbo::Affine::IDENTITY,
        );
        drop(a);
        drop(b);
        session.trim();
        scene.reset();
        assert_eq!(session.stats().resident_bytes, 16 * 1024 * 1024);
        let mut retired = 0;
        cache.begin(|_| retired += 1);
        assert_eq!(retired, 2);
        assert_eq!(session.stats().resident_bytes, 0);
        cache.begin(|_| panic!("quiescent cache unregisters nothing"));
    }
    #[test]
    #[ignore = "requires a native GPU adapter; run explicitly in native validation"]
    fn actual_renderer_releases_two_cold_rasters_on_the_next_begin() {
        use crate::{gpu::Gpu, paint::Backend};
        let session = Gate::new().session();
        let mut gpu = Gpu::new().expect("native GPU adapter");
        let a = bitmap(&session);
        let b = bitmap(&session);
        gpu.begin(64., 64., 1.);
        gpu.image(
            &a,
            (0., 0., 32., 64.),
            &[],
            tiny_skia::Transform::identity(),
            None,
        );
        gpu.image(
            &b,
            (32., 0., 32., 64.),
            &[],
            tiny_skia::Transform::identity(),
            None,
        );
        gpu.finish().unwrap();
        drop(a);
        drop(b);
        session.trim();
        assert_eq!(session.stats().resident_bytes, 16 * 1024 * 1024);
        gpu.begin(64., 64., 1.);
        assert_eq!(
            session.stats().resident_bytes,
            0,
            "Vello's retained Blob must retire too"
        );
        gpu.finish().unwrap();
        let reserved = session.reserve_allocation(24).unwrap();
        let mut pixels = tiny_skia::Pixmap::new(3, 2).unwrap();
        pixels.data_mut().copy_from_slice(&[
            128, 0, 0, 128, 0, 255, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255, 64, 32, 16, 128, 255, 255,
            255, 255,
        ]);
        let colored = Arc::new(Bitmap::new(pixels, (3000, 2000), reserved.charge()));
        drop(reserved.commit(24).unwrap());
        gpu.begin(3., 2., 1.);
        gpu.image(
            &colored,
            (0., 0., 3., 2.),
            &[],
            tiny_skia::Transform::identity(),
            None,
        );
        let painted = gpu.finish().unwrap();
        let expected = [
            255u8, 127, 127, 255, 0, 255, 0, 255, 255, 255, 255, 255, 0, 0, 255, 255, 191, 159,
            143, 255, 255, 255, 255, 255,
        ];
        for (actual, expected) in painted.data().iter().zip(expected) {
            assert!(
                actual.abs_diff(expected) <= 1,
                "odd-stride premultiplied pixel {actual} != {expected}"
            );
        }
        drop(colored);
        gpu.begin(3., 2., 1.);
        assert_eq!(session.stats().resident_bytes, 0);
    }
}
