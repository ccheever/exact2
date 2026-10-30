use super::*;
use objc2_foundation::NSRange;

impl Metal {
    // Metal's mip blit linearly samples odd dimensions (3→1 selects the
    // middle pixel). Use the core's area filter for those chains, as wgpu does.
    pub(super) fn area_mips(
        &self,
        texture: &Texture,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
    ) -> Result<()> {
        for level in 1..texture.desc.levels() {
            let width = (texture.desc.width >> level).max(1);
            let height = (texture.desc.height >> level).max(1);
            let view = |level| {
                // SAFETY: each view has one existing mip and slice zero.
                unsafe {
                    texture
                        .raw
                        .newTextureViewWithPixelFormat_textureType_levels_slices(
                            MTLPixelFormat::RGBA8Unorm,
                            MTLTextureType::Type2D,
                            NSRange::new(level as usize, 1),
                            NSRange::new(0, 1),
                        )
                }
                .ok_or_else(|| Error::Device("mip view allocation".into()))
            };
            let source = view(level - 1)?;
            let target = view(level)?;
            let pass = MTLRenderPassDescriptor::new();
            // SAFETY: attachment zero is in range.
            let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
            attachment.setTexture(Some(&target));
            attachment.setLoadAction(MTLLoadAction::DontCare);
            attachment.setStoreAction(MTLStoreAction::Store);
            let encoder = command
                .renderCommandEncoderWithDescriptor(&pass)
                .ok_or_else(|| Error::Device("mip encoder".into()))?;
            encoder.setRenderPipelineState(&self.replace_pipeline);
            let viewport = [width as f32, height as f32, 0., 0.];
            let quad = Instance::new(
                Kind::Downsample,
                crate::Brush::Solid,
                [0., 0., width as f32, height as f32],
            );
            encoder.setViewport(MTLViewport {
                originX: 0.,
                originY: 0.,
                width: width as f64,
                height: height as f64,
                znear: 0.,
                zfar: 1.,
            });
            // SAFETY: Metal copies these bytes. The single instance has the
            // pipeline's stride; source and destination are distinct mip levels.
            unsafe {
                encoder.setVertexBytes_length_atIndex(NonNull::from(&viewport).cast(), 16, 0);
                encoder.setVertexBytes_length_atIndex(
                    NonNull::from(&quad).cast(),
                    size_of::<Instance>(),
                    1,
                );
                for (index, t) in [0, 1, 2, 3, 4, 6, 7]
                    .into_iter()
                    .zip(self.bindings(Bindings::default())?)
                {
                    encoder.setFragmentTexture_atIndex(
                        Some(if index == 4 { &source } else { t }),
                        index,
                    );
                }
                encoder.setFragmentSamplerState_atIndex(Some(&self.sampler), 0);
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                    1,
                    0,
                );
            }
            encoder.endEncoding();
        }
        Ok(())
    }
}
