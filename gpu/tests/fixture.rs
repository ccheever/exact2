//! The readback helper (`exact_gpu::fixture`): a surface's frame comes back
//! as tightly packed pixels whatever the copy's row padding, and as a PPM.
//! Skips, saying so, without an adapter.

use exact_gpu::fixture;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Surface, SurfaceError, Value};

/// Clears to one color.
struct Fill([f64; 4]);

impl Surface for Fill {
    fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
        Ok(())
    }

    fn render(
        &mut self,
        _: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        _: wgpu::TextureFormat,
    ) -> bool {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: self.0[0],
                        g: self.0[1],
                        b: self.0[2],
                        a: self.0[3],
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        queue.submit([encoder.finish()]);
        false
    }
}

#[test]
fn a_frame_reads_back_unpadded_and_as_a_ppm() {
    let gpu = match fixture::device() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}; the readback test is skipped");
            return;
        }
    };
    let mut fill = Fill([0.2, 0.4, 0.6, 1.0]);
    // Three wide: a 12-byte row the copy pads to 256.
    let frame = Frame {
        width: 3.0,
        height: 2.0,
        scale: 1.0,
        now_ms: 0.0,
    };
    let (px, wants) = fixture::render(&gpu, &mut fill, &frame).unwrap();
    assert!(!wants);
    assert_eq!((px.width, px.height, px.data.len()), (3, 2, 24));
    assert_eq!(px.at(2, 1), [51, 102, 153, 255]);
    assert_eq!(px.count(|p| p == [51, 102, 153, 255]), 6);
    let ppm = px.ppm();
    assert_eq!(&ppm[..11], b"P6\n3 2\n255\n");
    assert_eq!(ppm.len(), 11 + 18);
}
