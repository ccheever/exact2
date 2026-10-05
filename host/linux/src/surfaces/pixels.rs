//! The portable native display baseline: GPU canvas readback composed by the
//! same painter as Canvas 2D. Device-free agent runs never enter this path.
use super::*;
use std::sync::Arc;
use tiny_skia::{IntSize, Pixmap};

impl Surfaces {
    pub(crate) fn has_rendered_canvas(&self) -> bool {
        self.canvases
            .values()
            .any(|c| self.abis.get(&c.artifact).is_some_and(|a| a.rendered))
    }
    pub(crate) fn pixels<D: DataSource>(
        &mut self,
        host: &mut Host<D>,
        scale: f32,
    ) -> BTreeMap<u32, crate::canvas2d::CanvasPaint> {
        let mut result = BTreeMap::new();
        for (&view, canvas) in &self.canvases {
            let Some(abi) = self.abis.get(&canvas.artifact).filter(|a| a.rendered) else {
                continue;
            };
            let Some(node) = host.kernel().node(view) else {
                continue;
            };
            let (width, height) = (node.frame.width, node.frame.height);
            if width <= 0.0 || height <= 0.0 {
                continue;
            }
            let (w, h) = (
                (width * scale).round() as u32,
                (height * scale).round() as u32,
            );
            let Some(size) = IntSize::from_wh(w, h) else {
                continue;
            };
            let Some(len) = (w as usize)
                .checked_mul(h as usize)
                .and_then(|n| n.checked_mul(4))
                .filter(|&n| n <= LIMIT)
            else {
                host.log("GPU canvas exceeds native readback limit");
                continue;
            };
            let mut bytes = vec![0; len];
            let code = unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, f32, f32, f32, f64, *mut u8, usize) -> u32>(
                    b"gpu_readback",
                )(
                    canvas.id,
                    width,
                    height,
                    scale,
                    host.now(),
                    bytes.as_mut_ptr(),
                    bytes.len(),
                )
            };
            if matches!(code, 0 | 2) {
                if let Some(pixels) = Pixmap::from_vec(bytes, size) {
                    result.insert(
                        view,
                        crate::canvas2d::CanvasPaint {
                            pixels: Arc::new(pixels),
                        },
                    );
                }
            } else {
                host.log(format!(
                    "GPU canvas readback failed ({code}): {}",
                    abi.error().unwrap_or_default()
                ));
            }
        }
        result
    }
}
